# offer-token

- **What it does.** Holds the investor's quotas in one offering. One token is one quota (0 decimals). There is one instance per offering.
- **Who uses it.** The offering's sale contract holds the supply and releases tokens to investors. Investors hold them and may send them only to whitelisted accounts. The tokenizer's treasury manages the whitelist, the offering URI, pause and admin transfers. The platform multisig holds upgrades and can pause (recommended, pending the team).
- **Where it comes from.** A port of Base's `OfferERC20`: a SEP-41 token (except `burn` and `burn_from`) with 0 decimals, a transfer whitelist, pause, admin transfer and offering metadata. It is built on OpenZeppelin's fungible `Base`.

**Draft (interface v0.5).** The contract lands in L2, which updates this document. The cross-cutting rules are in [interface.md](interface.md).

## Roles

| Role | How it is held | Held by | Can |
|---|---|---|---|
| `admin` | A stored address, set at construction, **with no setter** | Tokenizer's treasury | Manage the whitelist and the offering URI, pause and unpause |
| `xfer_admin` | A stored address, set at construction, **with no setter** (as in Base) | Tokenizer's treasury | Move tokens between any two accounts |
| OpenZeppelin access-control admin | OpenZeppelin `AccessControl` | Platform multisig | Grant and revoke `pauser` and `upgrader` |
| `pauser` | OpenZeppelin role | Platform multisig | Pause only, never unpause. **Recommended (ADR 0003), pending the team** |
| `upgrader` | OpenZeppelin role | Platform multisig | Replace the code |

Rotating the treasury's officers keeps the same account address, so the stored addresses need no setter.

The tokenizer's `admin` is deliberately **not** the OpenZeppelin access-control admin. OpenZeppelin always lets its top admin grant and revoke any role. If the treasury held it, it could grant itself `upgrader` or remove the platform's `pauser`.

## Constructor

| Argument | Meaning |
|---|---|
| `admin` | Tokenizer's treasury |
| `xfer_admin` | Tokenizer's treasury |
| `platform` | Platform multisig: access-control admin, `pauser` and `upgrader` |
| `name`, `symbol` | Offering's token metadata |
| `supply` | Total tokens, minted once. No later minting. |
| `initial_holder` | Receives the supply and is **whitelisted automatically**, as in Base. The leading candidate is the sale contract's precomputed address ([interface 5.1](interface.md#51-deploying-an-offering)); decided in L2. |
| `whitelist` | Further accounts to whitelist. Duplicates are ignored. |
| `offer_uri` | URI of the offering's documents |

## Calls

| Call | Authorized by | Effect |
|---|---|---|
| `transfer(from, to, amount)` | `from` | Moves tokens if `from` **or** `to` is whitelisted. For a muxed `to` (an address carrying a sub-account ID), the underlying address is checked. Blocked while paused. |
| `transfer_from(spender, from, to, amount)` | `spender` | Same rule, within the allowance. Blocked while paused. |
| `approve(owner, spender, amount, live_until_ledger)` | `owner` | Sets an allowance (SEP-41). |
| `admin_transfer(from, to, amount)` | `xfer_admin` | Moves tokens between any two accounts, **ignoring both pause and the whitelist**, as in Base (`adminTransfer`). That includes the sale contract's balance: it can move unsold tokens out of a Successful sale contract, so leftovers are not locked. It can also pull sold-but-unreleased tokens and break `release` ([interface 5.5](interface.md#55-after-success)). Where leftovers go is pending the team; this is a fact about the contract, not the decided exit path. |
| `set_transfer_whitelist(account, allowed)` | `admin` | Adds or removes an account from the whitelist. |
| `set_offer_uri(uri)` | `admin` | Replaces the offering URI. |
| `pause(caller)` | `caller`, holding `admin` or `pauser` | Pauses transfers. The platform's `pauser` is recommended (ADR 0003), pending the team. |
| `unpause(caller)` | `caller`, holding `admin` | Resumes transfers. |
| `upgrade(new_wasm_hash, operator)` | `operator`, holding `upgrader` | Replaces the code. |
| `grant_role(account, role, caller)` / `revoke_role(…)` | access-control admin | Manages `pauser` and `upgrader` (OpenZeppelin). |
| `transfer_admin_role(new_admin, live_until_ledger)` / `accept_admin_transfer()` | access-control admin, then the new admin | Two-step handover of the access-control admin (OpenZeppelin). |

