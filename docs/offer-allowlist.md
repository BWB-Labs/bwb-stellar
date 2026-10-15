# offer-allowlist

- **What it does.** Records who may invest and up to how much. Each investor has an allocation: a cap in USDC stroops and whether they may invest. Usage is counted per investor per UTC calendar month (the *period*), inside the contract, so a new month starts from zero by itself. The allowlist admits addresses, never assets.
- **Scope.** One global instance, shared by every offering. A cap is global, not per offering, by design.
- **Who uses it.** The platform hot key writes caps after KYC and rewrites them monthly at the current exchange rate. Each offering's sale contract consumes room on a purchase and restores it, per month, on a refund. The platform multisig is the access-control admin and the only account that upgrades.
- **Where it comes from.** A port of Base's `OfferSaleAllowlist`, with two changes that remove Base's monthly reset and its refund-after-reset failure (ADR 0007) and with OpenZeppelin's roles in place of Base's owner and controller map (ADR 0008).

**Delivered in L3 (interface v0.7).** Testnet deployment: [`deployments/testnet.json`](../deployments/testnet.json). The cross-cutting rules are in [interface.md](interface.md).

## Periods

Usage is counted per **period**, the UTC calendar month the ledger time falls in. A period is named by one number:

```
period = year * 12 + (month - 1)        year / month in UTC, month 1–12
year   = period / 12
month  = period % 12 + 1
```

October 2026 is `2026 * 12 + 9 = 24321`. The limit renews at 00:00 UTC on the first of the month, 21:00 in Brasília, as the off-chain gate already does. The monthly limit is BWB's product choice, not CVM 88 (annual, enforced off-chain), so the period is a code constant and changing it is an upgrade.

**Only the allowlist computes periods.** `consume` returns the period it charged, the sale contract keeps what each reservation paid per period, and `restore` receives those periods back. The backend reads `current_period()` and uses the formula above to label events. A ledger time whose period does not fit a `u32` fails with 6106 `InvalidPeriod`, which no real ledger time reaches (year 357,913,941).

## Roles

OpenZeppelin `AccessControl` (ADR 0008). Base's owner becomes the admin; Base's `controllers` map becomes the `controller` role.

| Role | Held by | Can |
|---|---|---|
| admin | platform multisig | grant and revoke any role, set a role's admin role, start and complete the two-step admin transfer, **upgrade** |
| `controller` | platform hot key, and each offering's sale contract | write allocations, consume and restore room, and grant and revoke `controller` itself |

`controller` is its own role-admin, set at construction, so a controller can add the next offering's sale contract without a multisig signature, as Base's `setControllerFromController` (audit NF-01, accepted). The admin can always grant and revoke any role regardless of the role-admin.

**The admin is the trust root.** It upgrades, and a code swap can do anything. `renounce_admin` always fails with 6105 `RenounceBlocked`: an admin-less allowlist could never be upgraded. Rotating the multisig is OpenZeppelin's two-step transfer: `transfer_admin_role(new_admin, live_until_ledger)` signed by the admin, then `accept_admin_transfer()` signed by the new admin before the deadline.

**Containing a leaked hot key.** The admin, in order:

1. `set_role_admin("controller", "nobody")`: the role-admin moves to a role nobody holds, so only the admin grants or revokes `controller` from here on, and the leaked key cannot keep re-adding itself.
2. `revoke_role(leaked_key, "controller", admin)`, then the same for every account the leaked key granted (the indexer lists them from `role_granted` events with that caller).
3. `grant_role(new_key, "controller", admin)` for the replacement hot key. The sale contracts keep their role throughout, so purchases and refunds never stop.

The admin may later move the role-admin back with `set_role_admin("controller", "controller")`.

**Revoking a sale contract's `controller`** makes its purchases and refunds fail with 2000 until the role is granted again. That is the platform's way to stop one offering without a pause on the allowlist; it stops the offering's refunds too.

