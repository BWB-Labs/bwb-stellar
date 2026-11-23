# BWB smart contracts

The on-chain contracts of BWB Digital Assets, one folder per chain. Both
chains implement the same offering model: an offer token per offering, a
global allowlist of who may invest and how much, and a sale contract that
holds the money until the offering closes.

| Folder | Chain | Stack | What is inside |
|---|---|---|---|
| [`stellar/`](stellar/README.md) | Stellar | Soroban, Rust | Offer token, allowlist and sale contracts; the TypeScript SDK; deploy scripts and per-network manifests; the event indexer |
| [`base/`](base/README.md) | Base (EVM) | Solidity, Foundry | Offer token, allowlist, sale and distributor contracts with their factories; deploy scripts and broadcast records |

Each folder is a self-contained project: build, test and deploy from inside it.

```bash
cd stellar && scripts/test.sh     # Stellar: build fixtures, run the tests
cd base && forge test             # Base: run the tests
```

`base/lib/` holds the Foundry libraries as git submodules. Clone with
`--recurse-submodules`, or run `git submodule update --init --recursive` once.

## Continuous integration

| Workflow | Runs |
|---|---|
| [`contracts`](.github/workflows/contracts.yml) | Stellar: format, tests, clippy, WASM build |
| [`sdk`](.github/workflows/sdk.yml) | Stellar SDK: typecheck, build, tests |
| [`base`](.github/workflows/base.yml) | Base: tests, per-file coverage gate, OpenZeppelin upgrade validation |

## Shared folders

- `docs/scf/`: Stellar Community Fund submission and evidence.

## License

[Apache 2.0](LICENSE).
