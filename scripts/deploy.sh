#!/usr/bin/env bash
# Build, upload and deploy one contract crate, then record it in
# deployments/<network>.json.
#
#   scripts/deploy.sh <crate> [-- <constructor args...>]
#
# Environment:
#   NETWORK   stellar network name from `stellar network ls` (default: testnet)
#   SOURCE    stellar keystore identity that signs and pays fees
#             (default: bwb-testnet-deployer)
#
# Example:
#   scripts/deploy.sh offer-token -- --admin GABC...
#
# Roles (admin, operator, ...) are constructor arguments, never the deployer.
set -euo pipefail

CRATE="${1:?usage: scripts/deploy.sh <crate> [-- <constructor args...>]}"
shift
if [[ "${1:-}" == "--" ]]; then shift; fi
CTOR_ARGS=("$@")

NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-bwb-testnet-deployer}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/deployments/$NETWORK.json"
WASM="$ROOT/target/wasm32v1-none/release/${CRATE//-/_}.wasm"

case "$NETWORK" in
  testnet) EXPLORER="https://stellar.expert/explorer/testnet"; HORIZON="https://horizon-testnet.stellar.org" ;;
  mainnet|public) EXPLORER="https://stellar.expert/explorer/public"; HORIZON="https://horizon.stellar.org" ;;
  *) echo "unknown network: $NETWORK" >&2; exit 1 ;;
esac

cd "$ROOT"

echo ">> building $CRATE"
stellar contract build --package "$CRATE" >/dev/null
[[ -f "$WASM" ]] || { echo "wasm not found: $WASM" >&2; exit 1; }

echo ">> uploading wasm"
WASM_HASH="$(stellar contract upload --wasm "$WASM" --network "$NETWORK" --source "$SOURCE" 2>/dev/null | tail -n1)"
echo "   wasm hash: $WASM_HASH"

echo ">> deploying"
CONTRACT_ID="$(stellar contract deploy --wasm-hash "$WASM_HASH" --network "$NETWORK" --source "$SOURCE" --alias "$CRATE" -- "${CTOR_ARGS[@]}" 2>/dev/null | tail -n1)"
echo "   contract id: $CONTRACT_ID"

DEPLOYER="$(stellar keys address "$SOURCE")"
# The deploy transaction is the deployer's most recent one.
DEPLOY_TX="$(curl -sf "$HORIZON/accounts/$DEPLOYER/transactions?order=desc&limit=1" | python3 -c 'import sys,json;print(json.load(sys.stdin)["_embedded"]["records"][0]["hash"])')"
DEPLOYED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUSTC="$(rustc --version | awk '{print $2}')"
CLI="$(stellar --version | head -n1 | awk '{print $2}')"
SDK="$(grep -A1 '^name = "soroban-sdk"$' Cargo.lock | awk -F'"' '/version/{print $2}')"
PASSPHRASE="$(stellar network ls --long 2>/dev/null | awk -v n="$NETWORK" '/^Name: /{found=($2==n)} found && /^Network passphrase: /{sub(/^Network passphrase: /,"");print;exit}')"

echo ">> writing $MANIFEST"
mkdir -p "$(dirname "$MANIFEST")"
python3 - "$MANIFEST" "$CRATE" "$CONTRACT_ID" "$WASM_HASH" "$DEPLOY_TX" "$DEPLOYER" "$DEPLOYED_AT" "$RUSTC" "$CLI" "$SDK" "$EXPLORER" "$NETWORK" "$PASSPHRASE" "${CTOR_ARGS[@]}" <<'PY'
import json, os, sys
(path, crate, cid, wasm_hash, tx, deployer, at, rustc, cli, sdk, explorer, network, passphrase, *ctor) = sys.argv[1:]
m = json.load(open(path)) if os.path.exists(path) else {}
m.setdefault("network", network)
if passphrase:
    m.setdefault("passphrase", passphrase)
m.setdefault("contracts", {})
m["contracts"][crate] = {
    "contract_id": cid,
    "wasm_hash": wasm_hash,
    "deploy_tx": tx,
    "deployer": deployer,
    "deployed_at": at,
    "constructor_args": ctor,
    "toolchain": {"rustc": rustc, "soroban_sdk": sdk, "stellar_cli": cli},
    "explorer": {"contract": f"{explorer}/contract/{cid}", "tx": f"{explorer}/tx/{tx}"},
}
json.dump(m, open(path, "w"), indent=2)
open(path, "a").write("\n")
PY

echo
echo "$CRATE deployed on $NETWORK"
echo "  contract: $EXPLORER/contract/$CONTRACT_ID"
echo "  tx:       $EXPLORER/tx/$DEPLOY_TX"
