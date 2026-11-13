# L6 push distributor: research notes

| | |
|---|---|
| Date | 2026-11-12 |
| For | Issue [#28](https://github.com/BWB-Labs/bwb-stellar/issues/28), the L6 design interview |
| Ports | `bwb-tokenization/contracts/src/TokenDistributor.sol` (108 lines) |
| Outcome | The design in [offer-distributor.md](../offer-distributor.md) and ADR 0009: pull-at-call funding, all-or-nothing, one persistent marker per id, 30 payouts with one event each |
| Stack checked | soroban-sdk 26.1.1, soroban-env-host 26.1.4, OpenZeppelin Stellar 0.7.2, mainnet protocol 29, testnet protocol 30 |

These notes hold the facts the L6 decisions depend on. They decide nothing. Every claim names its source. Anything I could not confirm from a primary source is listed in [Not verified](#not-verified).

## What this means for L6

1. The sale's `withdraw_payment` already pays up to 30 recipients in one owner-signed call from the sale's own USDC. L6 adds something only when the payer is not the sale, when there are more than 30 recipients, or when a distribution needs an id that cannot be paid twice. Classic operations cannot move the sale's USDC at all.
2. A bad recipient fails the SAC `transfer` with a contract error and nothing else: 13 for a missing trustline or a missing account, 11 for a deauthorized balance, 10 for a trustline at its limit. The distributor cannot create a trustline for a recipient, because the SAC's `trust` needs the recipient's signature.
3. Skip-and-report works. `try_transfer` rolls back the failed sub-call's storage, events and authorization, and the batch goes on. Two limits: codes 10 and 11 do not say whether the sender or the recipient failed, and a recipient that changes state between simulation and execution can fail the whole transaction, never leave it half applied.
4. Events bind the batch size: 16,384 B per transaction on both networks. A USDC transfer to a G account is 244 B. With one own event per leg the ceiling is 39 and 30 keeps the sale's margin. With one summary event per batch the ceiling is 66 and 50 keeps a 25% margin. Base's 200 does not fit in one transaction either way.
5. Mainnet and testnet have the same per-transaction limits, read live today: 200 write entries, 200 disk-read entries, 400 footprint entries, 400M instructions. The SDK's bundled "mainnet" table (50, 100, 600M) is stale, so the risk note in interface §12 can be closed.
6. A pre-funded distributor pays with no signature from whoever funded it. Pull-at-call funding needs the treasury's authorization entry over `distribute` and the nested `usdc.transfer(treasury, distributor, total)`, at the account's medium threshold, with at most 20 signers.
7. A persistent marker per distribution is durable: an archived entry can be restored but never re-created. A temporary marker is deleted at expiry and the guarantee goes with it. A 100-byte persistent marker costs about 0.017 XLM of rent per 120 days.
8. Nobody ships an audited push distributor for Soroban. OpenZeppelin has a Merkle claim distributor (pull). soroban-examples has an unaudited Merkle claim and a batching example that tries and skips. SDF's Disbursement Platform batches off chain, one payment per transaction.
9. OpenZeppelin released 0.7.3 and 0.8.0 on 2026-10-20. 0.8.0 builds on soroban-sdk 28. The repo's "0.7.2 is the latest stable release" is out of date. Issue #28 also asks for "ownable/admin via OpenZeppelin" while ADR 0005 uses stored addresses, so one of the two has to give.
10. L6 takes error block 6300–6399 and the standard package: a powers struct, `pause(caller)` and `unpause(caller)`, `upgrade` with a fixture test, `schema_version`, `binver`, the TTL top-up, tests in D2 modules, `scripts/deploy.sh` and `deployments/testnet.json`.

## How to read the sources

Paths are shortened with these prefixes. All of them are local copies of the exact versions in `Cargo.lock`.

| Prefix | Path |
|---|---|
| `HOST` | `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/soroban-env-host-26.1.4/src` |
| `SDK` | `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/soroban-sdk-26.1.1/src` |
| `OZ` | `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/stellar-contract-utils-0.7.2/src` |
| `XDR` | `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/stellar-xdr-26.0.1/src/curr/generated.rs` |
| `SAC` | `HOST/builtin_contracts/stellar_asset_contract` |

The networks run newer hosts than the pinned SDK: mainnet protocol 29, testnet protocol 30 (RPC `getNetwork` on both, 2026-11-12). I compared the files the SAC answers rest on (`contract_error.rs`, `balance.rs`, `contract.rs`, `host/error.rs`) with `stellar/rs-soroban-env` main at commit `c9de2ed` (2026-11-12, after v30.0.0). They are byte-identical to 26.1.4. `host/frame.rs` changed, and its rollback code is the same in main (lines 45–49, 267–268, 607–609).

Official docs were read from the `stellar/stellar-docs` source at commit `ea6f061` (2026-11-12), which builds developers.stellar.org.

"Measured" means I ran it in the soroban-sdk 26.1.1 test host against a throwaway Wasm contract that loops over SAC transfers, outside this repo. The method is in [Measurements](#measurements). The numbers are test-host numbers, not network numbers.

## 1. What a SAC transfer does with a bad recipient

`transfer(from, to, amount)` runs in this order: reject a negative amount, strip the mux id from `to`, `from.require_auth()`, debit the sender, credit the recipient, emit the event. Source: `SAC/contract.rs:206-226`.

Both the debit and the credit first ask whether the balance is authorized, then move it. Source: `SAC/balance.rs:100-107` (credit), `220-230` (debit).

All SAC errors are `Error(Contract, #n)` from one shared enum. Source: `HOST/builtin_contracts/contract_error.rs:9-30`. The official docs list the same codes: <https://developers.stellar.org/docs/tokens/stellar-asset-contract>.

| Recipient | Result | Why | Source |
|---|---|---|---|
| G account, authorized trustline, room left | paid | The trustline entry is credited, as a classic payment would | `SAC/balance.rs:584-628` |
| G account, no USDC trustline | `Error(Contract, #13)` `TrustlineMissingError` | The authorization check reads the trustline entry and finds none | `SAC/balance.rs:565-581`, `811-842`. Measured |
| G address with no account on the ledger | `Error(Contract, #13)`, the same code | For an issued asset the SAC never loads the account entry, only the trustline key. `AccountMissingError` (#6) appears only for native XLM and inside `trust` | `SAC/balance.rs:376-404`, `483-491`, `1019-1025`. Measured |
| Trustline exists, not authorized | `Error(Contract, #11)` `BalanceDeauthorizedError` | Only `AUTHORIZED_FLAG` counts | `SAC/balance.rs:101-107`, `835-842`. Measured |
| Trustline with only `AUTHORIZED_TO_MAINTAIN_LIABILITIES` | `Error(Contract, #11)` | Same check. This is the state a SAC `set_authorized(false)` leaves | `SAC/balance.rs:898-911`. Measured both ways |
| Trustline at its limit | `Error(Contract, #10)` `BalanceError` | The new balance must stay at or under `limit − buying liabilities` | `SAC/balance.rs:604-626`, `756-783`. Measured |
| Amount over `i64::MAX` to a G account | `Error(Contract, #12)` `OverflowError` | Trustline balances are 64-bit | `SAC/balance.rs:111-117`. Not measured |
| Muxed address (M...) | paid to the underlying G account | The SAC strips the id and applies the same trustline rules. The event's data becomes `{amount, to_muxed_id}` | `SAC/contract.rs:209-224`, `SAC/event.rs:68-93`. Measured |
| Contract (C), first payment | paid | The SAC creates a persistent `Balance(addr)` entry under the USDC contract. No trustline, no opt-in | `SAC/balance.rs:120-145`. Measured |
| Contract (C), deauthorized by the issuer | `Error(Contract, #11)` | The balance entry carries its own `authorized` flag | `SAC/balance.rs:236-245`. Measured |
| Contract (C), issuer has `AUTH_REQUIRED`, no entry yet | `Error(Contract, #11)` | Does not apply to USDC today, see below | `SAC/balance.rs:243-245`. Measured |
| The USDC issuer account | succeeds, emits `burn` | No trustline is touched | `SAC/balance.rs:384-386`, `SAC/event.rs:48-66`. Measured |
| Negative amount | `Error(Contract, #8)` | Checked first | `SAC/contract.rs:32-43`, `213` |

The same codes come from the sender's side. If Circle deauthorizes the distributor's own balance, every leg fails with 11. If the distributor is short, the leg fails with 10. Source: `SAC/balance.rs:180-187`, `220-229`. So 10 and 11 alone do not tell a contract which side is at fault. The SAC has an `authorized(addr)` read, outside SEP-41, that a contract can call on itself. Source: `SAC/contract.rs:196-203`.

The repo's interface document says 13 for a lost trustline, 11 for a balance Circle deauthorized and 10 for not enough balance (`docs/interface.md:397`, `702-709`). That holds. Two additions: 13 also covers an account that does not exist, and 10 also covers a recipient at its trustline limit.

**The distributor cannot open a trustline for a recipient.** Since protocol 26 the SAC has `trust(addr)`, which creates an unlimited trustline. It calls `addr.require_auth()` when it actually creates one, sponsorship is not supported, and the account must hold the extra base reserve. Source: `SAC/balance.rs:951-1089` (the `require_auth` is line 1002), CAP-73 <https://github.com/stellar/stellar-protocol/blob/master/core/cap-0073.md>, and the SAC docs page above.

**USDC's flags, read live.** Circle's issuer `GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN` has `auth_required: false`, `auth_revocable: true`, `auth_clawback_enabled: false`, `auth_immutable: false`, thresholds 2/2/2. The testnet issuer `GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5` has the same four flags. Source: Horizon `/accounts/<issuer>` on both networks, 2026-11-12. Circle lists the two issuers at <https://developers.circle.com/stablecoins/usdc-contract-addresses> and says an account needs a USDC trustline before it can hold USDC at <https://developers.circle.com/stablecoins/quickstarts/setup-usdc-trustline-stellar>. Consequences that follow from the source above: a contract recipient needs no prior authorization, any balance can be frozen, nothing can be clawed back today, and Circle can still change the flags because the account is not immutable.

**The SAC ids.** `stellar contract id asset` gives `CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75` for mainnet USDC and `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA` for testnet, which matches `docs/interface.md:751`. Circle's page does not publish a Stellar contract id.

**Muxed recipients need the right argument type.** Only `transfer` takes a `MuxedAddress`. A function that declares `Address` rejects an M-address, and the host rejects a muxed address inside a storage key. Source: the SAC docs page, section on muxed addresses, and CAP-67 <https://github.com/stellar/stellar-protocol/blob/master/core/cap-0067.md>. The SDK's token client declares `to: MuxedAddress`. Source: `SDK/token.rs:154`.

## 2. Surviving one failed transfer

**What rolls back.** Every contract call pushes a frame and saves a rollback point: a copy of the storage map, the length of the event buffer, and a snapshot of the authorization manager. If the call returns an error, the host restores the storage map, marks every event emitted since as coming from a failed call, and restores the authorization snapshot. Source: `HOST/host/frame.rs:44-48` (the rollback point), `190-205` (saved on push), `210-227` (restored on pop), `538-545` (pop with rollback on error).

**Events from a failed sub-call are dropped.** The host keeps them in the buffer flagged `failed_call`, then leaves them out of both the transaction's events and the event-size count. They survive only as diagnostic events with `in_successful_contract_call: false`. Source: `HOST/events/internal.rs:197-205`, `HOST/e2e_invoke.rs:836-846`, `875-885`, `892-900`.

**Authorization is un-done too.** A nonce consumed inside the failed sub-call comes back with the storage rollback. Source: `HOST/auth.rs:2198` (comment), `2279-2293` (nonces live in temporary storage).

**What `try_` catches.** `try_call` turns a recoverable error into a return value. A contract error code passes through unchanged. Any other recoverable host error becomes `Error(Context, InvalidAction)`. Source: `HOST/host.rs:2612-2671`. The SDK returns it as `Err(Ok(error))`, or `InvokeError::Contract(code)` and `InvokeError::Abort`. Source: `SDK/env.rs:431-453`, `SDK/error.rs:8-31`.

**What `try_` does not catch.** Running out of CPU or memory, touching a ledger entry outside the footprint, and internal host errors. Those trap the caller and the transaction fails. Source: `HOST/host/error.rs:148-167`, and the docs at <https://developers.stellar.org/docs/learn/fundamentals/contract-development/contract-interactions/overview#recoverable-and-non-recoverable-errors>.

**Measured.** One `distribute_try` call over 11 recipients, 7 of them bad in the ways listed in section 1, returned the 7 codes, paid the other 4, moved exactly 4 × amount out of the distributor, and left 15 events: 4 SAC transfers or burns, 4 own "paid" events and 7 own "skipped" events. No SAC event from a failed leg survived. The plain `transfer` version of the same batch failed whole with `Error(Contract, #13)` and moved nothing.

**Cost.** A failed leg costs about the same CPU as a paid one: 30 legs that all fail on a missing trustline used 4.82M instructions against 5.28M for 30 that all succeed. `try_transfer` itself adds about 3k instructions per leg over `transfer`. A failed leg adds no SAC event, so it is lighter on the event budget than a paid one. The worst case for limits stays a batch where every leg pays. Source: measured.

**The footprint consequence.** The footprint is fixed at simulation. Each recipient's trustline or balance key is in it as read-only or read-write depending on what simulation saw. Three cases follow from `HOST/storage.rs:130-154`:

- Simulation saw the leg succeed, the recipient loses the trustline before execution. The key is in the footprint, the read finds nothing, the SAC returns 13, `try_` catches it. The batch goes on.
- Simulation saw the leg fail, the recipient fixes the trustline before execution. The key is read-only in the footprint, the credit tries to write it, the host raises `Error(Storage, ExceededLimit)`. That is not recoverable. The whole transaction fails and is re-simulated.
- The recipient's contract balance entry is archived and missing from the restore list. The transaction fails at apply, before the contract runs. Source: <https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival>, section on transactions that touch an archived entry.

So drift can make a skip-and-report batch fail whole. It cannot make it pay a different set of people than the code decided on.

**Prior art for try-and-skip.** `soroban-examples/atomic_multiswap` batches swaps through `try_swap` and skips the ones that fail. Source: <https://github.com/stellar/soroban-examples/blob/main/atomic_multiswap/src/lib.rs>. The repo already does the same in `restore_or_report` (`contracts/offer-sale/src/lib.rs:725-737`) and keeps the opposite rule for payments: a failed USDC transfer fails the whole refund (`contracts/offer-sale/src/test/failure.rs:181-197`, `docs/interface.md:397`).

**What Base does.** `distributeTokens` is all-or-nothing through `safeTransfer`, with a pre-check that the balance covers the total. Source: `TokenDistributor.sol:80-106`.

## 3. How many transfers fit in one invocation

### Live limits

Read with `stellar network settings --network mainnet|testnet` on 2026-11-12. Mainnet was at ledger 65,445,530 on protocol 29, testnet on protocol 30.

| Limit | Mainnet | Testnet |
|---|---|---|
| `tx_max_instructions` | 400,000,000 | 400,000,000 |
| `tx_memory_limit` | 41,943,040 | 41,943,040 |
| `tx_max_write_ledger_entries` | 200 | 200 |
| `tx_max_disk_read_entries` | 200 | 200 |
| `tx_max_footprint_entries` | 400 | 400 |
| `tx_max_disk_read_bytes` | 200,000 | 200,000 |
| `tx_max_write_bytes` | 132,096 | 132,096 |
| `tx_max_size_bytes` | 132,096 | 132,096 |
| `tx_max_contract_events_size_bytes` | 16,384 | 16,384 |
| `contract_data_entry_size_bytes` | 65,536 | 65,536 |
| `max_entry_ttl` | 3,110,400 | 3,110,400 |
| `min_persistent_ttl` | 2,073,600 | 120,960 |
| `min_temporary_ttl` | 17,280 | 720 |

The two networks agree on every per-transaction limit. They differ only in minimum TTLs. These match `docs/interface.md` R7.

**The SDK's bundled table is stale.** `InvocationResourceLimits::mainnet()` in sdk 26.1.1 says 600M instructions, 50 write entries, 100 disk-read entries and 100 ledger entries, and its own comment says the values are not pulled live. Source: `SDK/testutils/cost_estimate.rs:133-152`. The test host also adds disk reads, memory reads and writes into its "total footprint ledger entries" check, so it counts every read-write key twice. Source: `HOST/host/invocation_metering.rs:396-405`, `1002-1010`. CAP-66 defines the real limit as the number of entries in the read-only and read-write footprints together. Source: <https://github.com/stellar/stellar-protocol/blob/master/core/cap-0066.md>, line 335. OpenZeppelin reached the same two conclusions in its own batch benchmark. Source: <https://github.com/OpenZeppelin/stellar-contracts/blob/v0.8.0/bench/rwa-batch/README.md>. The warning in `docs/interface.md:577` can be closed: mainnet is at 200 and 400.

### What one leg costs

Measured, one `transfer` per leg out of the contract's own balance, with the test SAC.

| Resource | G recipient | C recipient |
|---|---|---|
| Instructions | about 153k per leg, 163k with one own event | about 215k on first payment, 240k afterwards |
| Write entries | 1 trustline | 1 balance entry |
| Disk-read entries | 1 (a classic entry) | 0, plus 1 for the issuer account when any new balance is created |
| Write bytes | 116 | 224 |
| Rent | none | a new balance entry starts at the network minimum TTL, 120 days on mainnet, about 0.037 XLM at today's rate (224 B, formula in section 6) |
| SAC event, real USDC | 244 B | 240 B |

Fixed per call: about 0.32M instructions, 1 write entry for the distributor's own USDC balance. Extending the instance TTL writes no data entry.

The classification of trustlines as disk reads and live contract data as in-memory comes from CAP-66, lines 204–214, and `HOST/host/invocation_metering.rs:964-967`. The SAC itself extends a balance entry to 30 days when it writes it (`SAC/storage_types.rs:8-9`), which is below mainnet's 120-day minimum for a new entry and so adds nothing at creation.

**Event sizes.** The test SAC's asset string is 60 characters, USDC's is 61 and pads to 64 in XDR, so a real USDC event is 4 B larger than the measured one. I measured 240 B to a G account and 236 B to a contract, which gives 244 and 240 for USDC. That agrees with the 244 B in `docs/interface.md:657`. An account address encodes 4 B longer than a contract address, so any own event with the recipient in a topic is also 4 B larger for a G recipient. The repo pins `payment_withdrawn` at 168 B with a contract address as recipient (`contracts/offer-sale/src/test/events.rs:350-385`, where every party comes from `Address::generate`, which makes contract addresses, `SDK/address.rs:434-440`). To a G account it is 172 B. So a 30-leg `withdraw_payment` to G accounts emits 30 × 416 = 12,480 B, not 12,360. It still fits.

The limit counts the events and the return value together. Source: CAP-46-07 <https://github.com/stellar/stellar-protocol/blob/master/core/cap-0046-07.md>, lines 362–375.

### The arithmetic

Per-leg events to a G account with real USDC, against 16,384 B:

| Own events | Bytes per leg | Hard ceiling | With about 25% margin |
|---|---|---|---|
| One per leg shaped like `payment_withdrawn` (172 B) | 416 | 39 legs = 16,224 B | 30 legs = 12,480 B, 24% left |
| One per leg, the smallest possible (`paid`, recipient topic, amount: 156 B) | 400 | 40 legs = 16,000 B | 30 legs = 12,000 B |
| None per leg, one summary per batch (about 200 B) | 244 | 66 legs = 16,104 B + summary | 50 legs = 12,200 B + summary, 24% left |

Measured checkpoints with the test asset: 40 legs with a per-leg event emit 15,840 B and 41 emit 16,236 B, which becomes 16,400 B with USDC's longer string and is over. 67 legs with no own event emit 16,080 B, 16,348 B with USDC. 68 is over.

Every other limit is far away at these sizes:

| Limit | At 30 legs | At 66 legs | First breaks at |
|---|---|---|---|
| Instructions, of 400M | 5.2M | 11.4M | not before 198 legs, which use 46M |
| Memory, of 41.9 MB | 2.1 MB | 4.2 MB | 21.6 MB at 198 legs |
| Write entries, of 200 | 32 with a marker | 68 | 199 G legs plus a marker give 201 |
| Disk-read entries, of 200 | 30 | 66 | about 199 G legs |
| Footprint entries, of 400 | about 36 | about 72 | after write entries |
| Write bytes, of 132,096 | 3.8 KB | 8 KB | no |

A call signed through an authorization entry adds the signer's account entry as a disk read and a nonce entry as a write. Source: `docs/interface.md:479`, `HOST/builtin_contracts/account_contract.rs:197`, `HOST/auth.rs:2279-2293`. That puts the write-entry ceiling at about 196 legs.

**Transaction size, computed by hand and not measured.** One `{recipient, amount}` map entry is about 112 B of XDR. It appears in the call arguments and again in the signer's authorization entry. The recipient's trustline key adds 84 B to the footprint. That is about 310 B per leg, or about 400 legs in 132,096 B. Not binding.

**A defensible `MAX_BATCH`.** 30 if L6 emits one own event per recipient, the same shape and margin as `MAX_PAYOUTS`. 50 if L6 emits one summary event and lets the SAC's `transfer` events carry the legs. Past 66 nothing fits. Base's limit of 200 (`TokenDistributor.sol:86`) would need 200 × 244 = 48,800 B of SAC events alone, so a 200-recipient distribution is 4 calls at best and 7 at 30.

Skip-and-report changes little here. A skipped leg emits one own event (180 B measured with a `code` field) and no SAC event, less than a paid leg. A returned list of per-leg codes counts against the same 16,384 B, about 8 B per leg.

### How the sale derived its limits

The sale sized every batch from events first, then checked write entries. Source: `docs/interface.md:436-448`, `666-676`, `contracts/offer-sale/src/lib.rs:86-102`.

| Call | Per unit | Limit | Total | Margin |
|---|---|---|---|---|
| `release` | token `transfer` 168 B + `released` 188 B | 30 | 10,680 B | 35% |
| `withdraw_payment` | USDC `transfer` 244 B + `payment_withdrawn` 168 B | 30 | 12,360 B | 25% |
| `refund` | 2 units per investor + 1 per live purchase, 292 B per unit | 16 investors, 48 units | at most 14,016 B | 14% |

The refund limit is 16 because an investor with 20 purchases costs 22 units and three such investors already pass 48. The margins exist for the additive event rule, since a new field in a per-leg event grows every batch (`docs/interface.md:419`).

The sale's measuring tests lift the test host's limits and assert 200 write entries and 400 footprint entries themselves, for the reason given above. Source: `contracts/offer-sale/src/test/boundary.rs:70-82`. L6's boundary tests will need the same two lines.

## 4. The no-contract alternative

**100 operations, atomic.** A classic transaction holds 1 to 100 operations, and if one fails none applies. Source: <https://developers.stellar.org/docs/learn/fundamentals/transactions/operations-and-transactions>.

**One Soroban operation per transaction.** A transaction with `InvokeHostFunctionOp`, `ExtendFootprintTTLOp` or `RestoreFootprintOp` can have only that one operation, so a contract call cannot share a transaction with classic payments. Source: the same page, and <https://developers.stellar.org/docs/learn/fundamentals/contract-development/contract-interactions/stellar-transaction>.

**Multisig.** `Payment` is a medium-threshold operation. Each operation can name its own source account, and that account's signers must meet the threshold. A transaction carries at most 20 signatures. Source: <https://developers.stellar.org/docs/learn/fundamentals/transactions/list-of-operations#payment>, the operations page above, and <https://developers.stellar.org/docs/learn/fundamentals/transactions/signatures-multisig>. The officers sign the transaction hash, which includes the transaction source's sequence number. With BWB as transaction source and the treasury as operation source, BWB owns the sequence number, but a rebuilt transaction still needs new signatures. That is pattern (A)'s cost in `docs/interface.md:259-269`.

**Fees.** A fee-bump lets another account pay for an already signed transaction. Sponsored reserves cover base reserves, not fees. Source: <https://developers.stellar.org/docs/build/guides/transactions/fee-bump-transactions>, <https://developers.stellar.org/docs/build/guides/transactions/sponsored-reserves>.

**The restriction that matters.** Classic operations cannot move money a contract holds. A contract's balance of an issued asset is a contract data entry under the SAC, and the SAC is the only interface to it. Source: <https://developers.stellar.org/docs/tokens/stellar-asset-contract>. The `Payment` operation's source and destination are `MuxedAccount`, which is an ed25519 account with or without a mux id, never a contract. Source: `XDR:31247-31255`, `30764-30767`, `34903`. The sale's USDC leaves only through `usdc.transfer(from = sale, ...)`, and `from.require_auth()` on a contract address passes only when that contract makes the call. Source: `SAC/contract.rs:215`, `HOST/auth.rs:96-104`.

So the classic route is two steps. The sale pays its USDC to the treasury's G account with `withdraw_payment`, then the treasury signs one classic transaction with up to 100 payments.

**The same failure rules, with different codes.** A classic payment fails with `PAYMENT_NO_TRUST`, `PAYMENT_NOT_AUTHORIZED`, `PAYMENT_LINE_FULL` or `PAYMENT_NO_DESTINATION`, and one failed operation fails all 100. There is no skip. Source: the list of operations page.

**Contract recipients fall out.** A classic payment cannot pay a C address. That needs a SAC `transfer`, one per transaction. Source: <https://developers.stellar.org/docs/build/guides/transactions/send-and-receive-c-accounts>.

**Events.** Since protocol 23 a classic payment emits a `transfer` event in the SAC's format under the asset's contract address, in transaction meta. Source: CAP-67, section "New Events for Operations". CAP-67 also says these unified events are switched on by node configuration, so whether the L5 indexer sees them depends on the provider.

**Replay.** A classic transaction cannot apply twice because it consumes a sequence number. It has no notion of a distribution id.

## 5. Authorization

**The four ways an address authorizes.** A contract that makes the call, a classic account with signed credentials, the transaction's source account, and a custom account contract. Source: `HOST/auth.rs:88-129`, <https://developers.stellar.org/docs/learn/fundamentals/contract-development/authorization>.

### Pre-funded distributor

When the distributor calls `usdc.transfer(distributor, recipient, amount)`, the SAC calls `require_auth` on the distributor's own address. The host treats the direct caller as having authorized the call. No authorization entry, no signature. Source: `HOST/auth.rs:96-104`, and the authorization docs, section "Contract Invoker". Measured: a batch paid from the contract's balance in enforcing mode with zero authorization entries.

This covers only direct calls. A call two levels down on the contract's behalf needs `authorize_as_current_contract`. Source: `SDK/env.rs:455-467`.

Who may trigger `distribute` is a separate question, answered by L6's own `require_auth` on its owner.

### Pull-at-call funding

If `distribute` moves the treasury's USDC into the distributor in the same call, the SAC calls `treasury.require_auth()`. The treasury signs one authorization entry whose tree is the root call and the nested transfer. Measured, as recorded by the test host:

```
distributor.pull_and_pay(asset, treasury, payouts)
└── usdc.transfer(treasury, distributor, total)
```

The same mechanism as `buy` in `docs/interface.md:284-289`. What the signature covers and how a native multisig signs are in `docs/interface.md:295-305` and `612-620`. Checked against the host:

- The signers' weights must reach the account's medium threshold. Source: `HOST/builtin_contracts/account_contract.rs:242-256`.
- At most 20 signatures, sorted by public key, and a signer with weight 0 is an error. Source: `account_contract.rs:25`, `181-239`.
- The entry fixes every argument of the root call and of the nested transfer, so the recipient list and the total are final when the first officer signs. Source: the authorization docs, section on matching invocation trees.
- The sub-call needs nothing special from the contract. Calling `usdc.transfer(treasury, ...)` after `treasury.require_auth()` is enough. Source: the authorization docs, section "Authorizing Sub-contract Calls".

Two consequences for the design questions. With pull funding and skip-and-report, the pulled total is fixed by the signature, so the amount of a skipped leg stays in the distributor and needs a way out. And if the key that triggers distributions is a single BWB hot key that is also the transaction source, it can use source-account credentials and no entry is signed (`docs/interface.md:303`).

An allowance is the other pull route. The treasury calls `approve(treasury, distributor, amount, live_until_ledger)` once, and the distributor later calls `transfer_from` as spender, which the direct-caller rule authorizes. SAC allowances live in temporary storage and expire. Source: `SAC/contract.rs:158-183`, `229-250`, `SAC/allowance.rs:18`, `94`.

### The sale paying into a distributor

The sale can already pay a distributor. `withdraw_payment` takes any recipient except the sale itself, and a contract recipient needs no trustline. Source: `contracts/offer-sale/src/lib.rs:531-567`.

Doing that and the distribution in one transaction is a different matter, because a transaction has one root invocation. Three shapes are possible:

1. Two transactions. `withdraw_payment` with one leg to the distributor, then `distribute`. Each needs its signer.
2. The distributor is the root. A new entrypoint calls `sale.withdraw_payment(payouts, owner)` and then pays out. The sale checks `owner.require_auth()` and nothing about who calls it (`contracts/offer-sale/src/storage.rs:112-117`), so the owner's one authorization entry covers it. If the distributor's entrypoint also requires the owner, the tree is `distributor.fn → sale.withdraw_payment` and the signature binds the recipient list. If it does not, the owner's entry is rooted at `sale.withdraw_payment` alone and says nothing about where the distributor sends the money. Source: the authorization docs, section on matching invocation trees.
3. The sale is the root and calls the distributor after paying it. That needs new sale code and an upgrade of every deployed sale.

## 6. Idempotency markers

Base's contract has none. `distributeTokens` takes no id and pays again if called again. Source: `TokenDistributor.sol:80-106`.

### What each storage type does at expiry

| Storage | At TTL 0 | Marker guarantee |
|---|---|---|
| Temporary | deleted for good, can be created again | lost once the entry expires |
| Persistent | archived, restored by the transaction that touches it | holds |
| Instance | archived with the contract, restored with it | holds while the contract exists |

Source: <https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival>.

**Why an archived persistent marker still protects.** An archived persistent entry can never be re-created, only restored. A transaction whose footprint names an archived entry without listing it for restoration fails at apply, before the contract runs. With the entry listed, the host restores it first and the contract then reads it as present. Either way a second payment under the same id does not go through. Simulation fills the restore list. Source: the state archival page, sections "Contract Data Automatic Restoration" and "Behavior of transactions that try to access an archived Persistent entry", and CAP-66, "Automatic Entry Restoration".

**Why a temporary marker does not.** Its TTL can be extended to at most `max_entry_ttl`, 3,110,400 ledgers or about 180 days, counted from the current ledger. If nobody extends it, it is gone after that and the same id can be paid again. Source: the same page, sections "Temporary" and "Maximum TTL". The host's own nonces are temporary for this reason, and they expire with the signature (`HOST/auth.rs:2279-2293`).

**Instance storage is one entry.** Everything in it shares one ledger entry and one TTL, is loaded on every call, and is capped at 65,536 B. A map of paid ids there grows the entry on every distribution. A single counter does not. Source: the state archival page, section "Instance", and the live `contract_data_entry_size_bytes`.

### TTL numbers

- A new or restored persistent entry starts at the minimum TTL: 2,073,600 ledgers on mainnet, about 120 days, and 120,960 on testnet, about 7 days.
- A read or a write does not extend a TTL. Only `extend_ttl` in the contract or an `ExtendFootprintTTLOp` from anyone does.
- The repo's policy tops entries up to `max_ttl()` once they fall 30 days below it (`contracts/offer-sale/src/storage.rs:31-52`). It runs only on entries a call touches, and a paid marker is touched again only by a replay.

Source: the state archival page, sections "Minimum TTL" and "ExtendFootprintTTLOp", and the live settings in section 3.

### Rent, order of magnitude

Rent for an entry is `size × rent_rate_per_1KB × ledgers / (1024 × denominator)`, with denominator 1,215 for persistent and 2,430 for temporary. Source: `HOST/fees.rs:397-416`, live `state_archival` settings.

The rate per 1 KB follows the network's Soroban state size and has a floor of 1,000 stroops. Source: `HOST/fees.rs:17`, `245-280`. Mainnet's state averaged 1.54 GB over the last 30 samples against a 3 GB target, which puts the formula below the floor. Testnet is at 4.61 GB against 7 GB, also at the floor. Source: `stellar network settings --internal`, 2026-11-12. So today the rate is 1,000 stroops per KB on both.

For a 100-byte marker, the size I measured for a `Key(u64) → bool` entry:

| Case | Rent |
|---|---|
| Persistent, created on mainnet, 120 days | about 167,000 stroops, 0.017 XLM |
| Persistent, kept at the 180-day maximum | about 250,000 stroops, 0.025 XLM per 180 days |
| Temporary, 180 days | about 125,000 stroops, 0.0125 XLM |

Each write also pays 1,250 stroops per entry and 437 stroops per KB, and each TTL extension pays a 48-byte TTL-entry write. Source: live `fee_write_ledger_entry` and `fee_write1_kb`, `HOST/fees.rs:11`, `290-325`. A marker keyed by a 32-byte id is about 40 B larger.

One marker per (distribution, recipient) costs that per recipient, and one write entry per leg, which halves the write-entry ceiling in section 3.

### How others store claim markers

- OpenZeppelin's `MerkleDistributor` keeps `Claimed(u32)` in persistent storage and extends it by 30 days when it is read. Source: `OZ/merkle_distributor/storage.rs:13-18`, `47-59`, `OZ/merkle_distributor/mod.rs:83-85`.
- `soroban-examples/merkle_distribution` keeps `Claimed(u32)` in instance storage. Source: <https://github.com/stellar/soroban-examples/blob/main/merkle_distribution/src/lib.rs>.

## 7. Prior art

**OpenZeppelin stellar-contracts 0.7.2, the pinned version.** Its only distribution code is `stellar-contract-utils::merkle_distributor`, a pull model: the admin sets a Merkle root, each recipient claims with a proof, and an index can be claimed once. Errors 1300–1302. There is an example, `examples/fungible-merkle-airdrop`. No payment splitter, no push distributor, no batch transfer. Source: `OZ/merkle_distributor/mod.rs`, and the repository tree at tag `v0.7.2`, <https://github.com/OpenZeppelin/stellar-contracts/tree/v0.7.2>.

**OpenZeppelin 0.7.3 and 0.8.0, both released 2026-10-20.** 0.7.3 is a security release for `stellar-accounts` and still builds on soroban-sdk 26.1. 0.8.0 builds on soroban-sdk 28.0.0, renumbers most error codes so each package owns a disjoint range, and adds batch functions inside the RWA token (`batch_mint`, `batch_transfer`), which move the token's own balances and are not a payout contract. Still no splitter and no push distributor. Source: <https://github.com/OpenZeppelin/stellar-contracts/releases/tag/v0.7.3>, <https://github.com/OpenZeppelin/stellar-contracts/releases/tag/v0.8.0>, and each tag's `Cargo.toml`.

This makes three statements in the repo stale: "0.7.2, the latest stable release" and "OpenZeppelin 0.7.2 ... requires it" in `docs/interface.md:573`, and the comment in `Cargo.toml:18-20`.

**Audits.** The repository's `audits/` folder holds reports named for 0.1.0-RC, 0.2.0, 0.3.0-rc.2, 0.5.0 (and a re-audit), 0.6.0, 0.7.0 and 0.8.0. Source: <https://github.com/OpenZeppelin/stellar-contracts/tree/main/audits>. I did not open the PDFs.

**OpenZeppelin's batch benchmark.** `bench/rwa-batch` at 0.8.0 measures batch ceilings against live mainnet settings and finds them bound by footprint or event size at 37 to 99 items. It is the closest published method to section 3. Source: <https://github.com/OpenZeppelin/stellar-contracts/blob/v0.8.0/bench/rwa-batch/README.md>.

**stellar/soroban-examples.** `merkle_distribution` is a claim contract funded in its constructor. `atomic_multiswap` batches calls and skips failures. Nothing pays a list of recipients from a contract. The README says the examples are educational, not tested or audited, and not to be relied on. Source: <https://github.com/stellar/soroban-examples>, default branch, pushed 2026-10-06.

**SDF's Stellar Disbursement Platform.** It does bulk payouts off chain. Money sits in a G distribution account. Each payment is its own transaction with one operation, sent from a channel account: a classic `Payment` for a G destination, or a single SAC transfer (`PaymentToContract`) for a C destination. Its only contract is a smart wallet for recipients. Source: <https://github.com/stellar/stellar-disbursement-platform-backend/blob/develop/internal/transactionsubmission/payment_transaction_handler.go>, lines 43–118, and the repository tree.

## 8. Repo conventions L6 must follow

**Powers are stored addresses.** One `#[contracttype]` struct of holders, set in the constructor, kept in instance storage, no setter. Changing a holder takes an upgrade. Source: ADR 0005, `contracts/offer-sale/src/types.rs:6-18`, `contracts/offer-sale/src/storage.rs:57-63`. The sale has `owner`, `pauser`, `upgrader`.

**Issue #28 says otherwise.** It asks for "ownable/admin pattern via OpenZeppelin". ADR 0005 rejects OpenZeppelin `AccessControl` for contracts whose powers belong to two parties, and ADR 0008 allows it on the allowlist because both of its powers are BWB's. If the distributor's powers are all BWB's, ADR 0008's reasoning applies. If the tokenizer holds one, ADR 0005's does.

**Every privileged call takes its caller.** `require_power(e, caller, holders, err)` asks for the caller's signature first, then compares it with the stored holders, and fails with the power's own code. A missing signature fails as `Error(Auth, InvalidAction)`. Source: `contracts/offer-sale/src/storage.rs:109-117`, `docs/interface.md:460`, `802`.

**Pause.** Implement OpenZeppelin's `Pausable` trait from `stellar-contract-utils`. `pause(caller)` accepts `owner` or `pauser`. `unpause(caller)` accepts `owner` only. Both emit `pause_changed {paused}` with the caller as topic, after OpenZeppelin's `paused` or `unpaused`. Guard the money-moving entrypoints with `#[when_not_paused]`. Codes 1000 and 1001 come from OpenZeppelin. Source: ADR 0003, `contracts/offer-sale/src/lib.rs:771-806`, `OZ/pausable/mod.rs:134-136`.

**Upgrade.** Implement `Upgradeable`. `upgrade(new_wasm_hash, operator)` requires the stored `upgrader`, emits `upgraded {new_wasm_hash, from_schema_version}` from the old code, then calls `upgradeable::upgrade`. No `migrate` in v1. Source: ADR 0001, `contracts/offer-sale/src/lib.rs:808-828`, `docs/interface.md:410`, `422`.

**The upgrade test.** A second crate at `contracts/<crate>/upgrade-fixture/` redeclares the v1 storage layout by hand and adds `v2_marker`. `scripts/test.sh` builds it with the Stellar CLI into `target/fixtures/`. `build.rs` sets `cfg(fixture_missing)` when the Wasm is absent, so a plain `cargo test` fails one test with a hint. The test swaps the code and checks that every piece of state, the pause flag and `schema_version` survive, that the new code can upgrade again, that an unknown hash fails, and that a non-upgrader is refused. Source: `contracts/offer-sale/src/test/upgrade.rs`, `contracts/offer-sale/build.rs`, `contracts/offer-sale/upgrade-fixture/src/lib.rs:1-25`, `scripts/test.sh:21-27`. A new crate has to be added to the fixture loop in `scripts/test.sh` and to `members` in the root `Cargo.toml`.

**Versions.** `contractmeta!(key = "binver", val = env!("CARGO_PKG_VERSION"))`, `const SCHEMA_VERSION: u32 = 1` set through `upgradeable::set_schema_version` in the constructor, and a `schema_version()` read. Persistent structs carry their own `schema` field. `schema_version` stays 1 until mainnet. Source: `contracts/offer-sale/src/lib.rs:56-60`, `139`, `643-645`, `contracts/offer-sale/src/storage.rs:11-12`, `docs/interface.md:540`, `772`.

**Error codes.** One block of 100 per contract: token 6000–6099, allowlist 6100–6199, sale 6200–6299. The next free block is 6300–6399. Source: `docs/interface.md:680-686`, the three `errors.rs` files. OpenZeppelin 0.7.2 uses 100–499, 1000–1599, 2000–2299, 4000–4199 and 5000–5099, counted from the `= n,` discriminants in the four 0.7.2 crates, so 6300 is clear. Codes from called contracts surface unchanged, and the contract's document lists the ones the backend will meet, the SAC's 8, 10, 11, 12 and 13 included.

**Events.** Section 6.1 of the interface document:

- `#[contractevent]` structs, the name in snake case as the first topic, at most 3 more topics, data as a map of named fields.
- No optional fields.
- New fields may be added at any time, so a batch limit must leave room for them.
- Amounts are `i128` in the asset's base unit, timestamps are ledger seconds.
- A per-recipient event exists so the backend's ledger records one leg per account (`docs/interface.md:450`).
- The construction emits one event with the whole configuration (`configured` on the sale).
- Sizes are measured as XDR and pinned by a test.

Source: `docs/interface.md:414-422`, `contracts/offer-sale/src/events.rs`, `contracts/offer-sale/src/test/events.rs:327-385`.

**TTL.** Every state-changing call extends the instance, and every persistent entry it wrote, to `max_ttl()` once they fall 30 days below it. Reads extend nothing. No backend keep-alive job. The full policy is L9. Source: `contracts/offer-sale/src/storage.rs:27-52`, `docs/interface.md:535-540`.

**Batch rules.** A `MAX_*` constant with the arithmetic in its doc comment, `BatchTooLarge` when exceeded, the check before any transfer, and a boundary test that measures a full batch with `cost_estimate().resources()` after lifting the test host's limits. Source: `contracts/offer-sale/src/lib.rs:86-102`, `contracts/offer-sale/src/test/boundary.rs:70-82`, `231-252`.

**Constructor and types.** `__constructor` takes `#[contracttype]` structs so the CLI names every field, validates, and fails with an `InvalidConfig` code. The payment asset is a constructor parameter typed as an address and used through `token::TokenClient`. Source: `contracts/offer-sale/src/lib.rs:113-153`, `contracts/offer-sale/src/types.rs`. The sale already has a `Payout {recipient, amount}` type (`types.rs:105-111`).

**Crate layout.** `src/{lib,errors,events,storage,types}.rs` and `src/test/`, `crate-type = ["lib", "cdylib"]`, dependencies from the workspace, `#![no_std]`, the release profile with `overflow-checks = true`. Source: `contracts/offer-sale/Cargo.toml`, root `Cargo.toml`.

**Tests by the D2 vocabulary.** One module per word, each with a `//!` line that states the rule it proves, test names written as sentences. The sale has `authorization`, `boundary`, `pause`, `idempotency`, `failure`, `upgrade`, plus `events`, `ttl`, `state` and `cooling_off`. Issue #28 names the first six. Tests go through the generated client, beside a real SAC as USDC with `IssuerFlags::RevocableFlag` set. Source: `contracts/offer-sale/src/test/mod.rs:1-19`, `87-88`, `docs/offer-sale.md:218-233`.

What the existing modules assert, as a template:

| Module | Pattern |
|---|---|
| `authorization` | a signed wrong caller per privileged call gets the power's code, an unsigned call fails at authorization, positives use exact auths |
| `boundary` | N passes and N+1 fails for each limit, and a full batch measured against the documented network limits |
| `pause` | what a pause blocks and what it never blocks, with 1000 and 1001 |
| `idempotency` | repeating a call or a list entry converges to the same state |
| `failure` | every documented rejection reached through the client with its code, including a frozen USDC balance |
| `upgrade` | storage survives the swap |

**CI and toolchain.** `cargo fmt --check`, `scripts/test.sh --locked`, `cargo clippy --all-targets --workspace -- -D warnings`, `stellar contract build`, with Rust 1.97.1 and Stellar CLI 28.1.0 pinned. `clippy.toml` allows `unwrap` in tests only. Source: `.github/workflows/contracts.yml`, `rust-toolchain.toml`.

**Deploy.** `scripts/deploy.sh <crate> -- <constructor args>` builds, uploads, deploys and writes `deployments/<network>.json` with contract id, Wasm hash, deploy transaction, deployer, constructor arguments, toolchain and explorer links. A per-offering crate needs `OFFERING=<id>` and gets a salt of `sha256("bwb:<id>:<role>")`. A global crate is recorded under `contracts.<crate>`. Source: `scripts/deploy.sh:1-70`, `scripts/lib/offering.sh:27-49`, `deployments/README.md`. `crate_role` knows only `token` and `sale`, so a per-offering distributor needs a third role there. Base deploys one TokenDistributor per offering (`docs/interface.md:224`).

**Documents.** One `docs/<crate>.md` per contract with these sections: summary, Powers, Constructor, Calls, Differences from Base, Events, Errors, Storage and expiry, Tests. Changes to the cross-cutting rules go through the changelog in `docs/interface.md`. Decisions get an ADR in `docs/adr/`. Source: `docs/offer-sale.md`, `docs/interface.md:756`.

**Vocabulary.** `CONTEXT.md` lists "distribution" as a word to avoid for Release (line 83), and has no entry yet for the distributor or for a payout. L6 needs its terms added there.

## Measurements

A scratch workspace outside the repo, resolved offline from this repo's `Cargo.lock`:

- A Wasm contract built with `stellar contract build` (4,202 B) with four entrypoints: a loop of `transfer`, a loop of `try_transfer` that records each error code, one transfer to a `MuxedAddress`, and a pull from a funder followed by payouts.
- Tests that register that Wasm, create a SAC with `register_stellar_asset_contract_v2`, write classic account and trustline entries straight into the host's storage for G recipients, call `cost_estimate().budget().reset_unlimited()` and `disable_resource_limits()`, and read `cost_estimate().resources()` after each call.
- Batches of 1 to 199 legs, to G and to C recipients, with and without an own event and a marker.

The scratch files are not kept. L6's own boundary tests should pin the figures that end up in its document, as the sale's do.

Two things the test host does not show: the transaction's size, and the return value's share of the event budget (`HOST/host/invocation_metering.rs:17-22`).

## Not verified

- **Network cost parameters.** Instruction and memory figures come from the sdk 26.1.1 test host. Mainnet runs protocol 29 and testnet protocol 30, whose cost parameters I did not compare. Instructions sit at about 3% of the limit at 66 legs, so the conclusion does not depend on it, but no figure here was confirmed by a testnet simulation.
- **Transaction size per leg.** Computed by hand from XDR layouts, not measured.
- **Zero-amount and over-`i64` transfers.** Read from the SAC source, not run.
- **`MuxedAddress` inside a `#[contracttype]` payout struct.** I measured it as a plain function argument only.
- **Whether the L5 indexer or public RPC `getEvents` returns CAP-67 events for classic payments.** CAP-67 leaves it to node configuration.
- **OpenZeppelin audit scope.** I listed the report file names and did not read them, so I cannot say which commit of 0.7.x each covers, or whether `merkle_distributor` was in scope.
- **Audit status of the Stellar Disbursement Platform.** Not looked up.
- **Circle's freeze practice on Stellar.** I verified the issuer's flags, which make freezing possible. I found no Circle page describing when it uses them on Stellar.
- **The SAC ids.** Derived from the asset with the Stellar CLI. Circle does not publish them.
