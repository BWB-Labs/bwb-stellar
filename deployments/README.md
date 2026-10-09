# Deployments

One JSON file per network, written by `scripts/deploy.sh`. These files are the
source of truth for contract IDs and WASM hashes. Testnet resets periodically,
so every redeploy is a commit that updates `testnet.json`.

- `offerings.<id>.token` and `offerings.<id>.sale`: one entry per offering
  contract. Each records the salt its address derives from (see
  `scripts/lib/offering.sh`). A `"status": "precomputed"` entry is an address
  computed before that contract is deployed; the token's supply is minted
  there. Its own deploy replaces it with a `"status": "deployed"` entry.
- `contracts.<crate>`: contracts with a single instance per network
  (`offer-allowlist`).

`scripts/offering-addresses.sh <id>` prints an offering's two addresses without
deploying anything.
