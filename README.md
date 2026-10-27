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

From [`deployments/testnet.json`](deployments/testnet.json). The allowlist is global; each offering has a token and a sale contract. `t1-demo-1` and `t1-demo-3` are the demonstration's offerings; `l2-demo-3` is L2's, whose sale address was never deployed. The allowlist and the four contracts of `t1-demo-1` and `t1-demo-3` were deployed fresh on 2026-10-26 from commit `a0a9dd8`; a `stellar contract build` of that commit, run twice, gives the WASM hashes below.

| Contract | Contract ID | WASM hash | Deploy |
|---|---|---|---|
| `offer-allowlist` 0.6.0 | [`CASHJ3GSNWKEB4R35AZ4I52ME7RTKPNBOZMR3O5H4C5SQZ4WWQKZYT6A`](https://stellar.expert/explorer/testnet/contract/CASHJ3GSNWKEB4R35AZ4I52ME7RTKPNBOZMR3O5H4C5SQZ4WWQKZYT6A) | `577b1c184ec72a065d1a7e5dcc462de0343ac63af49de8c758fdb0d77f7e5597` | [`d6ca8379…`](https://stellar.expert/explorer/testnet/tx/d6ca8379269c672802e4c0ba4d79d6044305179c4811e0fcc69743b8044fcee5) |
| `offer-token` t1-demo-1 (0.2.0) | [`CBJCLJOQU5ZPD5J76YWTGH47XEOMICLWXRK6KWV2ZUFQ2H4K7QV7VYJP`](https://stellar.expert/explorer/testnet/contract/CBJCLJOQU5ZPD5J76YWTGH47XEOMICLWXRK6KWV2ZUFQ2H4K7QV7VYJP) | `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0` | [`ef50a123…`](https://stellar.expert/explorer/testnet/tx/ef50a123122ee8ee5e18982f56a561b2c5afcbbe2537f43cc8f31636affaed46) |
| `offer-sale` t1-demo-1 (0.1.0) | [`CBCZ2EHNSFKKLJQAFIPUA6KV2CHMMVQOVISUJTSIVUTHFBWDVADVIF5M`](https://stellar.expert/explorer/testnet/contract/CBCZ2EHNSFKKLJQAFIPUA6KV2CHMMVQOVISUJTSIVUTHFBWDVADVIF5M) | `02425e9f9f0af601d6567b92a269259a28e3da926fcad36312ba93c7675373c5` | [`7f5847d9…`](https://stellar.expert/explorer/testnet/tx/7f5847d94e076d380abd2f9fb2364f6a81a41f335545d6ad4453227059efc6a5) |
| `offer-token` t1-demo-3 (0.2.0) | [`CDWNAKEQ7HDNF3MHDVJ3F6ZR4WNKDMVVUJS3SQ2NI7JRBIBAZWH56QFA`](https://stellar.expert/explorer/testnet/contract/CDWNAKEQ7HDNF3MHDVJ3F6ZR4WNKDMVVUJS3SQ2NI7JRBIBAZWH56QFA) | `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0` | [`763ec770…`](https://stellar.expert/explorer/testnet/tx/763ec770efb46008e27da953d7341ac1af22b6650862f7562a857077b7d80b5c) |
| `offer-sale` t1-demo-3 (0.1.0) | [`CCDTVJYQ5UNB7OZSTOGRHZHM74RSLC23SCCXI5JRXYDQYH7RYGMCQQRV`](https://stellar.expert/explorer/testnet/contract/CCDTVJYQ5UNB7OZSTOGRHZHM74RSLC23SCCXI5JRXYDQYH7RYGMCQQRV) | `02425e9f9f0af601d6567b92a269259a28e3da926fcad36312ba93c7675373c5` | [`b9420580…`](https://stellar.expert/explorer/testnet/tx/b94205803fa5f7bdb379df35cc5733799beda67c670421af56eec3fecd16bca5) |
| demo USDC (payment asset) | [`CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY`](https://stellar.expert/explorer/testnet/contract/CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY) | a Stellar Asset Contract for `USDC:GDGBRCFA…`, issued by the `bwb-testnet-usdc` identity; not Circle's | — |
| `offer-token` l2-demo-3 (0.1.0) | [`CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM`](https://stellar.expert/explorer/testnet/contract/CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM) | `40bf9cc67d2cf9e0d2b3c238f8e5b0f3ed3dfb016998453d4ddfc0069037f385` | [`4a9856e2…`](https://stellar.expert/explorer/testnet/tx/4a9856e2587fe1cfade5a83f359a85260d70abbecb95507e5c8cae79c4888a6f) |

Each token's whole supply is minted to its sale contract's precomputed address. The allowlist's admin is the `bwb-testnet-platform` identity and its only allocator is the deployer hot key; each sale contract is granted `sale` at its own deploy, signed by the allocator. The sale contracts' owner is the `bwb-testnet-treasury` identity, their pauser and upgrader the platform. Earlier allowlist instances are superseded and unused: `CAWQW2XW…IIX3THF` (0.5.0, the first L4 run's, upgraded in place from `c60cbd64…`), `CBQLP3DC…TQQHJ26` (0.3.0, the receipt book still named `Receipts`), `CDMGBEK5…TJTH7JSD` (0.2.0, where `restore` still required the `sale` role), `CBQCDFMM…656AT6LX` (receipts numbered per sale contract, which made concurrent purchases conflict), `CAWUBAUY…ESWRBKF` (restore by period and amount, before per-purchase receipts) and `CBTU5S5V…KC42XU` (the single-role code before the audit).

Superseded offerings, kept in the manifest: `l4-demo-1` and `l4-demo-2`, the first L4 run on 2026-10-22, with the 0.5.0 allowlist and sale contracts from before the audit rounds (`7367f872…`); `l4-demo-1` stays in Closing, its stage 2 never run. The offering id `t1-demo-2` is taken on testnet by an L2-era token (`CAWMEO3F…O4TSENQT`, older code), so the failure path runs on `t1-demo-3`; the sale contract deployed at `t1-demo-2`'s address (`CA23WY2X…FP2AWSAL`) points at that token, was never activated and is unused. Neither is in the manifest.

### L4 demonstration

`scripts/demo-offer-sale.sh` runs the grant's demonstration on the two offerings above, each step a transaction with its explorer link; rejected calls are submitted anyway, so each rejection is a failed transaction on the ledger with its error code. Stage 1 ran on 2026-10-26 (steps 1–15 on `t1-demo-1`, the success path up to the close; 16–22 on `t1-demo-3`, the failure path). Stage 2 (`STAGE=2`: finalize with the burn, release, the payout and a refused late cooling-off) runs once `t1-demo-1`'s last cooling-off window has run out, from 2026-10-27 20:00:38 UTC (`finalizable_at` 1793131238); its hashes are added then.

| Step | What | Transaction |
|---|---|---|
| 1 | The allocator grants the sale contract the allowlist's sale role | [`d9bd1482…`](https://stellar.expert/explorer/testnet/tx/d9bd14827f5f2a7194f2bfb8547d52c6198521d99871f996eca1e36112c76f50) |
| 2 | Anyone activates: the sale holds the whole supply and the role | [`00e7e616…`](https://stellar.expert/explorer/testnet/tx/00e7e616b9d1d3cf0aaf8e9c30804d3d647efcbe83840a45c0b567879ded5673) |
| 3 | The allocator allows investor A up to 3,000 USDC this month | [`8e5f7df4…`](https://stellar.expert/explorer/testnet/tx/8e5f7df490e1a4c8e8793fde0d1f9e52c3fc083b25b9f2edda0a993ed2fcb9bc) |
| 4 | Eligibility rejection: investor B, not on the allowlist, buys 10 tokens (6100 InvestorNotEnabled) | [`7c278f13…`](https://stellar.expert/explorer/testnet/tx/7c278f13420ae5b305f3d5ed7c2793265ddb3b7af7ac0d5f47f35649575bf87c) |
| 5 | Allocation rejection: investor A buys 400 tokens, 4,000 USDC over a 3,000 cap (6101 AllocationExceeded) | [`50ab3689…`](https://stellar.expert/explorer/testnet/tx/50ab36893b53b3681ef177bbe1aec372458a53ce135365d281639253315d779b) |
| 6 | Purchase: investor A buys 50 tokens for 500 USDC | [`42d3146d…`](https://stellar.expert/explorer/testnet/tx/42d3146d2c0c1ce3357c853a5cfb7bf6495a926dc11fe6eaf6146d3ed006aa83) |
| 7 | The platform pauses the sale | [`8d42e26e…`](https://stellar.expert/explorer/testnet/tx/8d42e26e24ef98242436c16444a0b0e9039e84e248603153d119457f772d7e77) |
| 8 | Purchase while paused: rejected (1000 EnforcedPause) | [`602950bf…`](https://stellar.expert/explorer/testnet/tx/602950bfe1a6947870eb610208c9f24a5ecbfeb36051ef240b59c87f8db75fc9) |
| 9 | The platform tries to unpause: rejected, only the owner unpauses (6222 NotOwner) | [`ebe82f49…`](https://stellar.expert/explorer/testnet/tx/ebe82f49a3e956467f2e1050620529a390e74cf775d586daf5add5c79c6302ea) |
| 10 | The owner unpauses | [`1d569d1c…`](https://stellar.expert/explorer/testnet/tx/1d569d1c3a47b4e4633486033165dc1867ab08036ae97c2ed53cd90c9c6bab99) |
| 11 | Cooling-off: investor A undoes the purchase and gets the 500 USDC back; the allowlist room returns | [`80ca45e2…`](https://stellar.expert/explorer/testnet/tx/80ca45e27c36f276ee631329d3704e315758d75fae9783f57bea1a5a5b8ac0b8) |
| 12 | Purchases: investor A buys 300 tokens; the allocator allows investor B, who buys 500 | [`ccc19810…`](https://stellar.expert/explorer/testnet/tx/ccc19810044a6d6759fad46791a5e286c4192d9807c1c2f62b51151c94683315), [`2d713325…`](https://stellar.expert/explorer/testnet/tx/2d713325b0beb9768fa5db1e2b6fd7c728d8cd95e6c0cca9a19cef8125a74274), [`fbc4ca8f…`](https://stellar.expert/explorer/testnet/tx/fbc4ca8f89dc4dc5d71abde2d5fbfdfb45195de2d872a418ce06357fd6015c3c) |
| 13 | The owner closes buying, reopens, and closes again | [`c8ac2901…`](https://stellar.expert/explorer/testnet/tx/c8ac29016853969b094eb3c688291992f32f0487c0b6cee18f33986fa9c645cf), [`14b9a96d…`](https://stellar.expert/explorer/testnet/tx/14b9a96d58683af176829146f1df0f9578f95c362faf95fe55ce0b74a522c809), [`29101129…`](https://stellar.expert/explorer/testnet/tx/29101129cd438537df02b942a9e9ad697cb70f7b77a519c56b7abe62def2a9b3) |
| 14 | Purchase after close: rejected (6201 NotActive) | [`6cd5ffb6…`](https://stellar.expert/explorer/testnet/tx/6cd5ffb6486b193b7532d79453c4c38897519921ba080d232fccbd73de82ed5a) |
| 15 | Finalize before the windows run out: rejected (6214 CoolingOffOpen) | [`06ac6c6d…`](https://stellar.expert/explorer/testnet/tx/06ac6c6da8e277193b3c7db57e0c30d3f89ac234d009737cba42c6a0c52c7e4e) |
| 16 | The allocator grants the sale contract the allowlist's sale role | [`c2039a06…`](https://stellar.expert/explorer/testnet/tx/c2039a06a51bbb9dda0866fabfd7cfe2c45425be6db03e179bb42f6aedfc0cb0) |
| 17 | Anyone activates: the sale holds the whole supply and the role | [`e5eff9cc…`](https://stellar.expert/explorer/testnet/tx/e5eff9cc4a75930b7345108d34a84bb8045c6e101fbe812bf77cf88aaa794179) |
| 18 | The allocator raises investor A's cap to 4,000 USDC (the cap is per month across offerings); A buys 100 tokens | [`7344638c…`](https://stellar.expert/explorer/testnet/tx/7344638c9546ea0b5bc64040f46ff0c9c6ab99cf5a006836e55958d3afd32a59), [`360778ff…`](https://stellar.expert/explorer/testnet/tx/360778ffa17b4b51de75648ee89b7f40c276f89ec4c9535c7ea403b9af63ac0c) |
| 19 | Investor B tries to cancel the offering: rejected (6222 NotOwner) | [`f910db08…`](https://stellar.expert/explorer/testnet/tx/f910db082d534abbcc29d68a77d4d384286efb326f4bf543a6323cfd4fe56e37) |
| 20 | The owner cancels the offering | [`fef2c69e…`](https://stellar.expert/explorer/testnet/tx/fef2c69ed88d5a35c1cabe21ef6540c8c905afd5ef9dcb3553d1eec97d636333) |
| 21 | Refund: anyone refunds investor A; the money and the allowlist room come back | [`ddede9ed…`](https://stellar.expert/explorer/testnet/tx/ddede9edaf2d165db429298c4197daeb530312bc41eac51fd412e88fccc3b9b1) |
| 22 | The owner withdraws the tokens of the failed offering | [`21ca8179…`](https://stellar.expert/explorer/testnet/tx/21ca8179e24a77404dfb30b09dae2e642c8a7b7cb1a77c71932e15c86b752b7e) |

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
