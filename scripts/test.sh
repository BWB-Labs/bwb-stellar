#!/usr/bin/env bash
# Run the contract tests the same way CI does.
#
#   scripts/test.sh [cargo test args...]
#
# offer-token's upgrade test swaps to a second binary, the
# offer-token-v2-fixture crate. That WASM has to exist before the tests
# compile, and only the Stellar CLI builds it (OpenZeppelin 0.7.2 enables
# soroban-sdk's spec-shaking feature). So: build the fixture, then test.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v stellar >/dev/null || {
  echo "the Stellar CLI is required: https://developers.stellar.org/docs/tools/cli/install-cli" >&2
  exit 1
}

echo ">> building the upgrade fixture"
stellar contract build --package offer-token-v2-fixture >/dev/null

echo ">> cargo test"
cargo test --workspace "$@"
