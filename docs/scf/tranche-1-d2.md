# SCF #45 · Tranche 1 · Deliverable 2 evidence

**Deliverable 2 — Soroban Regulated Offering Asset Lifecycle Layer.** This document maps each of D2's four completion criteria, as written in the [awarded submission](https://communityfund.stellar.org/submissions/rec8bhUAafIcSJUzA), to the code, tests, contracts and transactions that meet it. Every claim links to something a reviewer can open or run.

- Network: Stellar **testnet** (passphrase `Test SDF Network ; September 2015`).
- Source release: tag `TODO(t1)` on `main`, commit `TODO(commit)`. WASM hashes below reproduce from that commit with `stellar contract build` (Stellar CLI 28.1.0; toolchain pinned in [`rust-toolchain.toml`](../../rust-toolchain.toml)).
- Explorer: [stellar.expert testnet](https://stellar.expert/explorer/testnet).
- Status: criteria 1, 2 and 4 met; criterion 3 met by a reproducible query over contract state (see the note under it). Deviations from the submission's wording are listed in [Deviations](#deviations).

## What was promised, what exists

D2's scope asks for "modular contracts for offering/position registration, eligibility and allocation policy, and lifecycle/refund controls". They are three contracts, one crate each:

| Scope part | Contract | What it does | Doc |
|---|---|---|---|
| Offering / position registration | [`offer-token`](../../contracts/offer-token) | One token per offering; an investor's balance is their position. Whole supply minted to the sale contract at construction; transfer whitelist; pause; admin transfer; metadata and offering URI. | [docs/offer-token.md](../offer-token.md) |
| Eligibility and allocation policy | [`offer-allowlist`](../../contracts/offer-allowlist) | One global registry: who may invest (`enabled`) and up to how much per UTC month (`max`). A purchase consumes room and gets a receipt; a refund restores it. | [docs/offer-allowlist.md](../offer-allowlist.md) |
| Lifecycle and refund controls | [`offer-sale`](../../contracts/offer-sale) | Escrow and state machine: Preparing → Active → Closing → Successful or Failed. Buy, cooling-off refund, pause, close, reopen, cancel, finalize (burning the unsold supply), release, refund batches, payout, token withdrawal. | [docs/offer-sale.md](../offer-sale.md) |

How the backend drives them, who signs what, and the event and error catalogues are in [docs/interface.md](../interface.md) (version 1.0).

## Criterion 1

> Public testnet contract IDs, deployment transactions, WASM hashes, source release, and explorer links demonstrate the offering and position modules.

**Met.** The deployments below are the current ones; earlier instances from development are listed in the README as superseded. The manifest [`deployments/testnet.json`](../../deployments/testnet.json) is written by the deploy script and is the source of truth.

| Contract | Version | Contract ID | WASM hash | Deploy tx |
|---|---|---|---|---|
| `offer-allowlist` (global) | 0.6.0 | [`CAVI4RRO…PWEDRQ`](https://stellar.expert/explorer/testnet/contract/CAVI4RROECISQXR4BR3O2BYH6NJFE7WI6NU6UTTHZKX3VCR6J6PWEDRQ) | `577b1c184ec72a065d1a7e5dcc462de0343ac63af49de8c758fdb0d77f7e5597` | [`826dd754…`](https://stellar.expert/explorer/testnet/tx/826dd75483610ff9a91c4fb488447b1c69fe0cd5cbcd8ecccf885d8d3603b517) |
| `offer-token` · offering `t1-demo-4` | 0.2.0 | [`CACNDXN7…LSZZWT`](https://stellar.expert/explorer/testnet/contract/CACNDXN74YKAIIGATVUZCSZHAKKWA5MARPX7FV4IEIDPGYIN6XLSZZWT) | `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0` | [`a9f4dd7c…`](https://stellar.expert/explorer/testnet/tx/a9f4dd7cc2882ccb4ac0d82e50232cc5a653cc534eae7803fcd6513ad1eece2a) |
| `offer-sale` · offering `t1-demo-4` | 0.2.0 | [`CAYQ6SOR…NDQJUT`](https://stellar.expert/explorer/testnet/contract/CAYQ6SOR3AL4XOTFMBEUATJ56X2KLSTUUJVXKXXYYHJWCKJIEJNDQJUT) | `49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd` (deployed as `7a6d8026…`, upgraded in place: [`779a16b6…`](https://stellar.expert/explorer/testnet/tx/779a16b639b7dfbfb7c360ba8477bdcd3d4b1e512f6964d615e8920ef50fede1)) | [`a9855097…`](https://stellar.expert/explorer/testnet/tx/a985509755d5127c6256d38e38c2ae83e2d1e750ad9c0f97989efe2d56522631) |
| `offer-token` · offering `t1-demo-5` | 0.2.0 | [`CDSY2PJZ…343653`](https://stellar.expert/explorer/testnet/contract/CDSY2PJZUBY5EQCYEP2QGZE7GP3NF3IUFRQ2A7CTJLZQHN2Y7F343653) | `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0` | [`80fccdd7…`](https://stellar.expert/explorer/testnet/tx/80fccdd77e86435110c654ce909b0613f37151f424c5bbe894472b0c160e2a32) |
| `offer-sale` · offering `t1-demo-5` | 0.2.0 | [`CAGFA4RW…IKYDFER`](https://stellar.expert/explorer/testnet/contract/CAGFA4RWRBBATYCJQAE5O5GECPTOKMLDVHM4R5BR454JN6UOCIKYDFER) | `49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd` (deployed as `7a6d8026…`, upgraded in place: [`65e74d91…`](https://stellar.expert/explorer/testnet/tx/65e74d917e0c2f7033e3289a7d19ee9aad79391e8cd461f8cb69b70f21cd9ade)) | [`4ea3f9a8…`](https://stellar.expert/explorer/testnet/tx/4ea3f9a8e90919308d15d6b63eea91eec0168a7d0abc5081e3244bdda659b1a1) |
| Payment asset (demo USDC) | — | [`CCW4PRV5…DTXNJGY`](https://stellar.expert/explorer/testnet/contract/CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY) | Stellar Asset Contract for `USDC:GDGBRCFA…`, issued by BWB's testnet identity, not Circle's | — |

`t1-demo-4` walks the success path of the demonstration, `t1-demo-5` the failure path. Each offering's two addresses are derived from the deployer and a salt from the offering ID ([ADR 0006](../adr/0006-the-supply-mints-to-the-sales-precomputed-address.md)), so the token can mint its supply to the sale before the sale exists. Decoded deploy transactions are snapshotted under [`evidence/deploys/`](evidence/deploys/).

## Criterion 2

> Published transactions and decoded events demonstrate eligibility/allocation rejection, investment, pause/unpause, cooling-off, cancellation, and refund state transitions.

**Met.** [`scripts/demo-offer-sale.sh`](../../scripts/demo-offer-sale.sh) ran every transition on the two offerings above as real transactions. Rejections are submitted anyway, so each one is a failed transaction on the ledger carrying its error code; the backend's rule for reading them is [interface §7.3](../interface.md#73-rejections-as-evidence). The full 26-step table, with every hash, is in the [README](../../README.md#l4-demonstration). The transitions D2 names:

| D2 transition | Step | Transaction | What to look at |
|---|---|---|---|
| Eligibility rejection | 4 | [`bb98c609…`](https://stellar.expert/explorer/testnet/tx/bb98c60984c32ada9b0fb770579bbffbf31102d51305df9d3e2ef468f44b96a6) | Failed; contract error `6100 InvestorNotEnabled` (investor B is not on the allowlist) |
| Allocation rejection | 5 | [`da1ac6bf…`](https://stellar.expert/explorer/testnet/tx/da1ac6bf4aa09f9e2d1b657878ae119771c1892c1c8143e884d5420dafd3ec97) | Failed; `6101 AllocationExceeded` (4,000 USDC against a 3,000 cap) |
| Investment | 6 | [`490380f8…`](https://stellar.expert/explorer/testnet/tx/490380f877d4d7250b0819cded99fd51c2ab1aab8a578fa0f74f11f4c41b2f62) | `purchased` event: 50 tokens, 500 USDC into escrow; allowlist `allocation_consumed` |
| Pause | 7 | [`47d1debb…`](https://stellar.expert/explorer/testnet/tx/47d1debb3855e60e43915ef0bf49929d41a3d98c3e69a7c76ef40ae18bd56513) | `pause_changed` by the platform; the next purchase (step 8, [`e5ca35a6…`](https://stellar.expert/explorer/testnet/tx/e5ca35a6225c7795f1ef2360046548730293f7b689c55662b0d12cf2225f9d3a)) fails with `1000 EnforcedPause` |
| Unpause | 10 | [`813e200b…`](https://stellar.expert/explorer/testnet/tx/813e200b690a169db6c3dee43b3deba7edb32db810305ed1134b6708b8c2c5e2) | `pause_changed` by the owner; step 9 ([`58ff8d0a…`](https://stellar.expert/explorer/testnet/tx/58ff8d0a4b254bb963d2d73f66aa195cd6dad29bc5399fe0212ec2525c05416f)) shows the platform refused with `6222 NotOwner` |
| Cooling-off | 11 | [`8833a61e…`](https://stellar.expert/explorer/testnet/tx/8833a61e85e46b46cf1d6eecd96735184b0b24427e863c40605b2e94f03f37e7) | `refunded` with reason `cooling_off`: 500 USDC back; allowlist `allocation_restored` |
| Cancellation | 20 | [`7ab128c2…`](https://stellar.expert/explorer/testnet/tx/7ab128c2b8b7ee3a13a8f838a5eb70d7c7ccf87578e68ecc7e614acf85c1fa6a) | `cancelled` and `state_changed` Active → Failed |
| Refund | 21 | [`42a52035…`](https://stellar.expert/explorer/testnet/tx/42a52035d494e23b636bb9f4b2b25fedefd8465b1827d340df880fddf6eadef7) | `refunded` with reason `failed`: investor A paid back; allowlist room restored |

Also on chain, beyond D2's list: activation (steps 2 and 17), more purchases (12, 18), close and reopen (13), a purchase after close refused (14, `6201 NotActive`), a too-early finalize refused (15, `6214 CoolingOffOpen`), a stranger's cancel refused (19, `6222`), the owner withdrawing a failed offering's tokens (22), and the settlement path: finalize with the burn of the unsold supply, release, payout and a refused late cooling-off (steps 23–26, `TODO(stage 2 hashes)`).

**Snapshot.** RPC keeps transactions and their events for about seven days and testnet resets periodically, so the decoded result and events of every step are committed under [`evidence/demo/`](evidence/demo/) (one JSON per transaction, index in [`index.json`](evidence/demo/index.json)), captured by [`scripts/capture-evidence.sh`](../../scripts/capture-evidence.sh).

## Criterion 3

> A public reproducible query calculates tokenized AUM from issuance and active-position events without PII.

**Met**, with one wording difference stated here. [`scripts/aum.sh`](../../scripts/aum.sh) computes tokenized AUM from the sale contracts' own accounting, read through simulated view calls (`state`, `config`, `accounting`); no key, fee or transaction is needed. Definition:

- An offering counts while it is not Failed (Active, Closing or Successful).
- Units = `sold_tokens`: tokens reserved or released to investors, net of refunds.
- Value = `sold_tokens × price`, in the payment asset. It equals `raised` (payment received, net of refunds) because every purchase is an exact multiple of the price; the script checks that.

```bash
scripts/aum.sh --json docs/scf/evidence/aum.json t1-demo-4 t1-demo-5
```

Output at ledger `TODO`, `TODO(UTC)`:

`TODO(table)`

Committed output: [`evidence/aum.json`](evidence/aum.json).

**Why state and not an event replay.** `sold_tokens` and `raised` are updated by exactly the transitions that emit `purchased`, `refunded` and `finalized`, so the view returns what replaying those events returns, without depending on an event archive (RPC drops events after about seven days). The event-to-value mapping, for a team that prefers to replay events from an indexer, is [interface §8.2](../interface.md#82-what-moves-value). The contracts carry no personal data: events and storage hold only addresses and amounts.

## Criterion 4

> Public CI and the evidence index map authorization, boundary, pause, refund, replay/idempotency, and failure scenarios to code, tests, transactions, and events.

**Met.** CI ([`.github/workflows/contracts.yml`](../../.github/workflows/contracts.yml)) runs `cargo fmt --check`, the whole test suite ([`scripts/test.sh`](../../scripts/test.sh)), `clippy -D warnings` and `stellar contract build` on every change under `contracts/`; latest run on the release: `TODO(run link)`. The test modules are named after D2's own categories, one set per contract (326 tests: token 80, allowlist 114, sale 132):

| D2 category | Tests | Code | On chain (demo step) |
|---|---|---|---|
| Authorization | [`offer-sale/src/test/authorization.rs`](../../contracts/offer-sale/src/test/authorization.rs), [`offer-allowlist/…/authorization.rs`](../../contracts/offer-allowlist/src/test/authorization.rs), [`offer-token/…/authorization.rs`](../../contracts/offer-token/src/test/authorization.rs) | Power holders in [`offer-sale/src/types.rs`](../../contracts/offer-sale/src/types.rs); roles in [`offer-allowlist/src/lib.rs`](../../contracts/offer-allowlist/src/lib.rs) | 9 (platform cannot unpause), 19 (stranger cannot cancel): `6222 NotOwner` |
| Boundary | [`offer-sale/…/boundary.rs`](../../contracts/offer-sale/src/test/boundary.rs), [`offer-allowlist/…/boundary.rs`](../../contracts/offer-allowlist/src/test/boundary.rs), [`offer-token/…/boundary.rs`](../../contracts/offer-token/src/test/boundary.rs) | Batch limits and purchase cap in [`offer-sale/src/lib.rs`](../../contracts/offer-sale/src/lib.rs) | 5 (over the monthly cap, `6101`); supply cap `6207` is tested, not demonstrated |
| Pause | [`offer-sale/…/pause.rs`](../../contracts/offer-sale/src/test/pause.rs), [`offer-token/…/pause.rs`](../../contracts/offer-token/src/test/pause.rs) | OpenZeppelin `pausable` on both contracts | 7, 8, 10 |
| Refund | [`offer-sale/…/cooling_off.rs`](../../contracts/offer-sale/src/test/cooling_off.rs), refund cases in [`state.rs`](../../contracts/offer-sale/src/test/state.rs) and [`failure.rs`](../../contracts/offer-sale/src/test/failure.rs) | `cooling_off_refund`, `refund`, allowlist `restore` | 11 (cooling-off), 21 (failed offering), 25 (late cooling-off refused, `6225 NotOpen`) |
| Replay / idempotency | [`offer-sale/…/idempotency.rs`](../../contracts/offer-sale/src/test/idempotency.rs), [`offer-allowlist/…/idempotency.rs`](../../contracts/offer-allowlist/src/test/idempotency.rs), [`offer-token/…/idempotency.rs`](../../contracts/offer-token/src/test/idempotency.rs) | Release and refund skip what is already done; receipts restore once | 13 (close, reopen, close), 24 and 26 (release and payout, `TODO`) |
| Failure | [`offer-sale/…/failure.rs`](../../contracts/offer-sale/src/test/failure.rs), [`offer-allowlist/…/failure.rs`](../../contracts/offer-allowlist/src/test/failure.rs), [`offer-token/…/failure.rs`](../../contracts/offer-token/src/test/failure.rs) | Error catalogue: [`offer-sale/src/errors.rs`](../../contracts/offer-sale/src/errors.rs), [`offer-allowlist/src/errors.rs`](../../contracts/offer-allowlist/src/errors.rs) | 4, 5, 8, 14, 15: every rejection is a failed transaction with its code |
| Events | [`offer-sale/…/events.rs`](../../contracts/offer-sale/src/test/events.rs) and the two others | [`offer-sale/src/events.rs`](../../contracts/offer-sale/src/events.rs); catalogue in [interface §6](../interface.md#6-events) | every successful step; snapshots in `evidence/demo/` |
| Upgrade | [`offer-sale/…/upgrade.rs`](../../contracts/offer-sale/src/test/upgrade.rs) and the two others: storage survives the code swap | `upgrade`, gated by the `upgrader` | the in-place upgrade of both sales ([`779a16b6…`](https://stellar.expert/explorer/testnet/tx/779a16b639b7dfbfb7c360ba8477bdcd3d4b1e512f6964d615e8920ef50fede1), [`65e74d91…`](https://stellar.expert/explorer/testnet/tx/65e74d917e0c2f7033e3289a7d19ee9aad79391e8cd461f8cb69b70f21cd9ade)): state, reservations and accounting unchanged after the swap |

Each contract's doc has a per-module description of what the tests cover ([offer-sale.md § Tests](../offer-sale.md#tests)).

## Scope lines

The submission's scope bullets for D2, and where each stands:

- **Modular contracts for offering/position registration, eligibility and allocation policy, lifecycle/refund controls.** The three contracts above; the sale calls the other two, which can be deployed and upgraded on their own.
- **Debt and equity position records.** The token is one quota of one offering; whether the offering is debt or equity is a property of the offering's documents, reachable through the token's `offer_uri`, not a field in the contract. Both kinds deploy from the same code with no change.
- **Per-investor and aggregate allocation limits.** Per investor: the allowlist's monthly cap (`6101`), plus a cap of 20 live purchases per investor per offering (`6220`). Aggregate: the offering's supply, fixed at the token's construction; a purchase past it fails with `6207 ExceedsSupply`. Supply × price is therefore the hard ceiling of what an offering can raise.
- **Privileged-action controls for pause/unpause, cancellation, cooling-off, and refund execution.** Pause by the platform or the owner, unpause by the owner only; cancel by the owner; cooling-off by the investor; refund batches by anyone, so the platform can run them without a signing ceremony. Negative authorization tests exist for every privileged entrypoint.
- **On-chain events for issuance, allocation, investment, cancellation, refund, and settlement, supporting reproducible tokenized-AUM reporting with PII off-chain.** The mapping from these six terms to event names is [interface §8.1](../interface.md#81-the-grants-terms). No event or storage entry carries personal data.
- **Public automated tests, boundary remediation, authorization and idempotency validation.** Criterion 4 above. The sale contract went through five internal audit rounds before this release; the fixes are in the commit history of [PR #42](https://github.com/BWB-Labs/bwb-stellar/pull/42).

## Deviations

Differences between the submission's wording and what was delivered. None changes a criterion's substance; they are listed so the reviewer does not have to find them.

1. **AUM from contract state, not an event replay.** Criterion 3 says "from issuance and active-position events". The query reads the accounting the same events update (see Criterion 3). Event replay stays possible through the documented mapping once an indexer exists (Tranche 2).
2. **Debt/equity is not a contract field.** See Scope lines. The contracts are instrument-agnostic by design.
3. **The aggregate limit is the supply.** An earlier design had a separate hard cap; it was dropped because supply × price already bounds the raise, and a second number could only disagree with it.
4. **The payment asset is BWB's own testnet USDC**, a Stellar Asset Contract issued by a BWB testnet identity, not Circle's testnet USDC. The contracts take the payment asset's address at construction and work with any SEP-41 asset.
5. **The sale contracts were upgraded in place after the demonstration's first stage.** Stage 1 (steps 1–22) ran on sale WASM `7a6d8026…`, built from the same source minus a rename of event modules and a schema-version note; both sales were then upgraded to the released WASM `49edfc27…` ([`779a16b6…`](https://stellar.expert/explorer/testnet/tx/779a16b639b7dfbfb7c360ba8477bdcd3d4b1e512f6964d615e8920ef50fede1), [`65e74d91…`](https://stellar.expert/explorer/testnet/tx/65e74d917e0c2f7033e3289a7d19ee9aad79391e8cd461f8cb69b70f21cd9ade)) on 2026-10-28, before stage 2, with every reservation and total intact. The two instances still report `schema_version` 2, stored by the constructor under the earlier build; the released code's constant is 1. The upgrade is itself part of the evidence (criterion 4, Upgrade row).
6. **A Closing state was added** between Active and Successful: buying is closed, cooling-off windows run out, then `finalize` decides. The Base (EVM) contracts this port follows go straight from Active to Successful. The reason and the alternatives considered are in [docs/offer-sale.md § States](../offer-sale.md#states).

## Reproduce

```bash
rustup show                                   # pinned toolchain
scripts/test.sh --locked                      # 326 tests, builds the upgrade fixtures
stellar contract build                        # WASM to target/wasm32v1-none/release/
shasum -a 256 target/wasm32v1-none/release/*.wasm   # compare with Criterion 1
scripts/aum.sh t1-demo-4 t1-demo-5            # Criterion 3, needs only the Stellar CLI
scripts/capture-evidence.sh --out /tmp/evidence     # re-fetch the demo's results while RPC still has them
```

Running the demonstration again needs funded testnet identities and a fresh pair of offerings (the state machine never goes back); `scripts/demo-offer-sale.sh` documents them in its header.
