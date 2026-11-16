# offer-distributor

- **What it does.** Pays a list of recipients in one call. The owner sends a distribution: an id, an asset and at most 30 `{recipient, amount}`. In that same call the contract moves each amount from the owner's account straight to its recipient. Every recipient is paid or nobody is, and an id is paid at most once.
- **Scope.** One instance per offering. The contract needs no balance and knows nothing about what a distribution is for: a payout to an offering's stakeholders and a return to its investors look the same.
- **Who uses it.** The owner, a treasury, distributes, pauses, unpauses and rescues; every distribution is paid from the owner's own account. The backend builds the list, chooses the id and submits what the treasury signed. The platform multisig pauses and upgrades.
- **Where it comes from.** A port of Base's push `TokenDistributor`, following it wherever Stellar allows, with four changes: the payment is one atomic call instead of "fund, then distribute" (ADR 0009), an id cannot be paid twice, the contract can be paused and upgraded, and the owner can take out a balance that reached it by mistake.

**Delivered in L6.** Testnet deployment: [`deployments/testnet.json`](../deployments/testnet.json), offering `t1-demo-4`; `scripts/demo-offer-distributor.sh` exercises it. The design decisions are on issue [#28](https://github.com/BWB-Labs/bwb-stellar/issues/28); the facts under them are in [research/l6-push-distributor.md](research/l6-push-distributor.md). The cross-cutting rules are in [interface.md](interface.md).

## A distribution

```
distribute(id, asset, payouts, caller)

  owner ──transfer──▶ recipient 1      distributed {id, recipient 1, asset, amount}
  owner ──transfer──▶ recipient 2      distributed {id, recipient 2, asset, amount}
  …                                    distribution_completed {id, asset, total, count}
```

- **The money never rests in the contract.** Each `transfer` goes from the owner's account to a recipient. A distribution leaves the distributor's own balance as it was, which is zero unless someone sent it money by mistake ([A stray balance](#a-stray-balance)).
- **The signature covers the list.** The owner's authorization names this call with its exact arguments and each `transfer` the asset makes in the owner's name. A signature for one list pays that list and no other.
- **All or nothing.** If the asset refuses one recipient (no trustline, a balance its issuer froze), the whole call fails and nobody is paid. The backend's simulation shows the failure before the treasury signs, so a bad recipient is taken out of the list first.
- **An id is paid once.** The backend chooses the id, 32 bytes, as a hash of its own record of the distribution. A paid id is refused (6307) whatever list or asset comes with it. `is_paid(id)` answers "did my call go through?" when a response was lost.
- **A correction is a new distribution.** If distribution `42` paid someone too little, the missing amount goes out as a new distribution with a new id; `42` stays as it happened. The contract cannot take back an amount paid in excess: that is settled outside it.
- **A longer list is several distributions.** 70 recipients are three calls with three ids, each protected on its own.

## Powers

Three powers, each a stored address fixed at construction with no setter, as on the token and the sale (ADR 0005). Changing a holder takes an upgrade; a treasury rotates its officers without changing its address.

| Power | Held by | Can |
|---|---|---|
| `owner` | The treasury that pays | `distribute`, `rescue`, `pause`, `unpause` |
| `pauser` | Platform multisig | `pause` only, never `unpause` |
| `upgrader` | Platform multisig | `upgrade`. The trust root: a code swap can do anything |

A signed but wrong caller fails with the power's own code (6300–6302); a missing signature fails as `Error(Auth, InvalidAction)`. The contract doesn't know which party is which; the deploy passes the accounts.

**What pause is for.** Only the owner can distribute, so a pause blocks no third party: it stops the owner's own distributions until the owner unpauses. It does not stop `rescue`, which pays nothing from the owner's account. When the platform pauses, it is a stop that only a deliberate act of the treasury's signers lifts, which is the point of it during an incident in the backend that prepares the lists. It is no defence against the treasury's own signers, who can unpause.

**Why an owner, when nobody can spend another's money.** A distribution only ever pays from its caller's own account, so there the owner check protects no balance. It protects the record: every `distributed` event of this contract is a distribution by the offering's owner, and nobody else can use up an id before the owner does. On `rescue` the check does protect a balance: the one that reached the distributor by mistake.

**Which treasury is the owner** is decided at deploy, not here. On Base the app gives an offering's distributor to the issuer's treasury when the offering has one, and to the tokenizer's otherwise. An offering that needs two payers gets two instances.

**A lost owner.** A treasury is a multisig: it replaces a lost signer itself and its address does not change. If a treasury is lost altogether, no money is stranded, since the distributor is not where money is kept (a balance sent to it by mistake waits for an upgrade); the offering gets a new distributor, or the platform upgrades this one.

## Constructor

`__constructor(power_holders)`, one struct so the CLI names every field:

| Argument | Meaning |
|---|---|
| `power_holders` | `{owner, pauser, upgrader}` |

There is nothing else to configure: no asset, no name, no limits. The construction emits `configured`.

## Calls

| Call | Signed by | What it does |
|---|---|---|
| `distribute(id, asset, payouts, caller)` | `caller` = `owner` | Pays every `{recipient, amount}` in `asset` from the owner's account and records `id` as paid. Blocked while paused. |
| `rescue(asset, amount, caller)` | `caller` = `owner` | Sends `amount` of `asset` from the distributor's own balance to the owner. Works while paused. |
| `is_paid(id)` | nobody | Whether a distribution with this id was paid. |
| `powers()` | nobody | `{owner, pauser, upgrader}` |
| `paused()` | nobody | The pause flag |
| `schema_version()` | nobody | The storage layout version, 1 |
| `pause(caller)` | `caller` = `owner` or `pauser` | Pauses `distribute`. |
| `unpause(caller)` | `caller` = `owner` | Resumes it. |
| `upgrade(new_wasm_hash, operator)` | `operator` = `upgrader` | Replaces the code; storage is kept. |

**What `distribute` checks, in this order.** Nothing moves until all of it passes:

1. The contract is not paused (1000).
2. `caller` signed and is the owner (6300).
3. The list is not empty (6303) and holds at most 30 payouts (6304).
4. `id` was never paid (6307).
5. Every amount is positive (6305), no payout goes to the distributor itself or to the asset's own contract (6306), and the amounts fit an `i128` together (6308).

Then it records the id and makes the transfers. A transfer the asset refuses fails the call with the asset's own code and undoes everything, the id's record included, so the same id can be sent again once the list is fixed.

**What it allows.** The same recipient may be listed more than once; each entry is paid and has its own event. The owner may list itself. Recipients are accounts or contracts. A muxed address is not an address the list can carry: such a call fails before the contract's checks, without a contract code.

**The asset** is an argument of each distribution, as on Base, so one distributor can pay in more than one asset. It is whatever address the owner passes, and the contract does not restrict it or check what stands behind it. An address with no contract fails the call. A contract that answers `transfer` and moves nothing makes the distribution "succeed": the id is recorded and the events are emitted with no money moved. So the asset is part of what the treasury signs, and a wrong asset address is the signers' to catch ([Authorization](#authorization)).

**Batch size.** At most 30 payouts per call, the same as the sale's payout. It is a cap, not a promise. A transaction may emit 16,384 B of events, and a full distribution of USDC to classic accounts emits 30 × (USDC `transfer` 244 B + `distributed` 260 B) + `distribution_completed` 252 B = 15,372 B, 94% of that. This is less room than the other batch limits leave ([interface R5](interface.md#r5-event-sizes)). It is accepted because exceeding the limit loses nothing: the call fails whole in simulation, before anyone signs, and the backend sends a shorter list. That happens with an asset whose own `transfer` event is larger than 276 B. Adding a field to `distributed` means lowering the cap in the same release. A full distribution writes 33 entries, against a limit of 200.

### A stray balance

The distributor needs no balance, and no contract can refuse a token transfer made to its address: the asset updates its own books and never calls the receiver. So money can reach the distributor by mistake, from someone used to Base's "fund, then distribute" or from a pasted address. The one exception is an asset whose issuer requires authorization for every holder: a transfer to a distributor it never authorized fails at the sender. `rescue(asset, amount, caller)` is the way out.

- **Owner only** (6300). The pauser, the upgrader and the account that sent the money cannot call it; the sender asks the treasury.
- **The money goes to the owner and nowhere else.** The call has no recipient to get wrong: a rescue built by mistake, or signed under a false story about whose money it is, lands in the treasury. Returning it to its sender is then an ordinary payment by the treasury, which can carry a memo or a muxed address for an exchange. The signers find the sender in the `from` of the asset's `transfer` that credited the distributor, not in whoever asks for the money.
- **It moves only what the distributor holds.** The amount is positive (6305); more than the balance fails with the asset's code (10), and so does a balance the issuer froze, the distributor's or the owner's, or an owner account with no trustline for the asset (11, 13; [Errors](#errors)). The owner signs the call and no transfer, so signed that way a rescue cannot spend from the owner's account ([Authorization](#authorization)).
- **It works while paused,** uses no distribution id and emits `rescued`.
- **A wrong asset address gets nothing from the distributor.** The distributor authorizes only the one `transfer` it makes, on the asset named; that contract cannot move another asset's balance in the distributor's name.

**What this adds to a leaked or mistaken owner.** The owner can take whatever the distributor holds into its own account. That is money that should not be there, and it is never an investor's payout in transit: a distribution does not pass through this balance.

### Authorization

The treasury signs one authorization entry. Its root is `distribute(id, asset, payouts, owner)`; under it comes the asset's `transfer(owner, recipient, amount)`, one per payout, in the list's order, and nothing else ([interface 4](interface.md#4-invocation-and-authorization)). BWB submits the transaction and pays the fee.

For a `rescue` the entry is the root alone, `rescue(asset, amount, owner)`, with no sub-invocation: the distributor authorizes its own `transfer`.

**The backend builds that tree, or checks it, before anyone signs. It never passes on a tree as a simulation recorded it.** A recording simulation grants whatever the called contracts ask for in the owner's name. If a wrong asset address reached the call, its `transfer` could ask to move the owner's real USDC elsewhere, the simulation would record that as one more sub-invocation, and a treasury signing the recorded tree would authorize it. Signed with the tree above, the same call fails and moves nothing (the `authorization` tests show both). The same holds for a rescue with a wrong asset address: a recorded tree with anything under the root is refused, and the `rescue` tests show both outcomes. The check is mechanical: the root's arguments are the distribution; every sub-invocation is `transfer` on the root's own `asset`, from the owner, to the payout's recipient, for its amount; there are as many as payouts; none has children.

**A signed authorization stays usable until its expiry ledger,** which the backend sets, and anyone holding it can submit it. If the treasury signs a list, finds a mistake before it is sent, and signs a corrected one, the corrected list keeps the **same id**: only one of the two can then ever be paid. A new id is for a correction to a distribution that was already paid. A short expiry limits how long an unsent authorization stays live. A signed rescue that fails for lack of balance is not used up either: it can be sent again until it expires, and would pass once the distributor holds enough.

**The distributor calls whatever `asset` the owner names, as itself.** Today its address holds no role, allowance or whitelist entry in any contract, so that reaches nothing but its own stray balances. Giving the distributor's address a power elsewhere would put that power in the owner's hands through `rescue`.

## Differences from Base

- **One atomic call instead of two steps.** Base funds the distributor with a plain transfer and then calls `distributeTokens`, which pays from the contract's balance; a Safe puts both in one transaction. A Stellar transaction carries one contract call, so the two steps would be two transactions with money parked in between. Here `distribute` moves the money from the owner to the recipients itself, and the contract needs no balance (ADR 0009).
- **`rescue` for a balance sent by mistake,** which goes only to the owner. Base's distributor has no such call, and none is needed there: whatever it holds is paid out by the next `distributeTokens`. Here nothing pays from the contract's balance, so a stray one would stay without it.
- **An id that cannot be paid twice.** Base has none: the same list sent twice pays twice.
- **Pause and upgrade through stored powers.** Base's distributor has one `Ownable` owner, transferable, and no pause; its code changes through the factory's beacon. Here the powers are three fixed addresses, the platform can pause, and each instance upgrades itself (ADR 0001, ADR 0005).
- **30 payouts per call, not 200.** The limit comes from the events a transaction may emit.
- **No `name` or `descriptionURI`,** and no `updateMetadata`. The backend's registry knows which offering a distributor belongs to.
- **`distributed` carries no balance.** Base's per-recipient event reports the distributor's balance after each transfer; there is no balance to report.
- **Amounts must be positive and the list non-empty.** Base accepts a zero amount and an empty list.
- **The caller is an explicit last argument,** as on the other contracts. Base reads `msg.sender`.
- **Kept from Base:** the asset is chosen per distribution; the same recipient may appear twice; one failing recipient fails the whole batch; one event per recipient and one per batch.

## Events

Data fields are listed in sorted name order.

| Event | Topics | Data | When |
|---|---|---|---|
| `configured` | `configured` | `{owner, pauser, upgrader}` | Construction |
| `distributed` | `distributed`, id, recipient | `{amount, asset}` | `distribute`, per payout, after the asset's `transfer` |
| `distribution_completed` | `distribution_completed`, id | `{asset, count, total}` | `distribute`, once, after the last `distributed` |
| `rescued` | `rescued`, asset | `{amount}` | `rescue`, after the asset's `transfer` |
| `pause_changed` | `pause_changed`, caller | `{paused}` | `pause` and `unpause`, after OpenZeppelin's |
| `upgraded` | `upgraded` | `{from_schema_version, new_wasm_hash}` | `upgrade`, emitted by the old code |

Also emitted, by the asset the distributor calls: one `transfer` from the owner to the recipient before each `distributed`, and one from the distributor to the owner before `rescued`. A refused call emits nothing.

**Reading them.** `id` is a topic on both events, so an indexer finds every payout of a distribution by it, and the backend matches it to its own record. `distribution_completed` is the proof a distribution was paid in full: `total` is the sum of its `count` payouts. `rescued` is the only event that goes with money leaving the distributor's own address. All three record what the distributor asked the asset to do; the asset's own `transfer` event is the proof the money moved.

**Sizes.** Measured XDR encodings, pinned by a test: `distributed` 260 B to a classic account and 256 B to a contract, `distribution_completed` 252 B.

## Errors

| Code | Name | When |
|---|---|---|
| 6300 | `NotOwner` | A signed caller that is not `owner` |
| 6301 | `NotPauser` | A signed caller that is neither `pauser` nor `owner`, on `pause` |
| 6302 | `NotUpgrader` | A signed caller that is not `upgrader` |
| 6303 | `EmptyBatch` | A distribution with no payouts |
| 6304 | `BatchTooLarge` | More than 30 payouts |
| 6305 | `NonPositiveAmount` | A payout, or a rescue, of zero or less |
| 6306 | `InvalidRecipient` | A payout to the distributor itself, or to the asset's own contract, where it would be stranded |
| 6307 | `AlreadyPaid` | The id was already paid |
| 6308 | `AmountOverflow` | The amounts add up to more than an `i128` holds |

Codes from the contracts it calls, which surface unchanged:

- **the asset** (a Stellar Asset Contract), during a distribution or a rescue: 10 `BalanceError` (the owner holds less than the list is worth, a recipient's trustline is at its limit, or a rescue asks for more than the distributor holds), 11 `BalanceDeauthorizedError` (the issuer froze the owner's or a recipient's balance, or in a rescue the distributor's or the owner's; the code does not say which), 13 `TrustlineMissingError` (a recipient account, or in a rescue the owner's, has no trustline for the asset, or does not exist) ([interface R6](interface.md#r6-error-codes));
- **OpenZeppelin:** 1000 `EnforcedPause` and 1001 `ExpectedPause` from the distributor's own pause.

A missing signature fails as `Error(Auth, InvalidAction)`, not as a contract code ([interface 7.1](interface.md#71-codes)). An authorization that does not cover every transfer is refused inside the asset's call, which fails the distribution without a contract code either.

## Storage and expiry

- Paid ids: one persistent entry per distribution, written by `distribute` and never removed.
- Powers: instance, with OpenZeppelin's pause flag and schema version.

**Policy.** Every state-changing call tops the instance up to the network maximum once it falls a month below it, and `distribute` writes its id's entry with the maximum lifetime. The reads extend nothing.

**An id stays paid without upkeep.** Nobody keeps old ids alive, and nobody needs to. A persistent entry that runs out is archived, not deleted: it can be restored but never written again, so a call with an archived id cannot pay a second time. A simulation still reads the archived entry as present, so `is_paid` answers true and `distribute` fails with 6307; a transaction sent anyway, without restoring the entry, is refused by the network before the contract runs.

**Money sent to the distributor stays there until the owner rescues it.** The contract never needs a balance, and `rescue` is the only call that moves one ([A stray balance](#a-stray-balance)).

## Tests

`scripts/test.sh` builds the upgrade fixtures and runs the tests; CI runs the same script. One seam: the contract client in the Soroban test environment, beside a Stellar Asset Contract as the payment asset, revocable like Circle's USDC. The owner is funded and pays from its own account, as in production. The modules follow the D2 vocabulary:

| Module | Covers |
|---|---|
| `authorization` | A signed wrong caller per privileged call gets the power's code, a funded stranger included, with the pauser and the upgrader as different accounts so neither passes for the other; an unsigned call fails at authorization; the reads with no auths at all; the owner's exact authorization with one `transfer` per payout; a signature for one list does not pay another; a signature without the transfers, or missing one, moves nothing; an asset that asks for more than was signed gets nothing, and gets paid under a recorded tree |
| `boundary` | 30 payouts and the 31st; an empty list; zero and negative amounts, `i128::MIN` included; one stroop; the largest amounts, which reach the asset and fail there; a payout to the distributor itself or to the asset's own contract; a recipient listed twice; the owner listing itself; an id of all zeros; amounts that overflow together; the distributor's balance stays zero; a full distribution to classic accounts measured against the network limits, with its 33 write entries |
| `rescue` | The owner takes a stray balance into its own account, whole or in parts; nobody else can, the sender included; the owner signs the call and no transfer; an unsigned call fails at authorization; a zero or negative amount, more than the balance and an empty distributor are refused; a balance the issuer froze, the distributor's or the owner's, fails with the asset's code; it works while paused and leaves distributions as they were; a hostile asset cannot take another asset's balance, and gets the owner's money only under a recorded tree; the exact `rescued` event after the asset's `transfer`; a refused rescue emits nothing; the instance is topped up |
| `pause` | A paused distributor blocks `distribute` until the owner unpauses; the owner may pause too; the reads answer while paused; OpenZeppelin's 1000 and 1001 |
| `idempotency` | The same distribution sent twice pays once; a paid id is refused with another list or asset; a correction as a new id; a failed call and a refused call leave their id unpaid; ids are separate per distributor; an id stays paid and refused after its entry's lifetime ran out |
| `failure` | A frozen recipient, a recipient without a trustline and a recipient account that does not exist each fail the whole distribution with the asset's code, and it passes once the list is fixed; an owner without enough balance and a frozen owner pay nobody; an asset address with no token behind it, the distributor's own included; an asset that moves nothing still records the distribution; an asset that calls back into the distributor; the first rule broken is the one reported; one distributor paying in two assets |
| `events` | Exact events and order for construction, a distribution, pause and unpause, OpenZeppelin's own included; each `distributed` follows the asset's `transfer`; a refused distribution emits nothing; the measured sizes |
| `upgrade` | Swap to `offer-distributor-upgrade-fixture`, a test-only second binary, with the exact `upgraded` event; the three powers, each a different account, the paid ids, the pause flag and `schema_version` survive; the upgraded code can upgrade again under the same gate; a hash never uploaded fails; neither the owner nor the pauser can swap; an upgrade tops the instance up |
| `ttl` | The instance on construction, after a quiet month and not before it, on pause and unpause; a paid id written with the maximum lifetime; reads extend nothing |
