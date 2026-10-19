# offer-allowlist

- **What it does.** Records who may invest and up to how much. Each investor has an allocation: a cap in USDC stroops and whether they may invest. Usage is counted per investor per UTC calendar month (the *period*), inside the contract, so a new month starts from zero by itself. The allowlist admits addresses, never assets.
- **Scope.** One global instance, shared by every offering. A cap is global, not per offering, by design.
- **Who uses it.** The platform hot key, as `allocator`, writes caps after KYC and when compliance changes them, and wires each new offering's sale contract in as `sale`. Each sale contract consumes room on a purchase, which leaves a *receipt*, and restores room on a refund by naming receipts. The platform multisig is the access-control admin and the only account that upgrades.
- **Where it comes from.** A port of Base's `OfferSaleAllowlist`, with two changes that remove Base's monthly reset and its refund-after-reset failure (ADR 0007), per-purchase receipts so that a refund gives back only what its own sale consumed, once, and with OpenZeppelin's roles in place of Base's owner and controller map, split in two so that a sale contract can only consume and restore (ADR 0008; Base audit NF-01).

**Delivered in L3 (interface v0.8: receipts).** Testnet deployment: [`deployments/testnet.json`](../deployments/testnet.json). The cross-cutting rules are in [interface.md](interface.md).

## Periods

Usage is counted per **period**, the UTC calendar month the ledger time falls in. A period is named by one number:

```
period = year * 12 + (month - 1)        year / month in UTC, month 1–12
year   = period / 12
month  = period % 12 + 1
```

October 2026 is `2026 * 12 + 9 = 24321`. The limit renews at 00:00 UTC on the first of the month, 21:00 in Brasília, as the off-chain gate already does. The monthly limit is BWB's product choice, not CVM 88 (annual, enforced off-chain), so the period is a code constant and changing it is an upgrade.

**Only the allowlist computes periods.** Each receipt records the period its purchase was charged to, and `restore` gives the amount back to that period, so the sale contract never handles a period. The backend reads `current_period()` and uses the formula above to label events. A ledger time whose period does not fit a `u32` fails with 6106 `InvalidPeriod`, which no real ledger time reaches (year 357,913,941).

**Around midnight UTC on the last day of a month:**

- A purchase simulated before 00:00 and included in a ledger after it writes a different usage entry than the simulation declared, so the transaction fails with a footprint error rather than a contract code. The backend re-simulates and resubmits; the investor's authorization is unaffected because the call's arguments did not change.
- An investor can use a full cap in the last minutes of a month and a full cap again in the first minutes of the next: up to twice the cap in one short window. ADR 0007 accepts this; it is the same with Base's reset. Compliance should know.

## Roles

OpenZeppelin `AccessControl` (ADR 0008). Base's owner becomes the admin. Base's single `controllers` map, which gave the hot key and every sale contract the same power, becomes two roles:

| Role | Held by | Can | Granted and revoked by |
|---|---|---|---|
| admin | platform multisig | grant and revoke any role, set a role's admin role, start and complete the two-step admin transfer, **upgrade** | the two-step transfer |
| `allocator` | platform hot key | write allocations (`set_allocations`); grant and revoke `sale` | the admin only: `allocator` has no role-admin |
| `sale` | each offering's sale contract | consume and restore room (`consume`, `restore`); nothing else | an allocator, or the admin |

`allocator` is the role-admin of `sale`, set at construction, so the backend wires the next offering's sale contract in without a multisig signature, as it does on Base with `setControllerFromController`. Unlike Base, a sale contract holds no power over allocations or over other sales: the "Controller Privilege Escalation" finding of the Base audit (NF-01, accepted there because the backend must wire sales itself) is closed by the split. An allocator cannot add another allocator. The admin can always grant and revoke any role regardless of the role-admin.

**The admin is the trust root.** It upgrades, and a code swap can do anything. `renounce_admin` always fails with 6105 `RenounceBlocked`: an admin-less allowlist could never be upgraded. Rotating the multisig is OpenZeppelin's two-step transfer: `transfer_admin_role(new_admin, live_until_ledger)` signed by the admin, then `accept_admin_transfer()` signed by the new admin before the deadline.

