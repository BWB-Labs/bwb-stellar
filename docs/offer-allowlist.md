# offer-allowlist

Who may invest and up to how much. One global instance, shared by every offering. It is a port of Base's `OfferSaleAllowlist`. The allowlist admits addresses, never assets. A cap is global, not per offering, by design.

**Draft (interface v0.1).** The contract lands in L3, which updates this document. The cross-cutting rules are in [interface.md](interface.md).

## Constructor

| Argument | Meaning |
|---|---|
| `admin` | Platform multisig. Grants and revokes roles. |
| `upgrader` | Platform multisig |
| `controllers` | Initial controllers, normally the platform hot key |

## Allocation

One persistent entry per investor:

| Field | Meaning |
|---|---|
| `schema` | Struct version |
| `max` | Cap, in USDC stroops |
| `consumed` | Amount already used, in USDC stroops |
| `enabled` | Whether the investor may invest at all |

Remaining room is `max − consumed`. When an allocation is written, the backend converts the investor's BRL limit into stroops ([interface 3](interface.md#3-premises)).

## Calls

All writes take the caller as an argument and require it to hold `controller`.

| Call | Authorized by | Effect |
|---|---|---|
| `set_allocations(caller, entries)` | `controller` | For each `(investor, max, enabled)`, at most 30: sets cap and status, keeps `consumed`. Repeating it is safe. |
| `reset_consumed(caller, investors)` | `controller` | Sets `consumed` to 0 for each investor, at most 30. Used monthly. |
| `consume(caller, investor, amount)` | `controller`, normally the offering's sale as the direct caller | Adds `amount` to `consumed`. Fails if the investor isn't enabled or the cap would be exceeded. |
| `restore(caller, investor, amount)` | `controller`, normally the sale on refund or cooling-off | Subtracts `amount` from `consumed`. |
| `grant_role(caller, account, role)` / `revoke_role(…)` | `admin`; `controller` may grant `controller` | Role management, from OpenZeppelin |
| `upgrade(new_wasm_hash, operator)` | `upgrader` | Replaces the code. |

Read functions (Base has none; it reads raw storage):

| Call | Returns |
|---|---|
| `allocation(investor)` | The allocation |
| `remaining(investor)` | `max − consumed`, or 0 when not enabled |
| `can_invest(investor, amount)` | Whether `consume` would succeed |

`caller` is the sale contract itself when a purchase consumes room. The allowlist authorizes it as the direct caller, so the investor's authorization entry doesn't include this call.

## Events

| Event | Topics | Data | When |
|---|---|---|---|
| `allocation_set` | `allocation_set`, investor | `{caller, enabled, max}` | `set_allocations`, per entry |
| `allocation_consumed` | `allocation_consumed`, investor, caller | `{amount, consumed}` | `consume`. `caller` identifies the offering. |
| `allocation_restored` | `allocation_restored`, investor, caller | `{amount, consumed}` | `restore` |
| `consumed_reset` | `consumed_reset`, investor | `{caller, previous}` | `reset_consumed`, per investor. Base emits nothing here. |
| `upgraded` | `upgraded` | `{new_wasm_hash, schema_version}` | `upgrade` |

Inherited from OpenZeppelin: `role_granted`, `role_revoked`, `role_admin_changed`, plus admin transfer events.

## Errors

| Code | Name | When |
|---|---|---|
| 6100 | `InvestorNotEnabled` | `consume` for an investor without an enabled allocation. **The eligibility rejection.** |
| 6101 | `AllocationExceeded` | `consume` beyond `max − consumed`. **The allocation rejection.** |
| 6102 | `InvalidAmount` | An amount or a cap that isn't positive |
| 6103 | `RestoreExceedsConsumed` | `restore` larger than `consumed` |
| 6104 | `BatchTooLarge` | More than 30 entries |

OpenZeppelin code that can surface: 2000 `Unauthorized`, for a caller without `controller` or `admin`.

## Storage

- One persistent entry per investor, extended on every write and read.
- Roles: persistent, through OpenZeppelin.
- Admin and schema version: instance.
