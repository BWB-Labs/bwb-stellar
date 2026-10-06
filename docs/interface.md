# Backend ↔ contracts interface

| | |
|---|---|
| Version | 0.3 (draft) |
| Status | Under review ([#23](https://github.com/BWB-Labs/bwb-stellar/issues/23)) |
| Contracts | `offer-token`, `offer-allowlist`, `offer-sale` (Tranche 1) |
| Stack | soroban-sdk 26.1, OpenZeppelin Stellar Contracts 0.7.2, protocol 29 |

This document is the contract between two layers: the BWB backend, and the Soroban contracts that carry BWB's offerings on Stellar. It says how the backend calls the contracts, who signs each call, what the contracts report back, and the rules both sides rely on. Each contract's calls, events and errors are in its own document:

- [offer-token.md](offer-token.md)
- [offer-allowlist.md](offer-allowlist.md)
- [offer-sale.md](offer-sale.md)

The contracts port BWB's Base (EVM) contracts. Where the Base backend already does something and Stellar can do it the same way, this document keeps Base's shape. Where Stellar can't, it says what replaces it. [Section 2](#2-from-base-to-stellar) is that comparison, part by part.

**Stability.** The contracts are written after this document (L2–L4). Until L4 lands, the document stays at 0.x, changes are allowed, and every change goes in the [changelog](#changelog) and is announced to the team. From 1.0, events only change by the [additive rule](#61-rules).

**Out of scope.** Stellar protocol work that involves no contract here (account creation, reserve sponsorship, USDC trustlines, fee-payer account pools, network error handling), any TypeScript code, the AUM query, the evidence index, and key custody.

## Contents

1. [Parties, roles and keys](#1-parties-roles-and-keys)
2. [From Base to Stellar](#2-from-base-to-stellar)
3. [Premises](#3-premises)
4. [Invocation and authorization](#4-invocation-and-authorization)
5. [Flows](#5-flows)
6. [Events](#6-events)
7. [Errors](#7-errors)
8. [Grant vocabulary and AUM inputs](#8-grant-vocabulary-and-aum-inputs)
9. [Storage and expiry](#9-storage-and-expiry)
10. [What the backend registry needs](#10-what-the-backend-registry-needs)
11. [Open points](#11-open-points)
12. [Risks](#12-risks)

## 1. Parties, roles and keys

Vocabulary follows the glossary of BWB's Base application.

- **Investor:** a user buying through their **wallet** (one signer), or an organization buying through its **treasury** (a threshold of officers). The contracts treat both the same: an address that authorized the call.
- **Issuer:** the organization raising money through an offering. Its treasury administers the offering.
- **Offering:** one `offer-token` contract plus one `offer-sale` contract.
- **Platform multisig:** BWB's own threshold account. It is used rarely.
- **Platform hot key:** a single BWB key held by the backend, which signs without a human.

| Role | Contract | Held by | Can |
|---|---|---|---|
| admin (stored address) | `offer-token` | issuer's treasury | manage the transfer whitelist and the offering URI, pause and unpause |
| `xfer_admin` (stored address, fixed at construction) | `offer-token` | issuer's treasury | move tokens between any two accounts, even while paused |
| owner (stored address) | `offer-sale` | issuer's treasury | finalize, mark failed, cancel, withdraw payment, withdraw tokens after failure, pause and unpause |
| access-control admin | all three | platform multisig | grant and revoke the roles below |
| `pauser` | `offer-token`, `offer-sale` | platform multisig | pause only, never unpause |
| `upgrader` | all three | platform multisig | replace the contract's code |
| `controller` | `offer-allowlist` | platform hot key, and each offering's `offer-sale` | write allocations, consume and restore them, reset consumption, grant `controller` |

The issuer's roles are stored addresses, not OpenZeppelin's access-control admin. OpenZeppelin always lets its top admin grant and revoke any role, so an issuer holding it could grant itself `upgrader` or remove the platform's `pauser`.

Two choices here deliberately differ from what a reader might expect.

- **Upgrades are held by the platform, not by the offering's admin.** In Base, an issuer's Safe owns its offering, but the code changes through the factory's beacon, which only the platform Safe controls. Soroban has no beacon: every contract replaces its own code. Keeping the split means a fix doesn't need a signing ceremony from every issuer.
- **The platform can pause any offering, but only the issuer unpauses.** In Base, only the offering's owner pauses, so an emergency waits on the issuer's officers.

`controller` is its own admin role, so a controller can grant and revoke it. That is parity with Base's `setControllerFromController` (audit NF-01, accepted), and offering deployment depends on it.

**Recommended custody.** The decision is BWB's.

- Platform multisig: 2 of 3, at the account's medium threshold. Soroban authorization checks the medium threshold.
- Platform hot key: a single key, limited to `controller` and to submitting transactions. A leak lets an attacker change allocations and revoke the sale contracts' `controller` role. That freezes purchases and refunds until the platform multisig grants it back. It can't move funds.
- Treasuries: whatever threshold the organization configures.
- Testnet (Tranche 1): single keys for every role.

## 2. From Base to Stellar

The Base backend's layer, part by part, and what each part becomes.

| Part | Base today | Stellar | Verdict |
|---|---|---|---|
| Who signs, who submits | The investor's Privy wallet signs a UserOp hash and the platform relays it through Notus. A treasury's signers sign a Safe hash and the platform executes. | The investor or treasury signs a Soroban authorization, and BWB submits: pattern (B), recommended ([4.1](#41-two-patterns)). | Same shape |
| Deploying an offering | One atomic batch of 10 calls through factories, driven by the backend | T1: the repository's deploy script, and the backend only registers addresses. T2: one call to the L8 factory, driven by the backend ([5.1](#51-deploying-an-offering)). | Changes |
| Allowlist writes | `setAllocations` after KYC, `resetConsumed` monthly | Same calls. The cap is in USDC stroops, converted from BRL by the backend ([3](#3-premises)). | Same shape, new unit |
| Investing | `approve` + `buyWithBrla` | One `buy`. The investor's authorization covers the USDC transfer inside it. | Simpler |
| Finalize | `finalizeSale` | `finalize` | Same |
| Release and payout | One Safe batch: `releaseTokens` in chunks of 200, then one `withdrawBrla` per stakeholder | `release` in batches of at most 30 (refunds 20); `withdraw_payment` takes every payout in one call | Changes |
| Confirming an investment | Read `TokensPurchased` from the receipt | Read `purchased` from the transaction's events | Same rule, new format |
| Reading state | RPC reads; raw storage slots for the allowlist; a partial subgraph | Read functions through simulation (the allowlist gains getters), and events from RPC, which keeps them about 7 days | Changes |
| Failures | Revert strings, decoded by Notus | Numeric error codes in one block per contract ([7](#7-errors)) | Changes |
| Roles | Platform Safe, platform server wallet, the organization's Safe | Platform multisig, platform hot key, the organization's treasury (native multisig) | Same split |

Base's backend never calls cancellation, mark failed, refunds, cooling-off, pause or admin transfer. The contracts have them and the grant requires them, so [section 5](#5-flows) defines those flows for the first time.

## 3. Premises

The interface holds under these. Changing one is a design change, not a defect.

- **One payment asset.** Each offering's payment asset is a constructor parameter. In practice it is USDC for every offering. The allowlist cap is a single number in USDC stroops (7 decimals) shared across offerings, so a second payment asset needs a new design.
- **No exchange rates on chain.** Investor limits are in BRL (CVM 88). The backend converts the BRL limit into stroops when it writes an allocation, and recalculates at the monthly reset. In Base this needed no conversion because 1 BRLA = R$1. That no longer holds, and neither does "one real per quota".
- **Price.** The price per token in USDC stroops is fixed at deployment. A purchase must pay an exact multiple of it. The team and the issuer choose the price.
- **Token.** 0 decimals: one token is one quota. It doesn't rebase, and there is no burn or snapshot.
- **Fee on transfer.** The sale measures its USDC balance before and after each incoming transfer and credits what actually arrived, as in Base. What arrived must be an exact multiple of the price. USDC charges no fee today.
- **Batches.** At most 30 investors per release call, 20 per refund call, and 30 entries per allowlist batch. The limit is the 16 KB of events a transaction may carry, not storage writes ([6.4](#64-batch-sizes)).
- **Cooling-off.** An investor may undo their reservation and get the payment back within the offering's window, counted from their last purchase, both while the offering is Active and after it succeeds, until their tokens are released. **Tokens are never released to an investor whose window is still open.** This differs from Base, where anyone could release early and end the right (audit SCAN-01, recommended fix). The audit response is treated as the written request for this change.

## 4. Invocation and authorization

### 4.1 Two patterns

The contracts are identical under both. Each user-facing call authorizes its investor itself, and a multisig treasury authorizes exactly like a single-key wallet. **Recommended: (B).** The table below gives the reasons.

- **(A) The investor is the transaction source and BWB fee-bumps.** The investor signs the whole transaction. BWB wraps it in a fee-bump transaction that pays the fee.
- **(B) BWB is the transaction source and the investor signs only an authorization entry.** BWB builds, simulates and submits the transaction. The investor signs a 32-byte hash of "this call, with these arguments".

| | (A) investor source + fee-bump | (B) BWB source + authorization entry |
|---|---|---|
| What the investor signs | The inner transaction: resources, resource fee, sequence number | Only the authorization: network, nonce, expiry ledger, invocation tree |
| Retry or re-simulation after signing | Needs a new signature. A fee-bump can't change the inner resource fee. | BWB rebuilds freely. The signature stays valid until it expires or the transaction succeeds. |
| Sequence number | The investor's, consumed even on failure | BWB's |
| Investor account must exist | Yes | Yes. The host loads the account to check signatures. |
| Investor needs XLM | No | No |
| Treasury multisig | Signatures are bound to the treasury's sequence number and go stale if it moves | Officers sign the same hash independently, at any time before expiry |
| Transaction hashes | Two: outer and inner. An inner failure shows as `txFEE_BUMP_INNER_FAILED`, with the inner hash in the result. | One |
| Cost | The fee-bump counts as one more operation | Lower |
| What the wallet must sign | A 32-byte transaction hash, ed25519 | A 32-byte hash, ed25519 |

Privy supports Stellar at its Tier 2: it signs a raw 32-byte hash with ed25519, server side and client side. Either pattern fits. It does not broadcast or sponsor on Stellar.

**Why (B).**

- **It mirrors Base.** There, the investor signs a UserOp hash and the platform relays; treasury officers sign offline and the platform executes.
- **The signature covers no fee or sequence number.** BWB can retry, re-simulate and adjust fees without asking the investor again.
- **Treasury officers can sign the same hash independently** until it expires, with no stale signatures.
- **It is cheaper**, with one transaction hash instead of two.

Pattern (A) stays documented for the case where a wallet can only sign whole transactions.

### 4.2 The purchase authorization tree

`buy(investor, amount)` authorizes `investor` first, then moves USDC from the investor to the sale. The investor's authorization entry is therefore:

```
offer-sale.buy(investor, amount)
└── usdc.transfer(investor, offer-sale, amount)
```

Simulation records this tree. The backend checks that the tree's root is the offering's sale contract before asking for a signature. A tree rooted at `usdc.transfer` alone would authorize a bare payment not tied to any offering.

The sale's call into the allowlist (`consume`) needs no authorization entry. The allowlist authorizes the sale contract as the direct caller.

### 4.3 Signing rules

- **The payload** is `sha256` of the XDR `HashIdPreimage::SorobanAuthorization { networkID, nonce, signatureExpirationLedger, invocation }`. It has no sequence number, fee or resources in it. Changing any argument of the call invalidates the signature.
- **Expiry** is the authorization's `signatureExpirationLedger`. Ledgers close about every 5 seconds, so Base's 30-minute approval window is about 360 ledgers. The maximum is about 180 days.
- **Nonce:** any `i64`, consumed only when the transaction succeeds. A signed authorization can be resubmitted until it succeeds or expires. Each authorization writes a short-lived nonce entry, paid by the submitter.
- **Multisig treasury:**
  - the signature is a list of `{public_key, signature}`, sorted strictly by public key, at most 20 entries;
  - the signers' weights must reach the treasury's medium threshold;
  - signers and weights are checked when the transaction runs, not when the signature is made.
- **Simulation:**
  - the first simulation records authorizations without checking signatures, so it under-counts CPU;
  - after attaching signatures, re-simulate in enforcing mode;
  - each signature costs about 0.42 M instructions, against 400 M per transaction.
- **Calls by BWB keys** (the hot key or the platform multisig) use the same mechanism. Under (B), a hot-key call can use source-account credentials, because the hot key is the transaction source.

## 5. Flows

Each step names its signer. The per-contract documents list each call's preconditions and errors.

### 5.1 Deploying an offering

**Who deploys.**

- **In Tranche 1, offerings are deployed by the repository's deploy script, not by the backend.** The backend doesn't automate deployment in T1. It registers the addresses the script records in [`deployments/testnet.json`](../deployments/testnet.json).
- **From Tranche 2, the factories (L8) deploy and wire an offering in one call.** The backend's automated deployment flow is built then, on top of the factory, and replaces the script.
- This saves the team from building a multi-transaction flow that the factory would retire three months later.

**Why it isn't one transaction in T1.** Soroban runs one contract call per transaction. Batching, as Base does through the Kernel smart account, needs a contract to do it, and that contract is the L8 factory. Without it, the script runs a sequence of transactions.

**Addresses.** A contract's address is `sha256(networkID, deployer, salt)`. It doesn't depend on the code or the constructor arguments, so every address is computed before anything is deployed. That replaces Base's prediction from the factory nonce, which breaks when two deployments race.

The script's sequence:

1. **Compute addresses.** One salt for the token and one for the sale. Both addresses are derived from the deployer, the platform hot key.
2. **Deploy `offer-token`.** Signed by the platform hot key. The constructor sets:
   - `admin` and `xfer_admin`: the issuer's treasury;
   - `pauser` and `upgrader`: the platform multisig;
   - name, symbol, supply and offering URI;
   - the initial transfer whitelist, which includes the precomputed sale address.
3. **Deploy `offer-sale`.** Signed by the platform hot key. The constructor sets:
   - owner: the issuer's treasury;
   - `pauser` and `upgrader`: the platform multisig;
   - the token, the payment asset, the price, the allowlist;
   - the configuration: hard cap, cooling-off seconds, minimum success percent.
4. **Grant `controller` to the sale** on the allowlist. Signed by the platform hot key, as a controller.
5. **Activate the sale.** The sale checks that it holds the whole supply, then moves from Preparing to Active.

Ownership is final from the constructors, so there is no transfer of ownership afterwards.

**Requirements on inventory** (open, decided in L2/L4):
- no key ever holds the offering's tokens;
- deployment adds no signature for the issuer.

In Base the issuer signs nothing on chain at deployment; the platform wallet does every step. The leading candidate is for the token's constructor to mint the whole supply straight to the sale's precomputed address. Who triggers activation follows from that choice. The same requirements apply to the L8 factory.

**Retrying a step.** The script is re-runnable. Deploying to an address that is already taken fails, so a retried step first checks whether its contract exists. If step 3 fails after step 2 succeeded, the tokens wait at the sale's address until step 3 is retried with the same salt.

### 5.2 Allowlist

1. **KYC approved.** The platform hot key calls `set_allocations` with the investor's address, the cap in stroops (converted from the BRL limit) and `enabled = true`. Repeating the same call is safe.
2. **Before each purchase**, the backend may read `allocation` or `remaining` instead of reading storage.
3. **Monthly**, the platform hot key calls `reset_consumed` for investors with confirmed purchases, in batches of at most 30, and recalculates caps if the exchange rate moved.

### 5.3 Investing

1. The backend simulates `buy(investor, amount)`. `amount` is in stroops and must be an exact multiple of the price.
2. The investor (wallet) or the officers (treasury) sign the authorization ([4](#4-invocation-and-authorization)).
3. The transaction is submitted under the chosen pattern.
4. The purchase is **confirmed by the `purchased` event**, never by the transaction status alone, as in Base. A purchase rejected for eligibility, allocation, supply or hard cap fails the transaction with an error code and emits nothing ([7.3](#73-rejections-as-evidence)).

### 5.4 Closing an offering

The issuer's treasury signs one of these, while the offering is Active:

| Call | When | Result |
|---|---|---|
| `finalize` | The minimum success percent is sold, or everything is sold | Successful |
| `mark_failed` | The minimum is not sold | Failed |
| `cancel` | Any time | Failed |

### 5.5 After success

- **Release.** Anyone may call `release(investors)`, in batches of at most 30. It delivers each investor's reserved tokens and ends the reservation.
  - An investor with nothing reserved is skipped silently.
  - An investor whose cooling-off window is still open fails the whole call. The backend leaves those investors out of the batch, since it knows each last purchase from the `purchased` events.
  - A failed token transfer also fails the whole call, as in Base.
- **Payout.** The issuer's treasury signs one `withdraw_payment(payouts)` call with every recipient and amount: issuer, distributor, platform, tokenizer. In Base this was one Safe batch with one withdrawal per stakeholder; one call keeps it to one signing ceremony.
- **Only released money can be withdrawn.** `withdraw_payment` takes at most the payment of reservations already released, minus what was withdrawn before. Money an investor can still ask back never leaves the sale. The order is: release everyone past their window, then withdraw.
  - This differs from Base, where the issuer could withdraw everything at once (audit NF-04, accepted).
  - Base's backend got away with it by releasing every investor in the same batch as the withdrawal. That silently ended any cooling-off right still open.
- **Leftover tokens** stay locked in the sale, as in Base. That covers tokens returned through cooling-off (audit SCAN-02) and tokens never sold, because withdrawing tokens requires Failed. This may change in L4.

### 5.6 After failure

- **Refunds.** Anyone may call `refund(investors)`, in batches of at most 20. Each investor gets their payment back, the reservation ends, and the allowlist room is restored.
  - **Known issue carried from Base:** restoring room fails when the monthly reset already set the investor's usage to 0. A refund after a reset therefore fails, together with its whole batch. The same happens to cooling-off. See [offer-allowlist](offer-allowlist.md#calls); it is decided in L3.
  - An investor with nothing reserved is skipped silently.
  - A failed USDC transfer fails the whole call, as in Base. A typical cause is an investor account that lost its USDC trustline or was deauthorized by the issuer. The backend leaves that investor out and retries the rest.
- **Tokens back.** The issuer's treasury may withdraw the offering's tokens with `withdraw_tokens`.

### 5.7 Cooling-off

The investor signs `cooling_off_refund(investor)` themselves, within the window, while the offering is Active or Successful and before their tokens are released. They get their payment back, the reservation ends, and the allowlist room is restored.

### 5.8 Pause, admin transfer and upgrade

- **Pause.** The issuer's treasury or the platform multisig calls `pause(caller)` on the token or the sale. Only the treasury calls `unpause(caller)`. Each call emits `pause_changed` with the caller.
  - A paused token blocks `transfer` and `transfer_from`, so release is blocked too.
  - A paused sale blocks `buy`. Refunds and cooling-off stay available.
- **Admin transfer.** The issuer's treasury (`xfer_admin`) moves tokens between any two accounts, even while paused, for court orders, lost keys or estates.
- **Upgrade.** The platform multisig calls `upgrade` on each contract, one call per contract. The contract emits `upgraded` with the new code hash and storage schema version. Any data migration runs in a separate, versioned call.

## 6. Events

### 6.1 Rules

- **Shape.** Every event has **topics** (its name, then at most 3 more) and **data** (a map of named fields, sorted by name). Four topics is the most the RPC can filter on.
- **Emitter.** Every event carries the address of the contract that emitted it. A sale event therefore identifies its offering. Allowlist events name the sale contract that caused them when a sale did.
- **No optional fields.** Every field is always present. The sdk 28 upgrade drops empty optional fields from events, so consumers also treat a missing field and a null field the same.
- **Additive rule.** New fields may appear in an event at any time, and consumers ignore fields they don't know. This is the same rule SEP-41 sets for token events. A change that would break a consumer gets a new event name instead.
- **Amounts** are `i128`. Payment amounts are in stroops of the payment asset (USDC, 7 decimals). Token amounts are whole tokens (0 decimals). Events don't repeat decimals; the backend registry holds them per deployment.
- **Timestamps** are ledger close times in seconds.
- **Upgrade event.** Every contract emits `upgraded { new_wasm_hash, schema_version }` on upgrade. Stellar also emits a system event, `executable_update`, which has no version.

### 6.2 Events the contracts inherit

From OpenZeppelin 0.7.2, on `offer-token` and wherever the library is used:

| Event | Topics | Data |
|---|---|---|
| `transfer` | `transfer`, from, to | the amount as a bare `i128` |
| `transfer` to a muxed address | `transfer`, from, to | `{amount, to_muxed_id}` |
| `mint` | `mint`, to | `{amount}` |
| `approve` | `approve`, owner, spender | `{amount, live_until_ledger}` |
| `paused` / `unpaused` | `paused` / `unpaused` | `{}` |
| `role_granted` / `role_revoked` | name, role, account | `{caller}` |

The USDC asset contract emits differently. Its `transfer` has a fourth topic with the asset, `USDC:G…`, and its data is a bare `i128`. **A consumer of payment events handles transfers with 3 topics (OpenZeppelin) and with 4 (USDC), and data as a bare amount or as a map.**

### 6.3 Ingestion

The RPC keeps events for 120,960 ledgers, about 7 days, on both networks. The backend has to ingest continuously from a stored cursor, and can't rely on RPC history.

A filter takes at most 5 contract IDs, and a request takes at most 5 filters. With many offerings, the backend either pages over contract IDs, or filters by topic and keeps only contracts in its registry.

### 6.4 Batch sizes

A transaction may carry at most 16,384 bytes of events, including the call's return value. Measured sizes:

| Event | Size |
|---|---|
| OpenZeppelin `transfer` | 172 B |
| USDC `transfer` | 244 B |
| A two-field event with one address topic | about 184 B |

Each investor in a batch emits several events. These per-investor figures are estimates, to be confirmed by simulation in L4:

| Call | Events per investor | Per investor | Limit | Total at the limit |
|---|---|---|---|---|
| `release` | token `transfer` + `released` | about 370 B | 30 | about 11.1 KB |
| `refund` | USDC `transfer` + `refunded` + allowlist `allocation_restored` | about 730 B | 20 | about 14.6 KB |
| `set_allocations`, `reset_consumed` | one allowlist event | about 200 B | 30 | about 6 KB |

Each investor keeps their own event because the backend's ledger records one movement per investor.

## 7. Errors

### 7.1 Codes

A failure returns `Error(Contract, #n)`. Each contract has its own block, clear of every OpenZeppelin range:

| Contract | Block |
|---|---|
| `offer-token` | 6000–6099 |
| `offer-allowlist` | 6100–6199 |
| `offer-sale` | 6200–6299 |

An error raised by a contract the call passes through comes back unchanged. A failed `buy` can carry an allowlist code, a token code, an OpenZeppelin code or a USDC code. Codes the backend will meet from OpenZeppelin:

| Code | Name | Meaning |
|---|---|---|
| 100 | `InsufficientBalance` | the sender doesn't hold enough |
| 101 | `InsufficientAllowance` | `transfer_from` beyond the allowance |
| 103 | `LessThanZero` | negative amount |
| 1000 | `EnforcedPause` | the contract is paused |
| 1001 | `ExpectedPause` | unpausing a contract that isn't paused |
| 2000 | `Unauthorized` | the caller lacks the role |
| 2007 | `RoleNotHeld` | revoking a role the account doesn't hold |

A missing or invalid signature is not a contract code. It fails as `Error(Auth, …)` before the contract decides anything.

### 7.2 Reading a failure

- **Before submitting:** simulation returns the error code. This is where most rejections should be caught.
- **After submitting:** a failed transaction's result only says `INVOKE_HOST_FUNCTION_TRAPPED`. The code is in the transaction's **diagnostic events**, which RPC returns as `diagnosticEventsXdr` when the node keeps them. Explorers show them. Diagnostic events also say which contract raised the error.

### 7.3 Rejections as evidence

Eligibility and allocation rejections emit no event: the transaction fails, lands on the ledger and pays its fee. The evidence for the grant (D2, criterion 2) is the failed transaction's hash plus the documented code:

- `6100 InvestorNotEnabled` for eligibility;
- `6101 AllocationExceeded` for allocation.

This proof is practical, not consensus-level. Diagnostic events are not part of the ledger's hash, and only nodes that keep them return them. The demonstration script at the end of L4 builds these failing transactions with a hand-made footprint, because simulation of a failing call returns none.

## 8. Grant vocabulary and AUM inputs

### 8.1 The grant's terms

| D2 term | Events |
|---|---|
| Issuance | `offer-token` `mint` at construction; `offer-sale` `activated` |
| Allocation | `offer-allowlist` `allocation_set`, `allocation_consumed`, `allocation_restored`, `consumed_reset` |
| Investment | `offer-sale` `purchased` |
| Pause / unpause | `paused` / `unpaused` on `offer-token` and `offer-sale` |
| Cooling-off | `offer-sale` `refunded` with reason `cooling_off` |
| Cancellation | `offer-sale` `cancelled` or `marked_failed`, each with `state_changed` Active → Failed |
| Refund | `offer-sale` `refunded` with reason `failed` |
| Settlement | `offer-sale` `released` (tokens to the investor) and `payment_withdrawn` (payment to the recipients) |
| Eligibility rejection | failed transaction, code 6100 |
| Allocation rejection | failed transaction, code 6101; also 6207 (over supply) and 6208 (over hard cap) |

### 8.2 What moves value

The AUM query and its definition belong to the team, for example whether money still in escrow counts. These are the events that move value, and what each one does:

| Event | Effect |
|---|---|
| `mint` (token) | Supply created. Held by the sale until released. |
| `purchased` | + payment into escrow, + tokens reserved for the investor |
| `refunded` | − payment from escrow, − tokens reserved |
| `released` | Tokens move from reservation to the investor's balance. Escrow is unchanged. |
| `payment_withdrawn` | − payment from escrow, to the recipient |
| `transfer` (token), after release | Holdings change hands: whitelisted transfers, or admin transfers |
| `tokens_withdrawn` | Tokens leave a failed offering, back to the issuer |

No event carries personal data: only addresses and amounts.

## 9. Storage and expiry

On Stellar, contract data has a lifetime and must be extended or it is archived. Archived data is restored automatically on its next use, at extra cost to that transaction.

| Data | Storage | Where |
|---|---|---|
| Allocation per investor | persistent, one entry per investor | `offer-allowlist` |
| Reservation per investor | persistent, one entry per investor | `offer-sale` |
| Balance per holder | persistent (OpenZeppelin) | `offer-token` |
| Roles | persistent (OpenZeppelin) | all |
| State, configuration, totals, admin, pause flag | instance | all |

- **The contracts extend** their instance and every entry they touch, on every call. OpenZeppelin extends balances and roles but never the instance, so the contracts do it.
- **There is no backend keep-alive job.** Anyone may extend any entry without permission (`ExtendFootprintTTLOp`), so the team can add one later without contract changes.
- **Lifetimes.** On mainnet a new entry lives at least about 120 days. On testnet only about 7, so idle testnet data archives quickly.
- **Persistent structs carry a schema version.** The full TTL policy is L9.

## 10. What the backend registry needs

The Base application's registry knows Stellar mainnet only. For Tranche 1 on testnet it needs:

- **The testnet chain.** Its id is the network ID, `cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472`, the SHA-256 of `Test SDF Network ; September 2015`.
- **The USDC testnet deployment:**
  - contract `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA`;
  - asset `USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5`;
  - 7 decimals.
- **The allowlist** and each offering's token and sale contract, from [`deployments/testnet.json`](../deployments/testnet.json).

Testnet resets periodically, so these contract IDs change with every redeploy.

## 11. Open points

Decided later. Each one changes this document through the changelog.

| Point | Today | Decided in |
|---|---|---|
| Inventory at deployment, and who activates | Mint straight into the sale is the leading candidate | L2 / L4 |
| Batch sizes (30 release, 20 refund) | Estimates with margin. L4 measures real event sizes and raises the limits as far as they fit, for example by trimming per-investor events. The backend needs a job that releases investors in rolling batches as their windows close. | L4, and the team for the job |
| Batch behaviour when one investor can't be served | Revert the whole call, as in Base; the alternative is skip and report | L4 |
| Leftover tokens after success | Locked, as in Base | L4 |
| Restoring allowlist room after the monthly reset | Fails, as in Base. The likely fix is to restore at most what is consumed. | L3 |
| What a paused sale blocks | `buy` only | L4 |
| Definition of AUM | Inputs listed in [8.2](#82-what-moves-value) | The team |
| Custody of each key | Recommendation in [1](#1-parties-roles-and-keys) | BWB |

## 12. Risks

- **soroban-sdk 26.x is outside the SDK's security window.** The SDK patches only its two newest major versions, which are 27 and 28. The contracts stay on 26.1 because OpenZeppelin 0.7.2, the latest stable release, requires it. The no-optional-fields rule and the versioned persistent structs keep the move to sdk 28 cheap.
- **Both networks run protocol 29.** Limits quoted here were read live on 2026-10-02 and can change by network vote. The current values are at [lab.stellar.org/network-limits](https://lab.stellar.org/network-limits).

## Changelog

**0.3, in progress.**

- Pattern (B) is recommended; pattern (A) stays documented.
- Batch sizes are marked for review in L4, with the backend's rolling-release job as team work.
- In Tranche 1, offerings are deployed by the repository's script and the backend only registers addresses. From Tranche 2 the backend deploys through the L8 factory.

**0.2, 2026-10-02.** Changes after a review against Base's contracts:

- **Roles and pause:**
  - issuer roles are stored addresses, and the platform holds the access-control admin;
  - `pause` and `unpause` take a caller and emit `pause_changed`;
  - controllers can revoke as well as grant.
- **Withdrawals:** only released money can be withdrawn (NF-04).
- **Known issues:** restoring allowlist room after the monthly reset fails, as in Base; carried as a known issue.
- **Token:** the initial holder is whitelisted automatically, and snapshot is removed.
- **Sale:**
  - fee-on-transfer credits what arrives;
  - finalize checks that the sold tokens are still held;
  - the supply is read from the token;
  - `purchased` and `refunded` carry the offering's totals;
  - a new `configured` event;
  - more read functions.
- **Allowlist:** `remaining` never goes negative, and a cap of 0 is valid.
- **Events:** batch sizes re-estimated.

**0.1, 2026-10-02.** First draft.