**What a leaked hot key can do** until the multisig acts: write any allocation (enable anyone, set any cap, disable real investors), revoke `sale` from any sale contract (its purchases and refunds then fail with 2000 until the role is granted again), and grant `sale` to accomplice contracts, which can then consume and restore for any investor. It cannot grant `allocator`, upgrade, or touch the admin.

**Containing a leaked hot key.** The admin, with one multisig transaction per step:

1. Optionally first, `set_role_admin("sale", "nobody")`: freezes every grant and revoke of `sale` by allocators while the cleanup runs. Undo it at the end with `set_role_admin("sale", "allocator")`.
2. `revoke_role(leaked_key, "allocator", admin)`, then `grant_role(new_key, "allocator", admin)` for the replacement hot key. From here on the leaked key writes nothing.
3. Enumerate `sale` with `get_role_member_count("sale")` and `get_role_member("sale", i)`, compare against the sale contracts in the deployments manifest, `revoke_role(unknown, "sale", admin)` for each one that is not there, and `grant_role(sale_contract, "sale", admin)` for each legitimate sale contract the leaked key revoked. Re-granting a sale contract restores its refunds; what the leaked key consumed or restored in the meantime is read back from the events and corrected with `set_allocations` where a cap matters.
4. If the damage cannot be undone by role changes, an upgrade by the admin is the full stop: it can rewrite any entry.

**Revoking a sale contract's `sale`** makes its purchases and refunds fail with 2000 until the role is granted again. That is the platform's way to stop one offering without a pause on the allowlist; it stops the offering's refunds too.

## Constructor

| Argument | Meaning |
|---|---|
| `admin` | Platform multisig: the access-control admin, and the only upgrader. |
| `allocators` | Initial allocators, normally the platform hot key. Each offering's deploy batch grants `sale` to its sale contract later, signed by an allocator. |

It sets the admin, makes `allocator` the role-admin of `sale`, grants `allocator` to each address, and writes `schema_version = 1`. It emits, in this order: `role_admin_changed` for `sale` (with an empty previous role symbol and `allocator` as the new one), one `role_granted` per allocator (with the admin as caller), then `initialized`.

## Allocation and usage

One persistent **allocation** per investor:

| Field | Meaning |
|---|---|
| `schema` | Struct version, 1 |
| `max` | The cap, in USDC stroops: the most the investor may have in usage within one period. 0 is valid and means no room, as in Base. |
| `enabled` | Whether the investor may invest at all |

One persistent **usage** entry per `(investor, period)`, a plain `i128`. Absent means 0. A purchase adds to the current period's entry; a refund subtracts from the period the money was consumed in. Writing the allocation never touches usage, so a cap change never gives or takes room already used this month.

An investor with no entry reads as `max = 0`, not enabled, usage 0. The backend's "already on the allowlist" check relies on `max > 0`, as Base's does.

**Room** is `max − usage(current period)` when the investor is enabled, else 0, and never negative: a cap lowered below this month's usage leaves no room until next month, and keeps the usage.

## Receipts

Every purchase leaves a **receipt**: the allowlist's record of that purchase, kept under the sale contract that made it.

| Field | Meaning |
|---|---|
| `amount` | What the purchase used, in USDC stroops |
| `investor` | Whose usage it went to |
| `period` | The period it was charged to |

`consume` writes the receipt and returns its **number**. Numbers count from 0 per sale contract (two offerings both have a receipt 0) and are never reused, not even after a restore. The sale contract keeps the number with the purchase.

A refund names receipts. `restore(receipts, caller)` looks each number up under the calling sale contract, subtracts its amount from its own investor's usage in its own period, and deletes it. So:

- **each purchase is given back once.** A second restore of the same number finds nothing and fails with 6103, whatever other usage the investor has that month;
- **a sale contract only gives back its own purchases.** It cannot name another sale contract's receipts: numbers are looked up under the caller. An accomplice granted `sale` by a leaked hot key holds no receipts, so it cannot free anyone's room;
- **receipts are whole.** There is no partial restore of a purchase;
- one call may name receipts of different investors and different months, at most 20.

Usage is always the sum of its period's live receipts. A restore that brings a usage entry to 0 removes the entry.

**When caps are written.** At KYC approval and whenever compliance changes an investor's limit or status, as on Base. There is no periodic rewrite and no reset: the only job the backend runs against the allowlist is the one it already runs on Base. Whether the cap is the BRL limit converted by the backend or a separate dollar limit is pending the team ([interface 3](interface.md#3-premises), [interface 11](interface.md#11-open-points)). Because `set_allocations` writes both `max` and `enabled`, **build every write from a fresh read**: an entry built from a read taken before compliance disabled the investor re-enables them.