`renounce_admin` is blocked: the platform's roles must never be left without an admin.

Read functions:

- SEP-41: `balance`, `allowance`, `decimals` (always 0), `name`, `symbol`, `total_supply`;
- whitelist and metadata: `is_transfer_whitelisted(account)`, `offer_uri`;
- state and roles: `paused`, `admin`, `xfer_admin`, `has_role(account, role)` (the role index, or void when not held), `schema_version`.

Investors are never whitelisted. An investor can only send tokens to a whitelisted account, never to another investor. Base has the same restriction.

**Removed from Base:**

- `snapshot`, `balanceOfAt`, `totalSupplyAt`. The Base app never calls them. Dropping them removes the record-date mechanism that the NF-03 audit response relies on for holdings distributions, so a future pull distributor on Stellar needs its own record-date design.
- `version()`, replaced by `schema_version`.
- `renounceOwnership`.

There is no `burn` or `burn_from`. SEP-41 requires both, so the token is SEP-41 except for them. L2 may expose both as calls that always fail with a contract error, so standard token clients get a clean error instead of a missing function.

**Changed from Base:** `admin_transfer` rejects `from == to` and amounts that aren't positive. Base accepted both.

## Events

Inherited from OpenZeppelin:

- `transfer`, `mint` (once, at construction), `approve`;
- `paused` and `unpaused`, with empty data;
- `role_granted`, `role_revoked`, `admin_transfer_initiated`, `admin_transfer_completed`.

Their shapes are in [interface R4](interface.md#r4-inherited-event-shapes).

Specific to this contract:

| Event | Topics | Data | When |
|---|---|---|---|
| `admin_transfer` | `admin_transfer`, from, to | `{amount}` | `admin_transfer`. A standard `transfer` event is emitted too, so balance trackers need no special case. |
| `whitelist_updated` | `whitelist_updated`, account | `{allowed}` | `set_transfer_whitelist`, and once per account at construction |
| `offer_uri_updated` | `offer_uri_updated` | `{uri}` | `set_offer_uri` |
| `pause_changed` | `pause_changed`, caller | `{paused}` | `pause` and `unpause`. It records **who** paused, since two parties can if the team confirms ADR 0003. Base's `Paused(account)` carried this. |
| `upgraded` | `upgraded` | `{new_wasm_hash, from_schema_version}` | `upgrade`. The old code emits it, so `from_schema_version` is the schema version before the upgrade. |

## Errors

| Code | Name | When |
|---|---|---|
| 6000 | `TransferNotWhitelisted` | Neither `from` nor `to` is whitelisted |
| 6001 | `InvalidAdminTransfer` | `from` equals `to`, or the amount isn't positive |
| 6002 | `RenounceBlocked` | `renounce_admin` |

OpenZeppelin codes that can surface:

- 100 `InsufficientBalance`
- 101 `InsufficientAllowance`
- 103 `LessThanZero`
- 1000 `EnforcedPause`
- 1001 `ExpectedPause`
- 2000 `Unauthorized`
- 2007 `RoleNotHeld`

A missing signature fails as `Error(Auth, …)`, not as a contract code ([interface 7.1](interface.md#71-codes)).

## Storage

- Balances and roles: persistent, through OpenZeppelin.
- Whitelist: one persistent entry per account.
- `admin`, `xfer_admin`, the pause flag, the supply, the URI and the schema version: instance.
