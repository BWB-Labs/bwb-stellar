# offer-token

- **What it does.** Holds the investor's quotas in one offering. One token is one quota (0 decimals). There is one instance per offering.
- **Who uses it.** The offering's sale contract receives the whole supply at construction and releases tokens to investors. Investors hold them and may send them only to whitelisted accounts. The tokenizer's treasury manages the whitelist and the offering URI, pauses and unpauses, and moves tokens by force when it must. The platform multisig replaces the code and can pause (recommended, pending the team).
- **Where it comes from.** A port of Base's `OfferERC20`: a SEP-41 token with 0 decimals, a transfer whitelist, pause, forced transfer and offering metadata. It is built on OpenZeppelin Stellar 0.7.2's fungible `Base`.

**Delivered in L2 (interface v0.6).** Testnet deployment: [`deployments/testnet.json`](../deployments/testnet.json), offering `t1-demo`. The cross-cutting rules are in [interface.md](interface.md).

## Powers

Four powers, each a stored address set at construction, **with no setter**. There is no access-control admin: nobody can grant anyone a power, so neither the tokenizer nor the platform can take the other's (ADR 0005).

| Power | Can | Held by (T1) |
|---|---|---|
| `admin` | manage the whitelist and the offering URI, pause and unpause | tokenizer's treasury |
| `xfer_admin` | move tokens between any two accounts, ignoring pause and the whitelist | tokenizer's treasury |
| `pauser` | pause only, never unpause | platform multisig. **Recommended (ADR 0003), pending the team.** If the team declines, the deploy passes the treasury here and the token behaves as Base. |
| `upgrader` | replace the code | platform multisig (ADR 0001) |

The contract doesn't know which party is which: the deploy passes the accounts, and the mainnet holders come from the key ceremony (L11). Rotating a treasury's signers keeps its address, so no setter is needed. Replacing an address takes an upgrade.

In BWB's production application on Base, the only tokenizer is BWB's own organization, BWB Tokenizadora, so `admin` and `xfer_admin` are BWB powers held through that organization's treasury, as they are on Base today.

## Constructor

| Argument | Meaning |
|---|---|
| `admin`, `xfer_admin`, `pauser`, `upgrader` | The four powers above |
| `name`, `symbol` | The offering's token metadata |
| `supply` | Total tokens, minted once. Must be positive. No later minting. |
| `initial_holder` | Receives the whole supply and is **whitelisted automatically**. It is the offering's sale contract, at its address precomputed from the deployer and a salt (ADR 0006, [interface 5.1](interface.md#51-deploying-an-offering)). |
| `whitelist` | Further accounts to whitelist. Duplicates, and the initial holder, are ignored. |
| `offer_uri` | URI of the offering's documents |

The constructor emits `initialized`, then OpenZeppelin's `mint`, then one `whitelist_updated` per account it whitelists.

## Calls

Every privileged call takes the signing account as its last argument (`caller` or `operator`). The call requires that account's signature, then checks it holds the power.

| Call | Authorized by | Effect |
|---|---|---|
| `transfer(from, to, amount)` | `from` | Moves tokens if `from` **or** `to` is whitelisted. For a muxed `to` (an address carrying a sub-account ID), the underlying account is checked. Blocked while paused. |
| `transfer_from(spender, from, to, amount)` | `spender` | Same rule, within the allowance. Blocked while paused. |
| `approve(owner, spender, amount, live_until_ledger)` | `owner` | Sets an allowance (SEP-41). Works while paused, as on Base. |
| `burn(from, amount)`, `burn_from(spender, from, amount)` | anyone | Always fail with `BurnDisabled`. They exist so standard SEP-41 clients get a contract error instead of a missing function. |
| `admin_transfer(from, to, amount, caller)` | `caller` = `xfer_admin` | Moves tokens between any two accounts, **ignoring pause and the whitelist**, as Base's `adminTransfer` (audit SCAN-04): court orders, lost keys, estates, manual returns. Rejects `from == to` and amounts that aren't positive. It can move the sale contract's balance too, including sold-but-unreleased tokens, which then breaks `release` ([interface 5.5](interface.md#55-after-success)). |
| `set_transfer_whitelist(account, allowed, caller)` | `caller` = `admin` | Adds or removes an account. Emits every time, even when nothing changes, as Base does. |
| `set_offer_uri(uri, caller)` | `caller` = `admin` | Replaces the offering URI. |
| `pause(caller)` | `caller` = `admin` or `pauser` | Pauses `transfer` and `transfer_from`. |
| `unpause(caller)` | `caller` = `admin` | Resumes them. |
| `upgrade(new_wasm_hash, operator)` | `operator` = `upgrader` | Replaces the code once the call succeeds. Storage is kept and the constructor doesn't run again. There is no `migrate` in this version. |

Read functions:

- SEP-41: `balance`, `allowance`, `decimals` (always 0), `name`, `symbol`, `total_supply`;
- whitelist and metadata: `is_transfer_whitelisted(account)`, `offer_uri`;
- state and powers: `paused`, `admin`, `xfer_admin`, `pauser`, `upgrader`, `schema_version` (1).

The code version is in the WASM metadata as `binver` (SEP-46, SEP-49): `stellar contract info meta`.

