# SCF #45 · Tranche 1 · Deliverable 2 evidence

**Deliverable 2 — Soroban Regulated Offering Asset Lifecycle Layer.** This document takes each of D2's four completion criteria, as written in the [awarded submission](https://communityfund.stellar.org/submissions/rec8bhUAafIcSJUzA), and places next to it the code, tests, contracts and transactions that meet it. Each piece of evidence is linked, and each criterion ends with the command that reproduces it.

Everything runs on Stellar testnet. The source release is tag `t1` on `main`, commit `TODO(commit)`.

## The contracts

Tranche 1 delivers three Soroban contracts, one for each part of the scope in the application. The table maps them.

| Scope part | Contract | What it does | Doc |
|---|---|---|---|
| Offering / position registration | [`offer-token`](../../contracts/offer-token) | One token per offering; an investor's balance is their position. Whole supply minted to the sale contract at construction; transfer whitelist; pause; admin transfer; metadata and offering URI. | [docs/offer-token.md](../offer-token.md) |
| Eligibility and allocation policy | [`offer-allowlist`](../../contracts/offer-allowlist) | One global registry: who may invest (`enabled`) and up to how much per UTC month (`max`). A purchase consumes room and gets a receipt; a refund restores it. | [docs/offer-allowlist.md](../offer-allowlist.md) |
| Lifecycle and refund controls | [`offer-sale`](../../contracts/offer-sale) | Escrow and state machine: Preparing → Active → Closing → Successful or Failed. Buy, cooling-off refund, pause, close, reopen, cancel, finalize (burning the unsold supply), release, refund batches, payout, token withdrawal. | [docs/offer-sale.md](../offer-sale.md) |

Besides the contracts, [docs/interface.md](../interface.md) (version 1.0) documents how to interact with them: the calls, who signs each one, and the events and errors they return. It is the reference for BWB's backend and for any integrator.

## Criterion 1 — met ✅

> Public testnet contract IDs, deployment transactions, WASM hashes, source release, and explorer links demonstrate the offering and position modules.

### Evidence

One allowlist, shared by every offering, and two offerings, each a token and a sale contract. The demonstration under criterion 2 takes one offering to success and cancels the other. Contract IDs and transactions link to the explorer; the hashes are the ones `stellar contract build` gives at the release commit.

