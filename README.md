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

From [`deployments/testnet.json`](deployments/testnet.json). The allowlist is global; the token and the sale contract belong to offering `l2-demo-3`:

| Contract | Contract ID | WASM hash |
|---|---|---|
| `offer-allowlist` | [`CAWQW2XWFBO3QHGEHLGXLVCLY4QDVAPR7TPLCH6PVYU4CVLU5IIX3THF`](https://stellar.expert/explorer/testnet/contract/CAWQW2XWFBO3QHGEHLGXLVCLY4QDVAPR7TPLCH6PVYU4CVLU5IIX3THF) | `c60cbd6498f66e6f555df6952646114bb4c0427c4f95611244234195dcf35b20` |
| `offer-token` | [`CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM`](https://stellar.expert/explorer/testnet/contract/CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM) | `40bf9cc67d2cf9e0d2b3c238f8e5b0f3ed3dfb016998453d4ddfc0069037f385` |
| `offer-sale` | `CCL2OOMX4NYI4Y6RDYHFRJ6VKNKLVM7POOZIMKJUSJRUTETDKAIPK5IT` (precomputed, not deployed) | |

The token's whole supply is minted to the sale contract's precomputed address. The allowlist's admin is the `bwb-testnet-platform` identity and its only allocator is the deployer hot key; each sale contract is granted `sale` at its own deploy (L4), signed by the allocator. Earlier instances are superseded and unused: `CBQLP3DC…TQQHJ26` (0.3.0, the receipt book still named `Receipts`), `CDMGBEK5…TJTH7JSD` (0.2.0, where `restore` still required the `sale` role), `CBQCDFMM…656AT6LX` (receipts numbered per sale contract, which made concurrent purchases conflict), `CAWUBAUY…ESWRBKF` (restore by period and amount, before per-purchase receipts) and `CBTU5S5V…KC42XU` (the single-role code before the audit).

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
admin transfer, unauthorized calls and burn. Rejected calls are submitted
anyway, so each rejection is a failed transaction on the ledger with its error
code (`scripts/lib/send_rejected.py`).

## License

Everything in this repository is licensed under [Apache 2.0](LICENSE).

## Contact

- Website: [bwbi.com.br](https://bwbi.com.br)
- Email: contato@bwbi.com.br