Investors are never whitelisted. An investor can only send tokens to a whitelisted account, never to another investor. Base has the same restriction.

The whitelist can drop the sale contract. Release then fails with 6000 until the sale contract is added back. Documented, not prevented, as on Base.

**Removed from Base:**

- `snapshot`, `balanceOfAt`, `totalSupplyAt`. The Base app never calls them. Dropping them removes the record-date mechanism that the NF-03 audit response relies on for holdings distributions, so a future pull distributor on Stellar needs its own record-date design.
- `version()`, replaced by `schema_version` and the `binver` metadata.
- `renounceOwnership`, and ownership altogether: there is no owner, only the four powers.

**Changed from Base:**

- `admin_transfer` rejects `from == to` and amounts that aren't positive. Base accepted both.
- A zero or negative supply fails at construction. Base accepted it.
- Base's owner held every power except the upgrade; here each power has its own address.

## Events

Inherited from OpenZeppelin:

- `transfer`, `mint` (once, at construction), `approve`;
- `paused` and `unpaused`, with empty data.

Their shapes are in [interface R4](interface.md#r4-inherited-event-shapes).

Specific to this contract:

| Event | Topics | Data | When |
|---|---|---|---|
| `initialized` | `initialized` | `{admin, xfer_admin, pauser, upgrader, initial_holder, supply, offer_uri}` | Once, at construction. An indexer can register the token from it. |
| `admin_transfer` | `admin_transfer`, from, to | `{amount}` | `admin_transfer`, after the standard `transfer`, so balance trackers need no special case. |
| `whitelist_updated` | `whitelist_updated`, account | `{allowed}` | `set_transfer_whitelist`, and once per account at construction |
| `offer_uri_updated` | `offer_uri_updated` | `{uri}` | `set_offer_uri` |
| `pause_changed` | `pause_changed`, caller | `{paused}` | `pause` and `unpause`, after OpenZeppelin's `paused` / `unpaused`. It records **who** paused, since two parties can. Base's `Paused(account)` carried this. |
| `upgraded` | `upgraded` | `{new_wasm_hash, from_schema_version}` | `upgrade`. The old code emits it, so `from_schema_version` is the schema version before the upgrade. The host also emits its own `executable_update`. |

## Errors

| Code | Name | When |
|---|---|---|
| 6000 | `TransferNotWhitelisted` | Neither `from` nor `to` is whitelisted |
| 6001 | `InvalidAdminTransfer` | `from` equals `to`, or the amount isn't positive |
| 6002 | `BurnDisabled` | `burn` or `burn_from` |
| 6003 | `NotAdmin` | The caller signed but isn't `admin` (`set_transfer_whitelist`, `set_offer_uri`, `unpause`) |
| 6004 | `NotXferAdmin` | The caller signed but isn't `xfer_admin` |
| 6005 | `NotPauser` | The caller signed but is neither `pauser` nor `admin` |
| 6006 | `NotUpgrader` | The operator signed but isn't `upgrader` |
| 6007 | `InvalidSupply` | The constructor's supply is zero or negative |

OpenZeppelin codes that can surface:

- 100 `InsufficientBalance`
- 101 `InsufficientAllowance`
- 103 `LessThanZero`
- 1000 `EnforcedPause`
- 1001 `ExpectedPause`

A missing signature fails as `Error(Auth, InvalidAction)`, not as a contract code ([interface 7.1](interface.md#71-codes)).

## Storage and expiry

- Balances: persistent, through OpenZeppelin. Allowances: temporary, through OpenZeppelin.
- Whitelist: one persistent entry per whitelisted account; removing an account deletes its entry.
- The four powers, the pause flag, the supply, the metadata, the URI and the schema version: instance.

Every state-changing call tops the instance, and every whitelist entry and balance it touches, up to the network maximum (about 180 days on mainnet) once it falls a month below it. An offering sees few calls, and rent is linear in time, so topping up to the maximum costs the same in total as many short extensions while keeping a quiet offering from archiving between calls. Balances that only OpenZeppelin touches keep its 30-day rule; keeping dormant balances alive is L9's.

## Tests

`scripts/test.sh` builds the upgrade fixture and runs the tests; CI runs the same script. The modules follow the D2 vocabulary:

| Module | Covers |
|---|---|
| `authorization` | A signed wrong caller per privileged call gets the power's code; an unsigned call fails at authorization; the pauser can't unpause; exact-signer positives |
| `boundary` | Supply 0, −1 and 1; whole balance and one more; forced-transfer edge amounts; duplicate whitelist inputs |
| `pause` | Paused blocks `transfer` and `transfer_from`, not `approve`, forced transfer or admin calls; pause twice, unpause a live token |
| `idempotency` | Repeated whitelist changes and URI updates converge |
| `failure` | Every documented code through the client, including both burn stubs |
| `upgrade` | Swap to `offer-token-v2-fixture`, a test-only second binary, and read every value back; a non-upgrader can't swap |
| `events` | Each call's exact events |
| `ttl` | The expiry policy |
| `transfers` | The whitelist rule (muxed recipients included), whitelist and URI management, forced transfer, fixed supply |