## Constructor

| Argument | Meaning |
|---|---|
| `admin` | Platform multisig: the access-control admin, and the only upgrader. |
| `controllers` | Initial controllers, normally the platform hot key. Each offering's deploy batch grants `controller` to its sale contract later. |

It sets the admin, makes `controller` its own role-admin, grants `controller` to each address, and writes `schema_version = 1`. It emits, in this order: `role_admin_changed` for `controller` (with an empty previous role symbol), one `role_granted` per controller (with the admin as caller), then `initialized`.

## Allocation and usage

One persistent **allocation** per investor:

| Field | Meaning |
|---|---|
| `schema` | Struct version, 1 |
| `max` | The cap, in USDC stroops: the most the investor may have in usage within one period. 0 is valid and means no room, as in Base. |
| `enabled` | Whether the investor may invest at all |

One persistent **usage** entry per `(investor, period)`, a plain `i128`. Absent means 0. A purchase adds to the current period's entry; a refund subtracts from the period the money was consumed in. Writing the allocation never touches usage, so a cap rewrite at a new exchange rate never gives or takes room already used this month.

An investor with no entry reads as `max = 0`, not enabled, usage 0. The backend's "already on the allowlist" check relies on `max > 0`, as Base's does.

**Room** is `max − usage(current period)` when the investor is enabled, else 0, and never negative: a cap lowered below this month's usage leaves no room until next month, and keeps the usage.

The cap is in USDC stroops. Where it comes from is pending the team: the recommendation is that the backend converts the investor's BRL limit into stroops when it writes an allocation and rewrites every cap monthly at the current rate ([interface 3](interface.md#3-premises), [interface 11](interface.md#11-open-points)). That monthly rewrite is the only monthly job: there is no reset.

## Calls

Every write that names its acting account takes it as the **last** argument (`caller`, or `operator` on `upgrade`), as the token's calls and OpenZeppelin's role functions do; OpenZeppelin's admin-only calls (`set_role_admin`, `transfer_admin_role`) take none and require the stored admin's signature. The call checks the account holds the role, then requires its authorization: a caller without the role fails with OpenZeppelin's 2000 `Unauthorized`; a missing signature fails as `Error(Auth, InvalidAction)`. The sale contract satisfies the authorization as the direct cross-contract caller, with no authorization entry, so the investor's authorization covers only the purchase and not this hop.

| Call | Authorized by | Effect |
|---|---|---|
| `set_allocations(entries, caller)` | `caller` holding `controller` | For each `{investor, max, enabled}`, at most 30: writes the cap and the flag, keeps usage. `max` must be non-negative (6102). Duplicates in one list: last wins. An empty list succeeds as a no-op. Emits `allocation_set` per entry. |
| `consume(investor, amount, caller) → period` | `caller` holding `controller`, normally the sale contract as the direct caller | Uses `amount` of the investor's room in the current period and returns that period. `amount` must be positive (6102), the investor enabled (6100) and `amount ≤ max − usage` (6101). Emits `allocation_consumed`. |
| `restore(investor, entries, caller)` | `caller` holding `controller`, normally the sale contract on refund or cooling-off | Gives room back per period: each `{period, amount}` names a period `consume` returned and the amount to return to it; 1 to 30 entries (6102 when empty, 6104 above 30), each `amount` positive (6102) and each `period` at or before the current one (6106). Entries are summed per period with checked arithmetic (overflow is 6102); a period whose usage is below its sum fails the whole call with 6103. No `enabled` check: a disabled investor can still be refunded. Emits one `allocation_restored` per period touched, in the order the periods first appear. |
| `grant_role(account, role, caller)` | the admin, or `caller` holding the role's admin role (a controller, for `controller`) | OpenZeppelin. Granting a role already held succeeds silently, with no event. |
| `revoke_role(account, role, caller)` | the admin, or `caller` holding the role's admin role (a controller, for `controller`, as Base's `setControllerFromController(…, false)`) | OpenZeppelin. Revoking a role the account does not hold fails with 2007. |
| `renounce_role(role, caller)` | `caller` | OpenZeppelin: the caller drops its own role. |
| `set_role_admin(role, admin_role)` | the admin (its signature, no `caller` argument) | OpenZeppelin: sets which role manages `role`. The containment step above. |
| `transfer_admin_role(new_admin, live_until_ledger)`, `accept_admin_transfer()` | the admin, then the pending admin | OpenZeppelin's two-step admin transfer with a deadline. |
| `renounce_admin()` | nobody | **Always fails with 6105.** |
| `upgrade(new_wasm_hash, operator)` | `operator` = the admin | Replaces the code once the call succeeds (2000 for anyone else). Storage is kept and the constructor does not run again. There is no `migrate` in this version. |