## Calls

Every write that names its acting account takes it as the **last** argument (`caller`, or `operator` on `upgrade`), as the token's calls and OpenZeppelin's role functions do; OpenZeppelin's admin-only calls (`set_role_admin`, `transfer_admin_role`) take none and require the stored admin's signature. The call checks the account holds the role, then requires its authorization: a caller without the role fails with OpenZeppelin's 2000 `Unauthorized`; a missing signature fails as `Error(Auth, InvalidAction)`. The sale contract satisfies the authorization as the direct cross-contract caller, with no authorization entry, so the investor's authorization covers only the purchase and not this hop.

| Call | Authorized by | Effect |
|---|---|---|
| `set_allocations(entries, caller)` | `caller` holding `allocator` | For each `{investor, max, enabled}`, at most 30: writes the cap and the flag, keeps usage. `max` must be non-negative (6102). Duplicates in one list: last wins. An empty list succeeds as a no-op. Emits `allocation_set` per entry. |
| `consume(investor, amount, caller) → receipt` | `caller` holding `sale`: the sale contract as the direct caller | Uses `amount` of the investor's room in the current period, writes a receipt under `caller` and returns its number ([Receipts](#receipts)). `amount` must be positive (6102), the investor enabled (6100) and `amount ≤ max − usage` (6101). Emits `allocation_consumed`. |
| `restore(receipts, caller)` | `caller` holding `sale`: the sale contract on refund or cooling-off | Gives back whole purchases: 1 to 20 receipt numbers (6102 when empty, 6104 above 20), which may belong to different investors. For each number in list order: the receipt is looked up under `caller`; a number that is unknown, already restored, another sale contract's, or repeated in the list fails the whole call with 6103. Its amount is subtracted from its investor's usage in its period (the entry is removed at 0) and the receipt is deleted. No `enabled` check: a disabled investor can still be refunded. Emits one `allocation_restored` per receipt, in list order. |
| `grant_role(account, role, caller)` | the admin for any role; `caller` holding `allocator` for `sale` | OpenZeppelin, plus the granted entry topped up to the network maximum. Granting a role already held succeeds silently, with no event. |
| `revoke_role(account, role, caller)` | the admin for any role; `caller` holding `allocator` for `sale`, as Base's `setControllerFromController(…, false)` | OpenZeppelin. Revoking a role the account does not hold fails with 2007. |
| `renounce_role(role, caller)` | `caller` | OpenZeppelin: the caller drops its own role. |
| `set_role_admin(role, admin_role)` | the admin (its signature, no `caller` argument) | OpenZeppelin: sets which role manages `role`. The optional freeze in the containment runbook. |
| `transfer_admin_role(new_admin, live_until_ledger)`, `accept_admin_transfer()` | the admin, then the pending admin | OpenZeppelin's two-step admin transfer with a deadline. |
| `renounce_admin()` | nobody | **Always fails with 6105.** |
| `upgrade(new_wasm_hash, operator)` | `operator` = the admin | Replaces the code once the call succeeds (2000 for anyone else). Storage is kept and the constructor does not run again. There is no `migrate` in this version. |

Read functions, the backend's read path (Base's app reads raw storage slots; here the views are the interface):

| Call | Returns | Base equivalent |
|---|---|---|
| `allocation(investor)` | `{consumed, enabled, max, period}`: the stored cap and flag, the current period and its usage. One simulation for the investor page. | none: Base reads storage slots |
| `usage(investor, period)` | That period's usage, any period, 0 when absent. For support audits of past months. | none |
| `receipt(sale, receipt)` | `{amount, investor, period}` of that sale contract's receipt, or void once restored and for a number never issued | none |
| `remaining(investor)` | The room: `max − usage(now)` when enabled, else 0, never negative | `remainingAllocation` |
| `can_invest(investor, amount)` | Whether `consume`'s amount, eligibility and room checks would pass at this ledger time. False for `amount ≤ 0`. It cannot see the caller's role. | `canPurchase` |
| `current_period()` | The period of the current ledger time | none |
| `schema_version()` | 1 | `version()` |
| `has_role(account, role)` | The role index when held, void otherwise. The backend tests for non-void. | `controllers(account)` |
| `get_role_member_count(role)`, `get_role_member(role, index)`, `get_role_admin(role)`, `get_admin()`, `get_existing_roles()` | OpenZeppelin's enumeration | none |

