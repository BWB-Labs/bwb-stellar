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
| `offer-allowlist` | `OfferSaleAllowlist` | Who may invest and up to how much; one global instance | 1 |
| `offer-sale` | `OfferTokenSale` | Escrow and lifecycle: reserve, finalize, cancel, cooling-off, refund, release | 1 |

`offer-token-v2-fixture` is a test-only second version of the token, used by
its upgrade test. It is never deployed.

Distributors and factories follow in Tranche 2. Each contract has a document in
`docs/` once it lands.

### Testnet

Offering `t1-demo`, from [`deployments/testnet.json`](deployments/testnet.json):

| Contract | Contract ID | WASM hash |
|---|---|---|
| `offer-token` | [`CDHVNF74K2J3LK63BTPEDBMXSY3U2FY4ZYXZZMVNHKMRJPHH2Q6PE557`](https://stellar.expert/explorer/testnet/contract/CDHVNF74K2J3LK63BTPEDBMXSY3U2FY4ZYXZZMVNHKMRJPHH2Q6PE557) | `19a7544def82d9b8b9ff93fdc64b5487486e922cff5af9a834503142db6fe824` |
| `offer-sale` | `CAMB22EZTVFQ5IDQNYKIYGONDNVA4XDWAECULSZZ2QDDPY6D5LTM54XO` (precomputed; deployed in L4) | |

The token's whole supply is minted to the sale contract's precomputed address.

## Build and test

Rust toolchain, target and components are pinned in `rust-toolchain.toml`;
`rustup` installs them on first use. The [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli)
(28.x) is needed to build the WASM and to deploy: OpenZeppelin 0.7.2 enables
soroban-sdk's spec-shaking feature, which only builds through the CLI.

```bash
rustup show                     # installs the pinned toolchain and wasm32v1-none
cargo fmt --all -- --check
scripts/test.sh                 # builds the upgrade fixture, then cargo test --workspace
cargo clippy --all-targets --workspace -- -D warnings
stellar contract build          # WASM to target/wasm32v1-none/release/
```

CI runs exactly these four commands on every pull request. Run the tests through
`scripts/test.sh`: `offer-token`'s upgrade test swaps to a second WASM that only
the Stellar CLI builds. A plain `cargo test` before that fails one test with a
hint saying so.

## Deploy

```bash
stellar keys generate <identity> --network testnet --fund
NETWORK=testnet scripts/offering-addresses.sh <offering-id>   # token and sale addresses
NETWORK=testnet SOURCE=<identity> OFFERING=<offering-id> scripts/deploy.sh offer-token -- \
  --admin G... --xfer_admin G... --pauser G... --upgrader G... \
  --name "..." --symbol ... --supply 1000 --initial_holder <sale address> \
  --whitelist '[]' --offer_uri ipfs://...
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

## License

Everything in this repository is licensed under [Apache 2.0](LICENSE).

## Contact

- Website: [bwbi.com.br](https://bwbi.com.br)
- Email: contato@bwbi.com.br