Read functions, the backend's read path (Base's app reads raw storage slots; here the views are the interface):

| Call | Returns | Base equivalent |
|---|---|---|
| `allocation(investor)` | `{consumed, enabled, max, period}`: the stored cap and flag, the current period and its usage. One simulation for the investor page. | none: Base reads storage slots |
| `usage(investor, period)` | That period's usage, any period, 0 when absent. For support audits of past months. | none |
| `remaining(investor)` | The room: `max − usage(now)` when enabled, else 0, never negative | `remainingAllocation` |
| `can_invest(investor, amount)` | Whether `consume`'s amount, eligibility and room checks would pass at this ledger time. False for `amount ≤ 0`. It cannot see the caller's role. | `canPurchase` |
| `current_period()` | The period of the current ledger time | none |
| `schema_version()` | 1 | `version()` |
| `has_role(account, role)` | The role index when held, void otherwise. The backend tests for non-void. | `controllers(account)` |
| `get_role_member_count(role)`, `get_role_member(role, index)`, `get_role_admin(role)`, `get_admin()`, `get_existing_roles()` | OpenZeppelin's enumeration | none |

Reads extend nothing, so simulation pays no rent. `has_role` is the exception OpenZeppelin makes on its own: it extends the role entry it reads, to 90 days.

The code version is in the WASM metadata as `binver` (SEP-46, SEP-49): `stellar contract info meta`.

## Differences from Base

- **No reset.** Base runs `resetConsumed` from a monthly cron; here a new month starts from zero by itself, and the only monthly job is the cap rewrite. There is no reset call at all.
- **Usage per month, and refunds per month.** Base keeps one running `consumed`; a refund after the reset reverts and takes its whole batch with it, and the backend skips investors with open positions to avoid it (bwb-tokenization #1235). Here `restore` names each month and the amount to give back to it, so a refund across a month boundary succeeds in one call and a refund of last month's money never frees this month's room.
- **`consume` rejects 0** (6102). Base accepts it; its sale contract never sends it.
- **`can_invest(…, 0)` is false.** Base's `canPurchase` returns true for 0.
- **The caller is an explicit last argument**, as on the token. Base reads `msg.sender`.
- **Roles are OpenZeppelin's:** granting a held role is a silent success (Base always emitted `ControllerUpdated`), revoking a role not held fails with 2007, the admin transfer is two-step with a deadline (Base's `transferOwnership` is one step), and a role's admin role can be moved. The admin can revoke any controller, as Base's owner could.
- **Getters are the read path.** `allocation`, `usage`, `remaining`, `can_invest`, `current_period` and the role enumeration. Base's app reads storage slots and only reads `controllers` once, at setup.
- **No removal of allocations**, as in Base: disable with `enabled = false`, which keeps the history.
- **No pause.** The platform disables investors or revokes a sale contract's `controller` instead.

## Events

Data fields are listed in sorted name order, which is how the SDK encodes a `contractevent` map.

