# SCF #45 · Tranche 2 · Deliverable 3 evidence

**Deliverable 3 — Distribution Attribution and Commissioning Engine.** This document takes each of D3's four completion criteria, as written in the [awarded submission](https://communityfund.stellar.org/submissions/rec8bhUAafIcSJUzA), and places next to it the code, tests, transactions and CI runs that meet it. A criterion with no evidence yet reads as open.

Everything runs on Stellar testnet. A CI run is cited only for code the platform runs. The evidence index is [`evidence/d3/index.json`](evidence/d3/index.json): per criterion, its status, its evidence and its CI run.

## Criterion 1 — open

> A tagged public release documents the attribution rule version and event-to-ledger mapping.

### Evidence

None yet.

## Criterion 2 — open

> Published anonymized fixtures and Stellar event references reproduce single, idempotent commission entries and reversals.

### Evidence

No commission entry exists yet. The purchases that commissions will be attributed to already go through one public interface, and one has landed on testnet:

| | |
|---|---|
| Sale interface | [`@bwb-labs/sdk`](../../sdk), merged in [Integrations SDK — sale interface, Stellar strategy, npm publishing](https://github.com/BWB-Labs/bwb-stellar/pull/63): `buy`, `authorize` and `purchaseReceipt`, which reads the quotas and USDC paid from the sale's `purchased` event |
| Testnet purchase | [`96dc4b9a…`](https://stellar.expert/explorer/testnet/tx/96dc4b9a4e235e29a8b2397ecc28407076532faba4a1690baca7aadb43afe7de), a fee-bump around [`c91678c0…`](https://stellar.expert/explorer/testnet/tx/c91678c09d3129412046e734a461ccd4dcc8eed4e4c3ba973131cf81c3bfc722): 20 USDC paid to the sale, with its `transfer`, `allocation_consumed` and `purchased` events. Its record is committed in [`evidence/d3/purchase.json`](evidence/d3/purchase.json) |

The purchase was sent from a local backend. The platform does not call the sale interface yet, so the SDK workflow's runs are not cited here.

## Criterion 3 — open

> A public demonstration and evidence export show invested value, units sold, and commission states.

### Evidence

None yet.

## Criterion 4 — open

> Public CI reproduces the multi-office scenario without duplicate attribution or commission entries.

### Evidence

None yet.
