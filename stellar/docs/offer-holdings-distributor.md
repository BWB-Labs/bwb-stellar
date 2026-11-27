# offer-holdings-distributor

Pays an offering's returns to whoever holds its quotas. The owner, the issuer's treasury, deposits money. Each holder's share is its quota balance times what one quota has earned, less what the holder was already paid, and it leaves the contract in a claim that anyone may send for up to 50 holders.

One instance per offering, tied at construction to the offering's token and to one payment asset. A port of Base's `TokenDistributorByHoldings` ([Differences from Base](#differences-from-base)).

**Delivered in L7.** Testnet deployment: [`deployments/testnet.json`](../deployments/testnet.json), offering `t1-demo-4`; `scripts/demo-offer-holdings-distributor.sh` exercises it. The decisions are on issue [#29](https://github.com/BWB-Labs/bwb-stellar/issues/29) and the cross-cutting rules in [interface.md](interface.md). The other distributor, [`offer-distributor`](offer-distributor.md), pays a list the owner writes and holds no balance.

## A deposit and its claims

```
deposit:  counter += amount × 10^18 ÷ total supply
claim:    pays  balance × counter ÷ 10^18 − paid so far
```

With 1,000 quotas and a deposit of 1,000 USDC, the counter rises by 1 USDC per quota. A holder of 100 quotas claims 100 USDC and its record becomes 100. After a second deposit of 2,000 USDC the same holder claims 200.

- **A deposit touches no holder**, so its cost does not grow with the number of holders. It is divided among the quotas that exist when it arrives.
- **A share is worked out when it is claimed.** A holder who skipped three deposits gets all three in one claim; a second claim with nothing new pays nothing.
- **Anyone sends a claim; the holder gets the money.** Nobody signs, so the platform relays claims without holding any key of the holders.
- **Rounding goes down**, so the contract never owes more than it holds. The remainder, under one stroop per quota per deposit and per holder per claim, stays in the contract.
- **No arithmetic wraps.** Every multiplication, addition and subtraction on these values is checked: a deposit that does not fit is refused (6407), a share that does not fit is skipped. The release build keeps overflow checks on.

## Who is not paid

A claim passes over a holder in four cases and pays the rest of the list. No holder can make a claim fail.

| Case | What happens |
|---|---|
| Nothing to receive | Passed over, no event |
| The holder is on the token's whitelist | `claim_skipped` |
| The payment asset refuses the transfer: no trustline, a frozen balance, or a distributor the owner emptied | `claim_skipped` |
| The share does not fit an `i128` | `claim_skipped` |

A skipped holder's record does not change, so a later claim pays the same share once the cause is gone. The backend reads `claim_skipped` to know who to retry.

Whitelisted accounts are operational accounts, today only the sale contract. Money paid to the sale would be stuck there, so the share of the quotas it holds waits in the distributor: a deposit made before every `release` is safe, and the investor claims after receiving the quotas. The share of quotas a whitelisted account keeps for good accumulates until the owner takes it out with `withdraw`.

## Quotas that move after a claim

The balance is read when the claim runs and "paid so far" is kept per account, as on Base. The contract does not follow quotas from one account to another.

**Operating rule.** After an offering's first deposit, no forced transfer (`admin_transfer`), and no quotas out of a whitelisted account other than a `release`, without first checking both accounts: claim for the sender until its `claimable` is zero, and compare the receiver's `credited` with what its new balance will earn. Today no flow moves quotas after a deposit: investors can send quotas only to a whitelisted account, and the only one is the sale contract, which sends them out only in a `release`.

A holder who returns quotas gives up money with them: returned before a claim, the quotas take their unclaimed share along; returned after one, what was paid on them is discounted from what the remaining quotas earn next.

## Powers

Three stored addresses, fixed at construction with no setter, as on the other contracts (ADR 0005).

| Power | Held by | Can |
|---|---|---|
| `owner` | the issuer's treasury | `deposit`, `withdraw`, `pause`, `unpause` |
| `pauser` | platform multisig | `pause` only |
| `upgrader` | platform multisig | `upgrade` |

The owner is the issuer, not the tokenizer: the return money leaves the issuer's account and a withdrawal goes back to it. The platform never holds it. This is the one contract where the issuer has a role.

## Calls

```
__constructor(power_holders: {owner, pauser, upgrader}, contracts: {token, payment_asset})
```

Stores both and emits `configured`. Neither can be changed later.

| Call | Who signs | What |
|---|---|---|
| `deposit(amount, caller)` | owner, for the call and the asset's one `transfer` of this amount | Takes `amount` from the owner's account and raises the counter. Blocked while paused |
| `claim(holders)` | nobody | Pays each of 1 to 50 holders its share. Blocked while paused |
| `withdraw(asset, amount, caller)` | owner | Sends `amount` of any asset from the distributor's balance to the owner's own account. Works while paused |
| `pause(caller)` | owner or pauser | Blocks `deposit` and `claim` |
| `unpause(caller)` | owner | Resumes |
| `upgrade(new_wasm_hash, operator)` | upgrader | Replaces the code; storage is kept |

`withdraw` has no recipient, and `asset` is free, so a token sent here by mistake leaves the same way. On the payment asset it can take money holders have not claimed yet, as Base's `withdrawTokens` can: no record changes, and claims are skipped until the money is back.

| Read | Answers |
|---|---|
| `claimable(holder)` | What a claim would try to pay now; zero for a whitelisted account; fails (6407) for a share that does not fit an `i128` |
| `credited(holder)` | What the holder was paid so far |
| `cumulative_per_quota()` | The counter, times 10^18 |
| `contracts()`, `powers()` | What the constructor stored |
| `paused()`, `schema_version()` | The pause flag; the storage layout version, 1 |

No read needs a signature or extends any of the distributor's entries.

## Batch size

A claim takes at most 50 holders. What binds is the events a transaction may emit: each paid holder costs the payment asset's `transfer` event, 244 B for USDC to a classic account. A full claim, measured by a test:

| Resource | A claim of 50 | Network limit |
|---|---|---|
| Events | 12,340 B with USDC | 16,384 B |
| Write entries | 101 | 200 |
| Footprint entries | 205 | 400 |

50 is a cap, not a promise: a list that does not fit fails whole in simulation and the sender splits it. Two things the table leaves out: holders whose entries were archived need them restored in the same transaction (about 150 disk reads of 200 and 151 writes for 50 holders, an estimate), and the instruction count comes from the transaction's simulation, since the test runs native code.

## Differences from Base

| Base (`TokenDistributorByHoldings`) | Here | Why |
|---|---|---|
| `distribute(payer, amount)` | `deposit(amount, caller)` | The payer was always the owner; the name says what the call does |
| `claim()`, `claimFor(holder)`, `claimMany(holders)` | `claim(holders)` | With no signature from the holder the three are one operation |
| Any holder can claim | A whitelisted account cannot | Money claimed to the sale would be stuck |
| One failing holder reverts `claimMany` | The holder is skipped with `claim_skipped` | A batch keeps moving and the backend learns who was left out |
| `withdrawTokens(token, recipient)` sends the whole balance anywhere | `withdraw(asset, amount, caller)` pays only the owner | A mistaken or coerced withdrawal lands in the issuer's treasury |
| No pause, no upgrade | Both, as on the other contracts | |
| Storage never expires | Each record is persistent and renewed when its holder is paid | [Storage and expiry](#storage-and-expiry) |

The same on both: the counter and its 10^18 magnitude, the per-account record, and claims open to anyone. Changes proposed for both are on [#59](https://github.com/BWB-Labs/bwb-stellar/issues/59).

## Events

Data fields are listed in sorted name order.

| Event | Topics | Data | When |
|---|---|---|---|
| `configured` | `configured` | `{owner, pauser, payment_asset, token, upgrader}` | Construction |
| `deposited` | `deposited` | `{amount}` | `deposit`, after the asset's `transfer` |
| `claim_skipped` | `claim_skipped`, holder | `{}` | `claim`, per holder it could not pay |
| `claimed` | `claimed` | `{count, total}` | `claim`, once, after the payments, even when nobody was paid |
| `withdrawn` | `withdrawn`, asset | `{amount}` | `withdraw`, after the asset's `transfer` |
| `pause_changed` | `pause_changed`, caller | `{paused}` | `pause` and `unpause`, after OpenZeppelin's |
| `upgraded` | `upgraded` | `{from_schema_version, new_wasm_hash}` | `upgrade`, emitted by the old code |

A paid holder has no event of the distributor's: the payment asset's `transfer` from the distributor to the holder is the record of who received how much, and an indexer reads it. A refused call emits nothing.

Sizes, pinned by a test: `claimed` 140 B, `claim_skipped` 132 B to a classic account and 128 B to a contract.

## Errors

| Code | Name | When |
|---|---|---|
| 6400 | `NotOwner` | A signed caller that is not `owner` |
| 6401 | `NotPauser` | A signed caller that is neither `pauser` nor `owner`, on `pause` |
| 6402 | `NotUpgrader` | A signed caller that is not `upgrader` |
| 6403 | `EmptyBatch` | A claim with no holders |
| 6404 | `BatchTooLarge` | More than 50 holders |
| 6405 | `NonPositiveAmount` | A deposit, or a withdrawal, of zero or less |
| 6406 | `NoSupply` | A deposit while the token has no supply |
| 6407 | `AmountOverflow` | A deposit too large for the counter, or `claimable` for a share too large for an `i128` (deposits beyond about 1.7 × 10^13 USDC in all) |

Codes from other contracts surface unchanged in a deposit or a withdrawal: the payment asset's 10 `BalanceError`, 11 `BalanceDeauthorizedError` and 13 `TrustlineMissingError` ([interface R6](interface.md#r6-error-codes)), and OpenZeppelin's 1000 `EnforcedPause` and 1001 `ExpectedPause`. In a claim the asset's codes do not surface: the holder is skipped. A missing signature fails as `Error(Auth, InvalidAction)` ([interface 7.1](interface.md#71-codes)).

## Storage and expiry

- **Persistent:** each holder's "paid so far", written with the maximum lifetime when the holder is paid and never removed.
- **Instance:** the counter, the powers, the two contracts, the pause flag and the schema version. Every state-changing call tops it up to the network maximum once it falls a month below it.

A record cannot read as zero again: an entry that runs out is archived, not deleted, and a claim for that holder restores it with the same value.

## Tests

The tests are in `src/test/`, one module per subject (`authorization`, `deposit`, `claim`, `skip`, `withdraw`, `pause`, `events`, `upgrade`, `ttl`), against the real `offer-token` and a Stellar Asset Contract as the payment asset. `scripts/test.sh` builds the upgrade fixture and runs them; CI runs the same script.