| Event | Topics | Data | When |
|---|---|---|---|
| `initialized` | `initialized` | `{admin, controllers}` | Once, at construction, after the role events. |
| `allocation_set` | `allocation_set`, investor | `{caller, enabled, max}` | `set_allocations`, per entry. The backend confirms its writes landed from it. |
| `allocation_consumed` | `allocation_consumed`, investor, caller | `{amount, consumed, period}` | `consume`. `consumed` is the period's usage after the call; `period` is the one returned. `caller` identifies the offering. |
| `allocation_restored` | `allocation_restored`, investor, caller | `{amount, consumed, period}` | `restore`, one per period touched. `amount` is the sum restored to that period, `consumed` its usage after the call. |
| `upgraded` | `upgraded` | `{from_schema_version, new_wasm_hash}` | `upgrade`. The old code emits it, so `from_schema_version` is the schema version before the upgrade. The host also emits its own `executable_update`. |

An indexer can rebuild usage per investor per period from `allocation_consumed` and `allocation_restored` alone, since each carries the period and the usage after.

Inherited from OpenZeppelin, shapes in [interface R4](interface.md#r4-inherited-event-shapes): `role_granted` and `role_revoked` (topics `role`, `account`; data `{caller}`), `role_admin_changed` (topic `role`; data `{new_admin_role, previous_admin_role}`, where the previous role is the empty symbol at construction), `admin_transfer_initiated`, `admin_transfer_completed`.

**Sizes.** Measured XDR encodings: `allocation_set` 240 B, `allocation_consumed` 268 B, `allocation_restored` 268 B. `set_allocations` at 30 stays within the 16 KB per-transaction event limit (about 7.2 KB). The L1 estimate of 20 investors per `refund` is **invalidated**: `allocation_restored` gained `period` (20 B) and a refund can emit one event per month the investor bought in. L4 re-measures by simulation with the worst case of investors × months ([interface 6.4](interface.md#64-batch-sizes)).

## Errors

| Code | Name | When |
|---|---|---|
| 6100 | `InvestorNotEnabled` | `consume` for a disabled or unknown investor. **The eligibility rejection.** |
| 6101 | `AllocationExceeded` | `consume` beyond `max − usage` this period. **The allocation rejection.** |
| 6102 | `InvalidAmount` | A negative cap, a non-positive amount in `consume` or `restore`, an empty restore list, or a restore sum that overflows |
| 6103 | `RestoreExceedsUsage` | A period's usage cannot account for the sum restored to it: a double restore, a wrong amount, or an investor with no allocation |
| 6104 | `BatchTooLarge` | More than 30 entries in `set_allocations` or `restore` |
| 6105 | `RenounceBlocked` | `renounce_admin` |
| 6106 | `InvalidPeriod` | A restore period after the current one, or a ledger time whose period does not fit |

OpenZeppelin codes that surface unchanged: 2000 `Unauthorized` (a caller without the role; a non-admin operator on `upgrade`; a non-admin granting a role it does not administer), 2007 `RoleNotHeld` (revoking or renouncing a role not held), and on the admin transfer 2001 `AdminNotSet` and the role-transfer codes (`NoPendingTransfer`, `InvalidLiveUntilLedger`, `TransferExpired`).

A missing signature fails as `Error(Auth, InvalidAction)`, not as a contract code ([interface 7.1](interface.md#71-codes)).

## Storage and expiry

- Allocations: one persistent entry per investor. Never removed.
- Usage: one persistent entry per `(investor, period)`, written by `consume` and `restore` only.
- Roles: persistent, through OpenZeppelin (membership, member lists, counts, role-admins).
- The admin and the schema version: instance, through OpenZeppelin.

**Policy.** Every state-changing call tops the instance, and every entry it touches, up to the network maximum (about 180 days on mainnet) once it falls a month below it: `set_allocations` the allocations it writes; `consume` the allocation and the current period's usage; `restore` the usage entries it writes. Reads extend nothing.

**Roles.** OpenZeppelin extends a role entry only to 90 days when it reads it. Every controller call additionally tops the **caller's own** `controller` entry up to the network maximum, so a sale contract that is quiet for months (Successful, then a late cooling-off; Failed, then late refunds) still finds its role alive, and the hot key's monthly cap rewrite keeps its own. The other OpenZeppelin keys (member lists, counts, role-admins, other accounts' memberships) stay on OpenZeppelin's 90-day rule; they are read by the role management calls, so routine operation keeps them alive, and anyone may extend any entry without permission.

**Old months expire on their own.** A usage entry is never written again once its month is over, so it archives at no cost and storage does not grow forever. Nobody deletes anything.

**Archived entries are not gone.** Since protocol 23 (CAP-0066) the call that touches an archived persistent entry restores it: simulation lists the key in the transaction's read-write footprint and the transaction pays the restoration rent, after which the allowlist tops the entry up to the maximum like any other. A late refund of a month older than the network maximum therefore works, at the cost of that restoration, provided the submitter simulates the call afresh and submits what simulation returned rather than a footprint built from an earlier run. A simulated read of an archived month (`usage`, `allocation`) returns its value and marks the entry for restoration; nothing is restored until a transaction is submitted.

## Tests

`scripts/test.sh` builds the upgrade fixtures and runs the tests; CI runs the same script. Two seams: the contract client in the Soroban test environment, with the ledger time pinned per test and a test-only sale stub contract that calls `consume` and `restore` as the direct caller; and the period module's pure arithmetic. The modules follow the D2 vocabulary:

| Module | Covers |
|---|---|
| `authorization` | The admin grants and revokes any role; a controller grants and revokes `controller` and nothing else; a non-controller cannot write, consume or restore (2000); an unsigned call fails at authorization; the sale stub consumes and restores as the direct caller with no authorization entry, and cannot without the role; only the admin upgrades (2000); `renounce_admin` fails (6105); the two-step admin transfer; the admin re-parents `controller`'s role-admin, the hot key can no longer grant, and the admin cleans up |
| `boundary` | Exact cap, cap + 1, cap 0, cap at the top of `i128`; batches of 30 and 31 for both lists; 23:59:59 against 00:00:00 UTC; December to January; the grill scenario (buy in September and October, restore both in October); last month's refund never frees this month's room; restore of the exact usage and one above (6103); a list summed per period; a period equal to the current one and one after it (6106); a cap lowered below the usage; a restore sum that overflows (6102) |
| `idempotency` | The same `set_allocations` twice keeps the usage; an empty batch; last duplicate wins; disabling keeps cap and usage; granting a held role is silent; `remaining` and `can_invest` agree with `consume` at a pinned time |
| `failure` | Every 61xx code reached at least once (6104 and 6105 here as well as in the modules that own the rule), 2007 on revoking a role not held, 6103 for an address with no allocation and for a double restore, and a failed restore reverting every period in the call |
| `events` | Exact events and order for construction (with and without controllers), each write, consume (with the returned period), a restore spanning two months (two events, first-seen order), the OpenZeppelin role and admin-transfer events; the measured event sizes the interface document quotes (`upgraded` is asserted in `upgrade`, which has the second binary) |
| `upgrade` | Swap to `offer-allowlist-upgrade-fixture`, a test-only second binary, with the exact `upgraded` event; allocations, usage, roles, role-admin, admin and `schema_version` survive; the upgraded code can upgrade again under the same gate; a hash never uploaded fails; a non-admin cannot swap |
| `ttl` | The instance, an allocation, a usage entry and the caller's role entry are extended on write, the role beyond OpenZeppelin's 90 days; reads extend nothing; an untouched old month is left to expire; a late refund of an archived month restores the entry (the test environment's recording mode) and tops it up |
| `period` | The date arithmetic against known timestamps, including the `u32` bound (inside the module), and the contract's getter at pinned ledger times (through the client), including 6106 |