The allowlist's own reads (`allocation`, `usage`, `receipt`, `remaining`, `can_invest`, `current_period`) extend nothing, so simulating them pays no rent. OpenZeppelin's role getters are not rent-free: `has_role`, `get_role_admin`, `get_role_member_count`, `get_role_member` and `get_existing_roles` each extend the role entry they read to 90 days when submitted (a simulation alone changes nothing).

The code version is in the WASM metadata as `binver` (SEP-46, SEP-49): `stellar contract info meta`.

## Differences from Base

- **No reset.** Base runs `resetConsumed` from a monthly cron; here a new month starts from zero by itself. There is no reset call and no periodic job at all.
- **Usage per month, refunds per purchase.** Base keeps one running `consumed`; a refund after the reset reverts and takes its whole batch with it, and the backend skips investors with open positions to avoid it (bwb-tokenization #1235). Here each purchase leaves a receipt with its period, and `restore` names receipts, so a refund across a month boundary succeeds in one call and a refund of last month's money never frees this month's room.
- **A refund can't over-restore.** Base's `restoreAllocation` takes an amount and only checks it against the running total, so one sale contract restoring twice, or more than it consumed, passes whenever another purchase covers it. Here a receipt is given back once, and only by the sale contract that made it.
- **Two roles instead of one controller map.** On Base every controller, the hot key and each sale contract alike, could write allocations, consume, restore and add or remove controllers (audit NF-01, accepted). Here the hot key is `allocator` and a sale contract is `sale`: it consumes and restores and nothing else. The backend still wires sales itself, because `allocator` administers `sale`.
- **`consume` rejects 0** (6102). Base accepts it; its sale contract never sends it.
- **`can_invest(…, 0)` is false.** Base's `canPurchase` returns true for 0.
- **The caller is an explicit last argument**, as on the token. Base reads `msg.sender`.
- **Roles are OpenZeppelin's:** granting a held role is a silent success (Base always emitted `ControllerUpdated`), revoking a role not held fails with 2007, the admin transfer is two-step with a deadline (Base's `transferOwnership` is one step), and a role's admin role can be moved. The admin can revoke any allocator or sale, as Base's owner could.
- **Getters are the read path.** `allocation`, `usage`, `remaining`, `can_invest`, `current_period` and the role enumeration. Base's app reads storage slots and only reads `controllers` once, at setup.
- **No removal of allocations**, as in Base: disable with `enabled = false`, which keeps the history.
- **No pause.** The platform disables investors or revokes a sale contract's `sale` instead.

## Events

Data fields are listed in sorted name order, which is how the SDK encodes a `contractevent` map.

| Event | Topics | Data | When |
|---|---|---|---|
| `initialized` | `initialized` | `{admin, allocators}` | Once, at construction, after the role events. |
| `allocation_set` | `allocation_set`, investor | `{caller, enabled, max}` | `set_allocations`, per entry. The backend confirms its writes landed from it. |
| `allocation_consumed` | `allocation_consumed`, investor, caller | `{amount, consumed, period, receipt}` | `consume`. `consumed` is the period's usage after the call; `receipt` is the number returned. `caller` identifies the offering. |
| `allocation_restored` | `allocation_restored`, investor, caller | `{amount, consumed, period, receipt}` | `restore`, one per receipt, in list order. `amount` and `period` are the receipt's, `consumed` the period's usage right after it. |
| `upgraded` | `upgraded` | `{from_schema_version, new_wasm_hash}` | `upgrade`. The old code emits it, so `from_schema_version` is the schema version before the upgrade. The host also emits its own `executable_update`. |

An indexer can rebuild usage per investor per period, and the open receipts of each sale contract (`caller`, `receipt`), from `allocation_consumed` and `allocation_restored` alone, since each carries the period, the usage after and the receipt number.

