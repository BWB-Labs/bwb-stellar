# offer-allowlist

Who may invest and up to how much. One global instance, shared by every offering. It is a port of Base's `OfferSaleAllowlist`. The allowlist admits addresses, never assets. A cap is global, not per offering, by design.

**Draft (interface v0.2).** The contract lands in L3, which updates this document. The cross-cutting rules are in [interface.md](interface.md).

## Constructor

| Argument | Meaning |
|---|---|
| `admin` | Platform multisig: the OpenZeppelin access-control admin, and `upgrader` |
| `controllers` | Initial controllers, normally the platform hot key |

## Allocation

One persistent entry per investor:

| Field | Meaning |
|---|---|
| `schema` | Struct version |
| `max` | Cap, in USDC stroops. 0 is valid and means no room, as in Base. |
| `consumed` | Amount already used, in USDC stroops |
| `enabled` | Whether the investor may invest at all |

An investor with no entry reads as `{max: 0, consumed: 0, enabled: false}`. The backend's "already on the allowlist" check relies on `max > 0`, as Base's does.

When an allocation is written, the backend converts the investor's BRL limit into stroops ([interface 3](interface.md#3-premises)).

## Calls

Writes take the caller as an argument and require it to hold `controller`. OpenZeppelin's role functions take it last.

| Call | Authorized by | Effect |
|---|---|---|
| `set_allocations(caller, entries)` | `controller` | For each `(investor, max, enabled)`, at most 30: sets cap and status and keeps `consumed`. Repeating it is safe. |
| `reset_consumed(caller, investors)` | `controller` | Sets `consumed` to 0 for each investor, at most 30. Used monthly. An investor with no entry is skipped, and no entry is created. |
| `consume(caller, investor, amount)` | `controller`, normally the offering's sale as the direct caller | Adds `amount` to `consumed`. Fails if the investor isn't enabled or the cap would be exceeded. |
| `restore(caller, investor, amount)` | `controller`, normally the sale on refund or cooling-off | Subtracts `amount` from `consumed`. Fails if `amount` is larger than `consumed`, as in Base. See the known issue below. |
| `grant_role(account, role, caller)` / `revoke_role(account, role, caller)` | `admin` for any role. A `controller` may grant **and revoke** `controller`, as Base's `setControllerFromController`. | Role management (OpenZeppelin) |
| `upgrade(new_wasm_hash, operator)` | `operator`, holding `upgrader` | Replaces the code. |

OpenZeppelin differs from Base in two small ways:
- revoking a role the account doesn't hold fails with 2007 `RoleNotHeld`;
- granting a role already held succeeds silently, with no event. Base always emitted `ControllerUpdated`.

**Known issue carried from Base: refunds after the monthly reset.**
- Every refund restores the investor's whole payment in that offering. The monthly reset sets `consumed` to 0.
- So a refund requested after a reset fails, both on cooling-off and on refunds after failure. One such investor fails a whole refund batch.
- If the investor bought again after the reset, the refund either fails or takes last month's amount off this month's usage.
- In Base it is a real but unreachable bug: the app never calls refunds. It was introduced when refunds started restoring allocations, after the reset existed, and is in no audit, test or document.
- The port keeps parity for now. The likely fix is to restore at most `consumed` and report the amount actually restored. It is an [open point](interface.md#11-open-points), decided in L3.

Read functions (Base has `remainingAllocation`, `canPurchase` and `controllers`, and the app calls them):

| Call | Returns | Base equivalent |
|---|---|---|
| `allocation(investor)` | The allocation, or the zero allocation | none: Base reads storage slots |
| `remaining(investor)` | `max − consumed`; 0 when not enabled **or when `consumed ≥ max`**, for example after a cap was lowered | `remainingAllocation` |
| `can_invest(investor, amount)` | Whether `consume` would succeed | `canPurchase` |
| `has_role(account, "controller")` | Whether the account is a controller | `controllers(account)` |

`caller` is the sale contract itself when a purchase consumes room. The allowlist authorizes it as the direct caller, so the investor's authorization entry doesn't include this call.

**Recalculating caps when the exchange rate moves** means a `set_allocations` over every allowlisted investor, at most 30 per call. Each entry must carry the investor's current `enabled`, so the backend reads before writing, as Base's provider does.

## Events

| Event | Topics | Data | When |
|---|---|---|---|
| `allocation_set` | `allocation_set`, investor | `{caller, enabled, max}` | `set_allocations`, per entry |
| `allocation_consumed` | `allocation_consumed`, investor, caller | `{amount, consumed}` | `consume`. `caller` identifies the offering. |
| `allocation_restored` | `allocation_restored`, investor, caller | `{amount, consumed}` | `restore` |
| `consumed_reset` | `consumed_reset`, investor | `{caller, previous}` | `reset_consumed`, per investor with an entry. Base emits nothing here. |
| `upgraded` | `upgraded` | `{new_wasm_hash, schema_version}` | `upgrade` |

Inherited from OpenZeppelin: `role_granted`, `role_revoked`, `role_admin_changed`, plus the access-control admin handover events.

## Errors

| Code | Name | When |
|---|---|---|
| 6100 | `InvestorNotEnabled` | `consume` for an investor without an enabled allocation. **The eligibility rejection.** |
| 6101 | `AllocationExceeded` | `consume` beyond `max − consumed`. **The allocation rejection.** |
| 6102 | `NegativeAmount` | A negative cap or amount |
| 6103 | `RestoreExceedsConsumed` | `restore` larger than `consumed` |
| 6104 | `BatchTooLarge` | More than 30 entries |

OpenZeppelin codes that can surface: 2000 `Unauthorized`, and 2007 `RoleNotHeld`.

## Storage

- One persistent entry per investor, extended on every write. A read through simulation extends nothing.
- Roles: persistent, through OpenZeppelin.
- Admin and schema version: instance.