**Allowlist** · `offer-allowlist` 0.6.0
Contract [`CAVI4RROECISQXR4BR3O2BYH6NJFE7WI6NU6UTTHZKX3VCR6J6PWEDRQ`](https://stellar.expert/explorer/testnet/contract/CAVI4RROECISQXR4BR3O2BYH6NJFE7WI6NU6UTTHZKX3VCR6J6PWEDRQ) · deployed by [`826dd754…`](https://stellar.expert/explorer/testnet/tx/826dd75483610ff9a91c4fb488447b1c69fe0cd5cbcd8ecccf885d8d3603b517)
WASM `577b1c184ec72a065d1a7e5dcc462de0343ac63af49de8c758fdb0d77f7e5597`

**Offering that succeeds** (`t1-demo-4` on chain)
Token `offer-token` 0.2.0 · contract [`CACNDXN74YKAIIGATVUZCSZHAKKWA5MARPX7FV4IEIDPGYIN6XLSZZWT`](https://stellar.expert/explorer/testnet/contract/CACNDXN74YKAIIGATVUZCSZHAKKWA5MARPX7FV4IEIDPGYIN6XLSZZWT) · deployed by [`a9f4dd7c…`](https://stellar.expert/explorer/testnet/tx/a9f4dd7cc2882ccb4ac0d82e50232cc5a653cc534eae7803fcd6513ad1eece2a)
WASM `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0`
Sale `offer-sale` 0.2.0 · contract [`CAYQ6SOR3AL4XOTFMBEUATJ56X2KLSTUUJVXKXXYYHJWCKJIEJNDQJUT`](https://stellar.expert/explorer/testnet/contract/CAYQ6SOR3AL4XOTFMBEUATJ56X2KLSTUUJVXKXXYYHJWCKJIEJNDQJUT) · deployed by [`a9855097…`](https://stellar.expert/explorer/testnet/tx/a985509755d5127c6256d38e38c2ae83e2d1e750ad9c0f97989efe2d56522631), upgraded in place by [`779a16b6…`](https://stellar.expert/explorer/testnet/tx/779a16b639b7dfbfb7c360ba8477bdcd3d4b1e512f6964d615e8920ef50fede1)
WASM `49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd`

**Offering that is cancelled** (`t1-demo-5` on chain)
Token `offer-token` 0.2.0 · contract [`CDSY2PJZUBY5EQCYEP2QGZE7GP3NF3IUFRQ2A7CTJLZQHN2Y7F343653`](https://stellar.expert/explorer/testnet/contract/CDSY2PJZUBY5EQCYEP2QGZE7GP3NF3IUFRQ2A7CTJLZQHN2Y7F343653) · deployed by [`80fccdd7…`](https://stellar.expert/explorer/testnet/tx/80fccdd77e86435110c654ce909b0613f37151f424c5bbe894472b0c160e2a32)
WASM `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0`
Sale `offer-sale` 0.2.0 · contract [`CAGFA4RWRBBATYCJQAE5O5GECPTOKMLDVHM4R5BR454JN6UOCIKYDFER`](https://stellar.expert/explorer/testnet/contract/CAGFA4RWRBBATYCJQAE5O5GECPTOKMLDVHM4R5BR454JN6UOCIKYDFER) · deployed by [`4ea3f9a8…`](https://stellar.expert/explorer/testnet/tx/4ea3f9a8e90919308d15d6b63eea91eec0168a7d0abc5081e3244bdda659b1a1), upgraded in place by [`65e74d91…`](https://stellar.expert/explorer/testnet/tx/65e74d917e0c2f7033e3289a7d19ee9aad79391e8cd461f8cb69b70f21cd9ade)
WASM `49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd`

Purchases are paid in a USDC issued by BWB on testnet, the Stellar Asset Contract [`CCW4PRV5…`](https://stellar.expert/explorer/testnet/contract/CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY); the contracts accept any SEP-41 asset given at construction. The full record of every deployment, with constructor arguments and toolchain versions, is [`deployments/testnet.json`](../../deployments/testnet.json), written by the deploy script; the decoded deploy and upgrade transactions are under [`evidence/deploys/`](evidence/deploys/).

### How to verify

Check out the release commit and run `stellar contract build` with Stellar CLI 28.1.0; the Rust toolchain is pinned in [`rust-toolchain.toml`](../../rust-toolchain.toml) and installs itself. `shasum -a 256 target/wasm32v1-none/release/*.wasm` gives the three hashes above, and `stellar contract info hash --id <contract> --network testnet` shows which hash each contract runs today.

## Criterion 2 — met ✅

> Published transactions and decoded events demonstrate eligibility/allocation rejection, investment, pause/unpause, cooling-off, cancellation, and refund state transitions.

### Evidence

The demonstration script, [`scripts/demo-offer-sale.sh`](../../scripts/demo-offer-sale.sh), ran every transition on the two offerings of criterion 1, each step a real transaction. Rejected calls were submitted anyway, so a rejection is a failed transaction on the ledger that carries the contract's error code. The [README's demonstration table](../../README.md#l4-demonstration) lists all 26 steps with their hashes; the transitions D2 names are these:

| Transition | Step | Transaction | What the transaction shows |
|---|---|---|---|
| Eligibility rejection | 4 | [`bb98c609…`](https://stellar.expert/explorer/testnet/tx/bb98c60984c32ada9b0fb770579bbffbf31102d51305df9d3e2ef468f44b96a6) | Failed with `6100 InvestorNotEnabled`: the investor is not on the allowlist |
| Allocation rejection | 5 | [`da1ac6bf…`](https://stellar.expert/explorer/testnet/tx/da1ac6bf4aa09f9e2d1b657878ae119771c1892c1c8143e884d5420dafd3ec97) | Failed with `6101 AllocationExceeded`: 4,000 USDC against a 3,000 cap |
| Investment | 6 | [`490380f8…`](https://stellar.expert/explorer/testnet/tx/490380f877d4d7250b0819cded99fd51c2ab1aab8a578fa0f74f11f4c41b2f62) | `purchased`: 50 tokens, 500 USDC into escrow; `allocation_consumed` on the allowlist |
| Pause | 7 | [`47d1debb…`](https://stellar.expert/explorer/testnet/tx/47d1debb3855e60e43915ef0bf49929d41a3d98c3e69a7c76ef40ae18bd56513) | `pause_changed` by the platform; the purchase that follows (step 8, [`e5ca35a6…`](https://stellar.expert/explorer/testnet/tx/e5ca35a6225c7795f1ef2360046548730293f7b689c55662b0d12cf2225f9d3a)) fails with `1000 EnforcedPause` |
| Unpause | 10 | [`813e200b…`](https://stellar.expert/explorer/testnet/tx/813e200b690a169db6c3dee43b3deba7edb32db810305ed1134b6708b8c2c5e2) | `pause_changed` by the owner; the platform's own attempt (step 9, [`58ff8d0a…`](https://stellar.expert/explorer/testnet/tx/58ff8d0a4b254bb963d2d73f66aa195cd6dad29bc5399fe0212ec2525c05416f)) fails with `6222 NotOwner` |
| Cooling-off | 11 | [`8833a61e…`](https://stellar.expert/explorer/testnet/tx/8833a61e85e46b46cf1d6eecd96735184b0b24427e863c40605b2e94f03f37e7) | `refunded`, reason `cooling_off`: 500 USDC back; `allocation_restored` on the allowlist |
| Cancellation | 20 | [`7ab128c2…`](https://stellar.expert/explorer/testnet/tx/7ab128c2b8b7ee3a13a8f838a5eb70d7c7ccf87578e68ecc7e614acf85c1fa6a) | `cancelled`, then `state_changed` Active → Failed |
| Refund | 21 | [`42a52035…`](https://stellar.expert/explorer/testnet/tx/42a52035d494e23b636bb9f4b2b25fedefd8465b1827d340df880fddf6eadef7) | `refunded`, reason `failed`: the investor paid back; allowlist room restored |

The same run also put on chain the activation of each offering (steps 2 and 17), further purchases (12, 18), close and reopen (13), a purchase after close refused (14), a finalize before the cooling-off windows ran out refused (15), a cancel by a non-owner refused (19), the owner taking back a cancelled offering's tokens (22), and the settlement of the successful offering: finalize with the burn of the unsold supply, release of the tokens, payout, and a late cooling-off refused (23–26; `TODO(stage 2)`).

RPC keeps transactions and their events for about seven days, and testnet resets from time to time, so the decoded result and events of every step are committed under [`evidence/demo/`](evidence/demo/), one JSON file per transaction with an [index](evidence/demo/index.json), captured by [`scripts/capture-evidence.sh`](../../scripts/capture-evidence.sh).

### How to verify

Open any transaction on the explorer: the Events tab shows the decoded events, and a failed transaction shows its contract error code. `scripts/capture-evidence.sh --out /tmp/evidence` re-fetches every result and event from RPC while it still holds them.

## Criterion 3 — met ✅

> A public reproducible query calculates tokenized AUM from issuance and active-position events without PII.

### Evidence

[`scripts/aum.sh`](../../scripts/aum.sh) computes tokenized AUM from the sale contracts through simulated view calls, which need no key, fee or transaction. It reads each sale's accounting rather than replaying its events: `sold_tokens` and `raised` are updated by exactly the transitions that emit `purchased`, `refunded` and `finalized`, so the result is the same, and it does not depend on an event archive (RPC drops events after about seven days). The mapping from events to value, for an indexer that replays them instead, is [interface §8.2](../interface.md#82-what-moves-value).

What counts as AUM here: every token an investor has bought and not been refunded for, in every offering that has not failed. The query takes `sold_tokens` from each sale contract (tokens reserved or already released, net of refunds), multiplies by the offering's price, and sums; it also checks that figure against `raised`, the payment the contract actually holds net of refunds, so the two cannot drift. Nothing personal enters the calculation: the contracts' events and storage hold only addresses and amounts.

```bash
scripts/aum.sh --json docs/scf/evidence/aum.json t1-demo-4 t1-demo-5
```

At ledger `TODO(ledger)`, `TODO(date)`, after the demonstration's settlement:

| Offering | State | Counted | Supply | Sold (units) | Price | Value (USDC) |
|---|---|---|---|---|---|---|
| Offering that succeeds (`t1-demo-4`) | Successful | yes | 800 | 800 | 10.00 | 8,000.00 |
| Offering that is cancelled (`t1-demo-5`) | Failed | no | 1,000 | 0 | 10.00 | 0.00 |
| **Total** | | 1 of 2 | | **800** | | **8,000.00** |

The committed output is [`evidence/aum.json`](evidence/aum.json), with the ledger and timestamp it was taken at.

### How to verify

Run the command above; it needs only the Stellar CLI and `jq`.

## Criterion 4 — met ✅

> Public CI and the evidence index map authorization, boundary, pause, refund, replay/idempotency, and failure scenarios to code, tests, transactions, and events.

### Evidence

CI runs on every change under `contracts/`: formatting, the full test suite, clippy with warnings as errors, and the WASM build ([workflow](../../.github/workflows/contracts.yml); [run on the release](TODO(run link))). The suite has 326 tests (token 80, allowlist 114, sale 132), in modules named after the scenarios the criterion lists, so each row below points at a file rather than a search:

| Scenario | Tests | Code | On chain |
|---|---|---|---|
| Authorization | [sale](../../contracts/offer-sale/src/test/authorization.rs) · [allowlist](../../contracts/offer-allowlist/src/test/authorization.rs) · [token](../../contracts/offer-token/src/test/authorization.rs) | the powers in [`offer-sale/src/types.rs`](../../contracts/offer-sale/src/types.rs); the roles in [`offer-allowlist/src/lib.rs`](../../contracts/offer-allowlist/src/lib.rs) | steps 9 and 19: a signed non-owner refused with `6222 NotOwner` |
| Boundary | [sale](../../contracts/offer-sale/src/test/boundary.rs) · [allowlist](../../contracts/offer-allowlist/src/test/boundary.rs) · [token](../../contracts/offer-token/src/test/boundary.rs) | batch limits and the purchase cap in [`offer-sale/src/lib.rs`](../../contracts/offer-sale/src/lib.rs) | step 5: a purchase over the monthly cap, `6101` |
| Pause | [sale](../../contracts/offer-sale/src/test/pause.rs) · [token](../../contracts/offer-token/src/test/pause.rs) | OpenZeppelin `pausable` on both contracts | steps 7, 8, 10: pause, a purchase refused with `1000`, unpause |
| Refund | [`cooling_off.rs`](../../contracts/offer-sale/src/test/cooling_off.rs); refund cases in [`state.rs`](../../contracts/offer-sale/src/test/state.rs) and [`failure.rs`](../../contracts/offer-sale/src/test/failure.rs) | `cooling_off_refund`, `refund`, and the allowlist's `restore` | steps 11, 21, 25: cooling-off, refund after cancellation, late cooling-off refused with `6225` |
| Replay / idempotency | [sale](../../contracts/offer-sale/src/test/idempotency.rs) · [allowlist](../../contracts/offer-allowlist/src/test/idempotency.rs) · [token](../../contracts/offer-token/src/test/idempotency.rs) | release and refund skip what is already done; a receipt restores once | steps 13, 24, 26: close, reopen, close; release and payout |
| Failure | [sale](../../contracts/offer-sale/src/test/failure.rs) · [allowlist](../../contracts/offer-allowlist/src/test/failure.rs) · [token](../../contracts/offer-token/src/test/failure.rs) | the error catalogues, [`offer-sale/src/errors.rs`](../../contracts/offer-sale/src/errors.rs) and [`offer-allowlist/src/errors.rs`](../../contracts/offer-allowlist/src/errors.rs) | steps 4, 5, 8, 14, 15: each a failed transaction with its code |

The events of every successful step are in the snapshot under [`evidence/demo/`](evidence/demo/). The in-place upgrade of both sale contracts ([`779a16b6…`](https://stellar.expert/explorer/testnet/tx/779a16b639b7dfbfb7c360ba8477bdcd3d4b1e512f6964d615e8920ef50fede1), [`65e74d91…`](https://stellar.expert/explorer/testnet/tx/65e74d917e0c2f7033e3289a7d19ee9aad79391e8cd461f8cb69b70f21cd9ade)) exercised one more scenario on chain, covered by the `upgrade` test modules: storage survives a code swap. Each contract's documentation describes its test modules one by one ([offer-sale.md § Tests](../offer-sale.md#tests)).

### How to verify

`scripts/test.sh --locked` runs the 326 tests; `rustup show` installs the pinned toolchain first. The CI run linked above shows the same commands passing on GitHub's runners.
