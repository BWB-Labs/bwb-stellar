# offer-token

The investor's position in one offering, one instance per offering. It is a port of Base's `OfferERC20`: a SEP-41 token with 0 decimals, a transfer whitelist, pause, admin transfer and offering metadata. It is built on OpenZeppelin's fungible `Base`.

**Draft (interface v0.1).** The contract lands in L2, which updates this document. The cross-cutting rules are in [interface.md](interface.md).

## Constructor

| Argument | Meaning |
|---|---|
| `admin` | Issuer's treasury |
| `xfer_admin` | Issuer's treasury |
| `pauser` | Platform multisig |
| `upgrader` | Platform multisig |
| `name`, `symbol` | Offering's token metadata |
| `supply` | Total tokens, minted once. No later minting. |
| `initial_holder` | Receives the supply. The leading candidate is the sale's precomputed address ([interface 5.1](interface.md#51-deploying-an-offering)); decided in L2. |
| `whitelist` | Initial transfer whitelist. It includes the sale contract. |
| `offer_uri` | URI of the offering's documents |

## Calls

| Call | Authorized by | Effect |
|---|---|---|
| `transfer(from, to, amount)` | `from` | Moves tokens if `from` **or** `to` is whitelisted. Blocked while paused. |
| `transfer_from(spender, from, to, amount)` | `spender` | Same rule, within the allowance. Blocked while paused. |
| `approve(owner, spender, amount, live_until_ledger)` | `owner` | Sets an allowance (SEP-41). |
| `admin_transfer(from, to, amount)` | `xfer_admin` | Moves tokens between any two accounts, **even while paused**. |
| `set_transfer_whitelist(account, allowed)` | `admin` | Adds or removes an account from the whitelist. |
| `set_offer_uri(uri)` | `admin` | Replaces the offering URI. |
| `pause()` | `admin` or `pauser` | Pauses transfers. |
| `unpause()` | `admin` | Resumes transfers. |
| `upgrade(new_wasm_hash, operator)` | `upgrader` | Replaces the code. |

Read functions: `balance`, `allowance`, `decimals` (always 0), `name`, `symbol`, `total_supply`, `is_transfer_whitelisted(account)`, `offer_uri`, `paused`.

Investors are never whitelisted, so an investor can only send tokens to a whitelisted account, never to another investor. This is the same restriction as Base.

There is no burn and no snapshot. Base's snapshot is not ported.

## Events

Inherited from OpenZeppelin: `transfer`, `mint` (once, at construction), `approve`, `paused`, `unpaused`, `role_granted`, `role_revoked`. Their shapes are in [interface 6.2](interface.md#62-events-the-contracts-inherit).

Specific to this contract:

| Event | Topics | Data | When |
|---|---|---|---|
| `admin_transfer` | `admin_transfer`, from, to | `{amount}` | `admin_transfer`. A standard `transfer` event is emitted too, so balance trackers need no special case. |
| `whitelist_updated` | `whitelist_updated`, account | `{allowed}` | `set_transfer_whitelist`, and once per account at construction |
| `offer_uri_updated` | `offer_uri_updated` | `{uri}` | `set_offer_uri` |
| `upgraded` | `upgraded` | `{new_wasm_hash, schema_version}` | `upgrade` |

## Errors

| Code | Name | When |
|---|---|---|
| 6000 | `TransferNotWhitelisted` | Neither `from` nor `to` is whitelisted |
| 6001 | `InvalidAdminTransfer` | `from` equals `to`, or the amount isn't positive |

OpenZeppelin codes that can surface: 100 `InsufficientBalance`, 101 `InsufficientAllowance`, 103 `LessThanZero`, 1000 `EnforcedPause`, 1001 `ExpectedPause`, 2000 `Unauthorized`.

## Storage

- Balances and roles: persistent, through OpenZeppelin.
- Whitelist: one persistent entry per account.
- Admin, pause flag, supply, URI and schema version: instance.
