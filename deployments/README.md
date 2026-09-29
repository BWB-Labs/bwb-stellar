One JSON file per network, written by `scripts/deploy.sh`. These files are the
source of truth for contract IDs and WASM hashes. Testnet resets periodically,
so every redeploy is a commit that updates `testnet.json`.
