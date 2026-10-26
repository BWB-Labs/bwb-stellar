# BWB on Stellar

Soroban contracts and Stellar integration code for BWB Digital Assets. This is
the public side of the platform; the application, its Convex backend and the
EVM contracts live in a private repository.

**Status:** nothing is deployed to mainnet. No external audit has been
performed yet.

## Layout

| Path | What | Owner |
|---|---|---|
| `contracts/` | Soroban contracts, one crate each (see table below) | contracts |
| `scripts/` | `deploy.sh`: build, upload, deploy, record | contracts |
| `deployments/` | Per-network manifests with contract IDs and WASM hashes | contracts |
| `docs/` | Backend ↔ contracts interface document and one doc per contract | contracts |
| `sdk/` | TypeScript adapters (stubs) | BWB team |
| `audit/` | Placeholder for audit reports | BWB team |

`indexer/` is reserved for the contract-event indexer in a later tranche.

### Contracts

The contracts port BWB's Base (EVM) contracts to Soroban with the same
semantics, built on [OpenZeppelin Stellar Contracts](https://github.com/OpenZeppelin/stellar-contracts) 0.7.2.

| Crate | Base counterpart | Role | Tranche |
|---|---|---|---|
| `offer-token` | `OfferERC20` | Investor position in one offering; transfer whitelist, pause, admin transfer, metadata ([docs](docs/offer-token.md)) | 1 |
| `offer-allowlist` | `OfferSaleAllowlist` | Who may invest and up to how much, per UTC month; one global instance ([docs](docs/offer-allowlist.md)) | 1 |
| `offer-sale` | `OfferTokenSale` | Escrow and lifecycle: reserve, close, finalize, cancel, cooling-off, refund, release ([docs](docs/offer-sale.md)) | 1 |

`offer-token-upgrade-fixture`, `offer-allowlist-upgrade-fixture` and `offer-sale-upgrade-fixture` (each under its crate's
`upgrade-fixture/`) are test-only second versions of the contracts, used by their upgrade tests. They are never deployed.

Distributors and factories follow in Tranche 2.

### Testnet

From [`deployments/testnet.json`](deployments/testnet.json). The allowlist is global; each offering has a token and a sale contract. `l4-demo-1` and `l4-demo-2` are the L4 demonstration's offerings; `l2-demo-3` is L2's, whose sale address was never deployed.

| Contract | Contract ID | WASM hash |
|---|---|---|
| `offer-allowlist` 0.5.0 | [`CAWQW2XWFBO3QHGEHLGXLVCLY4QDVAPR7TPLCH6PVYU4CVLU5IIX3THF`](https://stellar.expert/explorer/testnet/contract/CAWQW2XWFBO3QHGEHLGXLVCLY4QDVAPR7TPLCH6PVYU4CVLU5IIX3THF) | `443ca5cd132525fc476516467989a41e006c297c278daf712b68a827554cfcd1` (upgraded in place from `c60cbd64…`, [tx](https://stellar.expert/explorer/testnet/tx/38cf17f90b9b17f3a7e7b269afc4912855aff2336d85698878f1bc0d937dc200)) |
| `offer-token` l4-demo-1 (0.2.0) | [`CAV5WNZHOLJUN7NK6NSKQH73VASYDYSZNE4QTS4DUSDEXK454HQ7FK3V`](https://stellar.expert/explorer/testnet/contract/CAV5WNZHOLJUN7NK6NSKQH73VASYDYSZNE4QTS4DUSDEXK454HQ7FK3V) | `9d72cf50dacd2882a38f349321a82e37ec52e9d0ca3a1521f6587590f2310b71` |
| `offer-sale` l4-demo-1 | [`CBYL7BH4BVEBE4RPOKH5WHFQOQK4WBLJ46G2MVRONG3OUHIY5LQYG2DH`](https://stellar.expert/explorer/testnet/contract/CBYL7BH4BVEBE4RPOKH5WHFQOQK4WBLJ46G2MVRONG3OUHIY5LQYG2DH) | `7367f872c91773251b5521645f8377e8a2d2a0a18ea42235f69d80aa3bc8cfcd` |
| `offer-token` l4-demo-2 (0.2.0) | [`CAMYZHD3QV57AG4BFTUSR5R2NFBOGZU624FRY3SVG4PVC2JYXJLDKHPZ`](https://stellar.expert/explorer/testnet/contract/CAMYZHD3QV57AG4BFTUSR5R2NFBOGZU624FRY3SVG4PVC2JYXJLDKHPZ) | `9d72cf50dacd2882a38f349321a82e37ec52e9d0ca3a1521f6587590f2310b71` |
| `offer-sale` l4-demo-2 | [`CATBCDJUGDEXVQI4DWQFW36ANF7GXI4A6HNHQ65HWZEQGEEQCKZNQ5VQ`](https://stellar.expert/explorer/testnet/contract/CATBCDJUGDEXVQI4DWQFW36ANF7GXI4A6HNHQ65HWZEQGEEQCKZNQ5VQ) | `7367f872c91773251b5521645f8377e8a2d2a0a18ea42235f69d80aa3bc8cfcd` |
| demo USDC (payment asset) | [`CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY`](https://stellar.expert/explorer/testnet/contract/CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY) | a Stellar Asset Contract for `USDC:GDGBRCFA…`, issued by the `bwb-testnet-usdc` identity; not Circle's |
| `offer-token` l2-demo-3 (0.1.0) | [`CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM`](https://stellar.expert/explorer/testnet/contract/CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM) | `40bf9cc67d2cf9e0d2b3c238f8e5b0f3ed3dfb016998453d4ddfc0069037f385` |

Each token's whole supply is minted to its sale contract's precomputed address. The allowlist's admin is the `bwb-testnet-platform` identity and its only allocator is the deployer hot key; each sale contract is granted `sale` at its own deploy, signed by the allocator. The sale contracts' owner is the `bwb-testnet-treasury` identity, their pauser and upgrader the platform. Earlier allowlist instances are superseded and unused: `CBQLP3DC…TQQHJ26` (0.3.0, the receipt book still named `Receipts`), `CDMGBEK5…TJTH7JSD` (0.2.0, where `restore` still required the `sale` role), `CBQCDFMM…656AT6LX` (receipts numbered per sale contract, which made concurrent purchases conflict), `CAWUBAUY…ESWRBKF` (restore by period and amount, before per-purchase receipts) and `CBTU5S5V…KC42XU` (the single-role code before the audit).

**Upgrade order.** The two L4 sale contracts on testnet run code from before the audit rounds, which calls the allowlist's `restore_all` directly. The repository's allowlist 0.6.0 no longer has it, so upgrade the sale contracts first and the allowlist after; the other way round, their refunds fail until they are upgraded.

### L4 demonstration

`scripts/demo-offer-sale.sh` runs the grant's demonstration on the two offerings above, each step a transaction with its explorer link; rejected calls are submitted anyway, so each rejection is a failed transaction on the ledger with its error code. Stage 1 ran on 2026-10-22 (steps 1–15 on `l4-demo-1`, the success path up to the close; 16–22 on `l4-demo-2`, the failure path). Stage 2 (`STAGE=2`: finalize with the burn, release, the payout and a refused late cooling-off) runs once `l4-demo-1`'s last cooling-off window has run out, from 2026-10-23 20:36 UTC; its hashes are added then.

| Step | What | Transaction |
|---|---|---|
| 1 | The allocator grants the sale contract the allowlist's sale role | [`2497731f…`](https://stellar.expert/explorer/testnet/tx/2497731f3db1cacfc8d1e90ac4ed889ea31f01aac79030e49f5cde60c0c580bd) |
| 2 | Anyone activates: the sale holds the whole supply and the role | [`5c34cf85…`](https://stellar.expert/explorer/testnet/tx/5c34cf8581a4ffd1b056291842e22ed85027bbea6cef200e2a6cfbdd668f2fb4) |
| 3 | The allocator allows investor A up to 3,000 USDC this month | [`4823b307…`](https://stellar.expert/explorer/testnet/tx/4823b30701c0054cf25ffc15bd4d96d1ecc8070283dd6706a3b8c0b22b5a4c2c) |
| 4 | Eligibility rejection: investor B, not on the allowlist, buys 10 tokens (6100 InvestorNotEnabled) | [`d1926957…`](https://stellar.expert/explorer/testnet/tx/d1926957785787c84b5f10fb516a01313f19b8b4671fec3e39661a2684630767) |
| 5 | Allocation rejection: investor A buys 400 tokens, 4,000 USDC over a 3,000 cap (6101 AllocationExceeded) | [`2051820e…`](https://stellar.expert/explorer/testnet/tx/2051820e3bc60f706aaa1e3768da4ff944eb28bc4917470346f48a571c096592) |
| 6 | Purchase: investor A buys 50 tokens for 500 USDC | [`f9b1fb53…`](https://stellar.expert/explorer/testnet/tx/f9b1fb53f9c022968e3ce02696044be82d36b717d8b80e20621f094c324f6e53) |
| 7 | The platform pauses the sale | [`1b54187f…`](https://stellar.expert/explorer/testnet/tx/1b54187f144a61d9b47d8889c6406da02487758e75c09d66a5e69806d7b7d472) |
| 8 | Purchase while paused: rejected (1000 EnforcedPause) | [`9534b7a5…`](https://stellar.expert/explorer/testnet/tx/9534b7a58d6b9f79d90005035aefc8b03b6c6ee06df8807b635acd0d5bfca281) |
| 9 | The platform tries to unpause: rejected, only the owner unpauses (6222 NotOwner) | [`9dd613ef…`](https://stellar.expert/explorer/testnet/tx/9dd613efd5405cafcd79a000289e354dfa08f8351db24310e78391f7a8eecb82) |
| 10 | The owner unpauses | [`fe50d739…`](https://stellar.expert/explorer/testnet/tx/fe50d739e4bb02de3423b08e6101406cba604188d920c0ac9d65342c5908095c) |
| 11 | Cooling-off: investor A undoes the purchase and gets the 500 USDC back; the allowlist room returns | [`32c48381…`](https://stellar.expert/explorer/testnet/tx/32c483811a5923ddc017fee079f41a8e6466754be5546ebbffa1013cf7d97aac) |
| 12 | Purchases: investor A buys 300 tokens; the allocator allows investor B, who buys 500 | [`04318831…`](https://stellar.expert/explorer/testnet/tx/04318831b6fbb16ba335a5c590db41f6132b78e03a0367b0ca6405fad73f66f7), [`2f419a79…`](https://stellar.expert/explorer/testnet/tx/2f419a79b0f97a890a845dfa2da3c73101eabeb7dee6a51c984948e7e6120d2e), [`536e88e6…`](https://stellar.expert/explorer/testnet/tx/536e88e69d661b70f74a83a9ef4b8b870082c96eb74cf9ab2664067f4d030b2b) |
| 13 | The owner closes buying, reopens, and closes again | [`5307d363…`](https://stellar.expert/explorer/testnet/tx/5307d363f2db9081ee81bf6309dfbacbe37bf4da49e44410d8fd0637c1174f25), [`7726fda5…`](https://stellar.expert/explorer/testnet/tx/7726fda50779031eee811de93e32069c9b1c11d7fd9325d38e6d744d4470a78a), [`9cc043e5…`](https://stellar.expert/explorer/testnet/tx/9cc043e5f84bd364889378b6759613b5e441343e7d9fefe023c6e65a6aa75e79) |
| 14 | Purchase after close: rejected (6201 NotActive) | [`0f0c069b…`](https://stellar.expert/explorer/testnet/tx/0f0c069b38a10048db8da40c2e9a839e6a315e18cb8f1cb1ea62dcf152c5009b) |
| 15 | Finalize before the windows run out: rejected (6214 CoolingOffOpen) | [`1b52e71d…`](https://stellar.expert/explorer/testnet/tx/1b52e71d71299a2fae29ac414fdf02474937459044b68d40a7e2e4f98312f585) |
| 16 | The allocator grants the sale contract the allowlist's sale role | [`da7dee2f…`](https://stellar.expert/explorer/testnet/tx/da7dee2f56753fcb23e82a25eb0da78e5c01118528a655a2aea2605f4e27a81f) |
| 17 | Anyone activates: the sale holds the whole supply and the role | [`a1bd18a4…`](https://stellar.expert/explorer/testnet/tx/a1bd18a4c26daabf58a6c8218526256e5a9e2b18585ba657c0e00247c11618e8) |
| 18 | The allocator raises investor A's cap to 4,000 USDC (the cap is per month across offerings); A buys 100 tokens | [`c126b916…`](https://stellar.expert/explorer/testnet/tx/c126b9165e7c544a5942bb9354843cfefe55010bf5bde7a1e6c40fd7c6add18e), [`48734d9d…`](https://stellar.expert/explorer/testnet/tx/48734d9ddc6352e6f84bbcf6584ee07ceaeb5d4030a3438c0b0581c865fcea4a) |
| 19 | Investor B tries to cancel the offering: rejected (6222 NotOwner) | [`98043472…`](https://stellar.expert/explorer/testnet/tx/98043472583467b972ff5d2c21ec9c39d715c3ba19b95c52c5e3cdb31ab9c163) |
| 20 | The owner cancels the offering | [`34961277…`](https://stellar.expert/explorer/testnet/tx/349612771c6c2a94b3d1ed7bd50c4bc8cfbabfab85596dc5439d9581408fb054) |
| 21 | Refund: anyone refunds investor A; the money and the allowlist room come back | [`12867dab…`](https://stellar.expert/explorer/testnet/tx/12867dab74d9b55b9a2f6558b71fa4a77f3e6da3f02f1cf5e0e8c76a5b13cc4f) |
| 22 | The owner withdraws the tokens of the failed offering | [`36c0a63c…`](https://stellar.expert/explorer/testnet/tx/36c0a63c04b9b5f9334a93adeb3918c2e8154175931dfa5f66ca743713d094f7) |

## Build and test

Rust toolchain, target and components are pinned in `rust-toolchain.toml`;
`rustup` installs them on first use. The [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli)
(28.x) is needed to build the WASM and to deploy: OpenZeppelin 0.7.2 enables
soroban-sdk's spec-shaking feature, which only builds through the CLI.

```bash
rustup show                     # installs the pinned toolchain and wasm32v1-none
cargo fmt --all -- --check
scripts/test.sh                 # builds the upgrade fixtures, then cargo test --workspace
cargo clippy --all-targets --workspace -- -D warnings
stellar contract build          # WASM to target/wasm32v1-none/release/
```

CI runs exactly these four commands on every pull request. Run the tests through
`scripts/test.sh`: the upgrade tests of the three contracts swap to second
WASMs that only the Stellar CLI builds. A plain `cargo test` before that fails
one test per crate with a hint saying so.

## Deploy

```bash
stellar keys generate <identity> --network testnet --fund
NETWORK=testnet scripts/offering-addresses.sh <offering-id>   # token and sale addresses
NETWORK=testnet SOURCE=<identity> OFFERING=<offering-id> scripts/deploy.sh offer-token -- \
  --power_holders '{"admin":"G...","xfer_admin":"G...","pauser":"G...","upgrader":"G..."}' \
  --metadata '{"name":"...","symbol":"...","offer_uri":"ipfs://..."}' \
  --supply 1000 --initial_holder <sale address> --whitelist '[]'
NETWORK=testnet SOURCE=<identity> OFFERING=<offering-id> scripts/deploy.sh offer-sale -- \
  --power_holders '{"owner":"G...","pauser":"G...","upgrader":"G..."}' \
  --contracts '{"token":"<token address>","payment_asset":"<USDC contract>","allowlist":"<allowlist>"}' \
  --config '{"price":"100000000","cooling_off_secs":432000,"min_success_percent":75}'
```

The sale contract is then granted `sale` on the allowlist by an allocator and
activated by anyone (both in `scripts/demo-offer-sale.sh`).

The global allowlist takes no `OFFERING`; its admin and initial allocators are
constructor arguments:

```bash
NETWORK=testnet SOURCE=<identity> scripts/deploy.sh offer-allowlist -- \
  --admin "$(stellar keys address bwb-testnet-platform)" \
  --allocators "[\"$(stellar keys address bwb-testnet-deployer)\"]"
```

An offering's token and sale contract live at addresses derived from the
deployer and the offering identifier, so both are known before either is
deployed and a retry finds the same ones. The script builds the crate, uploads
the WASM, deploys with the constructor arguments given after `--`, and records
contract ID, salt, WASM hash, deploy transaction, deployer, toolchain versions
and explorer links in `deployments/<network>.json`. Administrative powers are
constructor arguments; the deployer identity only pays fees. Pass `--network`
explicitly to any manual `stellar` command: a `STELLAR_NETWORK` in your
environment otherwise wins.

`OFFERING=<offering-id> scripts/demo-offer-token.sh` then exercises a deployed
token on testnet: transfers allowed and rejected by the whitelist, pause,
admin transfer, unauthorized calls and burn. `STAGE=1 scripts/demo-offer-sale.sh`
and, after the cooling-off window, `STAGE=2`, exercise the two demo sale
contracts end to end. Rejected calls are submitted anyway, so each rejection is
a failed transaction on the ledger with its error code
(`scripts/lib/send_rejected.py`): a call that touches only its own storage
names its keys; a call that reaches other contracts, such as a purchase,
reuses the simulated footprint of a passing twin.

## License

Everything in this repository is licensed under [Apache 2.0](LICENSE).

## Contact

- Website: [bwbi.com.br](https://bwbi.com.br)
- Email: contato@bwbi.com.br