Inherited from OpenZeppelin, shapes in [interface R4](interface.md#r4-inherited-event-shapes): `role_granted` and `role_revoked` (topics `role`, `account`; data `{caller}`), `role_admin_changed` (topic `role`; data `{new_admin_role, previous_admin_role}`, where the previous role is the empty symbol at construction), `admin_transfer_initiated`, `admin_transfer_completed`.

**Sizes and the batch limits.** Measured XDR encodings: `allocation_set` 240 B, `allocation_consumed` 292 B, `allocation_restored` 292 B. At 30 entries, `set_allocations` emits about 7.2 KB of the 16 KB event limit; what binds it is the transaction's **write entries** (31 of 50: the entries plus the instance) and its footprint (about 35 of 100). `restore` writes up to two entries per receipt (the receipt deleted, its usage entry written or removed), so its worst case, 20 receipts in 20 different months, writes **40 of the 50 write entries**, reads about 45 footprint entries of 100 and emits 5,840 B of events, measured on the real WASM. That leaves 10 write entries to the calling sale contract's own refund (its own entries, the USDC balances, an authorization nonce), which sizes L4's refund batch ([interface 6.4](interface.md#64-batch-sizes)).

## Errors

| Code | Name | When |
|---|---|---|
| 6100 | `InvestorNotEnabled` | `consume` for a disabled or unknown investor. **The eligibility rejection.** |
| 6101 | `AllocationExceeded` | `consume` beyond `max − usage` this period. **The allocation rejection.** |
| 6102 | `InvalidAmount` | A negative cap, a non-positive amount in `consume`, or an empty restore list. Also two states the contract never reaches: a sale contract out of receipt numbers (`u32::MAX` purchases), and a receipt larger than its period's usage, which is always the sum of its live receipts |
| 6103 | `ReceiptNotFound` | `restore` names a receipt the calling sale contract doesn't hold: never issued, already restored (a double refund), another sale contract's, or repeated in the same list. The whole call reverts |
| 6104 | `BatchTooLarge` | More than 30 entries in `set_allocations`, or more than 20 receipts in `restore` |
| 6105 | `RenounceBlocked` | `renounce_admin` |
| 6106 | `InvalidPeriod` | A ledger time whose period does not fit a `u32`, which no real ledger time reaches |

OpenZeppelin codes that surface unchanged: 2000 `Unauthorized` (a caller without the role; a non-admin operator on `upgrade`; a caller granting or revoking a role it does not administer), 2007 `RoleNotHeld` (revoking or renouncing a role not held), and on the admin transfer 2001 `AdminNotSet` and the role-transfer codes (`NoPendingTransfer`, `InvalidLiveUntilLedger`, `TransferExpired`).

A missing signature fails as `Error(Auth, InvalidAction)`, not as a contract code ([interface 7.1](interface.md#71-codes)).

## Storage and expiry

- Allocations: one persistent entry per investor. Never removed.
- Usage: one persistent entry per `(investor, period)`, written by `consume` and `restore` only, removed when a restore brings it to 0.
- Receipts: one persistent entry per purchase, `(sale contract, number)`, written by `consume` and removed by `restore`.
- Receipt counters: one persistent `u32` per sale contract, the number its next purchase gets.
- Roles: persistent, through OpenZeppelin (membership, member lists, counts, role-admins).
- The admin and the schema version: instance, through OpenZeppelin.

**Policy.** Every state-changing call tops the instance, and every entry it touches, up to the network maximum (about 180 days on mainnet) once it falls a month below it: `set_allocations` the allocations it writes; `consume` the allocation, the current period's usage, the new receipt and the sale contract's counter; `restore` the usage entries it writes (a removed entry needs nothing); `grant_role` the membership it grants. The allowlist's own reads extend nothing.

**Roles.** OpenZeppelin extends a role entry only to 90 days when it reads it. The allowlist goes further for the memberships that matter: every `set_allocations` tops the **caller's own** `allocator` entry up to the network maximum, every `consume` and `restore` the caller's own `sale` entry, and every `grant_role` the entry it grants, so a sale contract wired in today and then quiet for months (Successful, then a late cooling-off; Failed, then late refunds) still finds its role alive. The other OpenZeppelin keys (member lists, counts, role-admins) stay on OpenZeppelin's 90-day rule; the role calls and getters read them, which keeps them alive in routine operation, and anyone may extend any entry without permission.

**Old months and old receipts expire on their own.** A usage entry is written after its month only by a refund, and a receipt is never written again, so a month nobody refunds and a receipt nobody restores archive at no cost and storage does not grow forever. Nobody needs to delete anything; a restore deletes the receipts it gives back.

**Archived entries are not gone.** Since protocol 23 (CAP-0066) the call that touches an archived persistent entry restores it: simulation lists the key in the transaction's read-write footprint and the transaction pays the restoration rent, after which the allowlist tops the entry up to the maximum like any other. A late refund of a receipt or a month older than the network maximum therefore works, at the cost of that restoration, provided the submitter simulates the call afresh and submits what simulation returned rather than a footprint built from an earlier run. A simulated read of an archived entry (`usage`, `receipt`, `allocation`) returns its value and marks the entry for restoration; nothing is restored until a transaction is submitted.

## Tests

`scripts/test.sh` builds the upgrade fixtures and runs the tests; CI runs the same script. Two seams: the contract client in the Soroban test environment, with the ledger time pinned per test and a test-only sale stub contract, granted `sale`, that calls `consume` and `restore` as the direct caller; and the period module's pure arithmetic. The modules follow the D2 vocabulary:

| Module | Covers |
|---|---|
| `authorization` | The admin grants and revokes any role, `allocator` included; an allocator grants and revokes `sale` and nothing else, and cannot consume or restore; a sale cannot write allocations, grant or revoke; a stranger cannot do any of it (2000); an unsigned call fails at authorization; the sale stub consumes and restores as the direct caller with no authorization entry, an unwired stub cannot, and nobody can act in a sale's name from outside; only the admin upgrades (2000); `renounce_admin` fails (6105); the two-step admin transfer; the containment runbook step by step against what a leaked hot key did first, including an accomplice sale that cannot restore anyone's receipts |
| `boundary` | Exact cap, cap + 1, cap 0, cap at the top of `i128`; batches of 30 and 31 for `set_allocations` and of 20 and 21 receipts for `restore`; 23:59:59 against 00:00:00 UTC; December to January; receipt numbers from 0, per sale contract, never reused; the cooling-off case (buy, buy again 10 days later, restore only the second); one restore across investors; a usage entry removed at 0; the grill scenario (buy in September and October, restore both in October); last month's refund never frees this month's room; each receipt gives back exactly its amount; a cap lowered below the usage; a full restore (20 receipts in 20 months) uses 40 write entries and 5,840 B of events |
| `idempotency` | The same `set_allocations` twice keeps the usage; an empty batch; last duplicate wins; disabling keeps cap and usage; granting a held role is silent; `remaining` and `can_invest` agree with `consume` at a pinned time; the `receipt` getter before a purchase, after it and after its restore |
| `failure` | Every 61xx code reached at least once (6104 and 6105 here as well as in the modules that own the rule), 2007 on revoking a role not held; 6103 for an unknown receipt, a double restore, another sale contract's receipt and a number repeated in one list (which changes nothing); the audit's cross-sale over-restore, now rejected; a failed restore reverting every receipt in the call; 6102 for a sale out of receipt numbers and for a receipt its usage no longer covers (both unreachable, forced through storage) |
| `events` | Exact events and order for construction (with and without allocators), each write, consume (with the receipt), a restore of four receipts across investors and months (one event per receipt, list order, usage falling step by step), the OpenZeppelin role and admin-transfer events; the measured event sizes the interface document quotes (`upgraded` is asserted in `upgrade`, which has the second binary) |
| `upgrade` | Swap to `offer-allowlist-upgrade-fixture`, a test-only second binary, with the exact `upgraded` event; allocations, usage, a receipt and the receipt counter, both roles, the role-admin, admin and `schema_version` survive; the upgraded code can upgrade again under the same gate and still blocks `renounce_admin`; a hash never uploaded fails; a non-admin cannot swap |
| `ttl` | The instance, an allocation, a usage entry, a receipt, the receipt counter and the caller's role entry (`allocator` on a write, `sale` on consume and restore) are extended on write; a grant tops the granted entry up; the sale's role outlives OpenZeppelin's 90 days; the allowlist's reads extend nothing; an untouched old month and its receipt are left to expire; a late refund of an archived receipt and month restores the entries (the test environment's recording mode) and tops it up |
| `period` | The date arithmetic against known timestamps, including the `u32` bound (inside the module), and the contract's getter at pinned ledger times (through the client), including 6106 |
