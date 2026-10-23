#!/usr/bin/env bash
# Run the contract tests the same way CI does.
#
#   scripts/test.sh [cargo test args...]
#
# The upgrade tests of offer-token, offer-allowlist and offer-sale each swap to a second
# binary, the crate's upgrade-fixture. Those WASMs have to exist before the
# tests compile, and only the Stellar CLI builds them (OpenZeppelin 0.7.2
# enables soroban-sdk's spec-shaking feature). So: build the fixtures, then
# test.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v stellar >/dev/null || {
  echo "the Stellar CLI is required: https://developers.stellar.org/docs/tools/cli/install-cli" >&2
  exit 1
}

# Copied to a fixed folder so the tests find them wherever cargo builds
# (CARGO_TARGET_DIR included).
echo ">> building the upgrade fixtures"
for fixture in offer-token-upgrade-fixture offer-allowlist-upgrade-fixture offer-sale-upgrade-fixture; do
  stellar contract build --package "$fixture" --out-dir target/fixtures >/dev/null
done

echo ">> cargo test"
cargo test --workspace "$@"
