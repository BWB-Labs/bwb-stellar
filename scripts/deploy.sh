#!/usr/bin/env bash
# Build, upload and deploy one contract crate, then record it in
# deployments/<network>.json.
#
#   scripts/deploy.sh <crate> [-- <constructor args...>]
#
# Environment:
#   NETWORK   stellar network name from `stellar network ls` (default: testnet)
#   SOURCE    stellar keystore identity that signs and pays fees.
#             Defaults to bwb-testnet-deployer on testnet only; every other
#             network requires it explicitly.
#   OFFERING  offering identifier, required for the per-offering crates
#             (offer-token, offer-sale). The contract is deployed at the
#             address derived from SOURCE and a salt built from it (see
#             scripts/lib/offering.sh), and recorded under
#             offerings.<id> in the manifest together with its sibling's
#             precomputed address. Global crates (offer-allowlist) take no
#             OFFERING and are recorded under contracts.
#
# Example, an offering's token minting to its sale's future address:
#   SALE=$(scripts/offering-addresses.sh aurora-01 | jq -r .sale.contract_id)
#   OFFERING=aurora-01 scripts/deploy.sh offer-token -- --admin G... \
#     --initial_holder "$SALE" ...
#
# Roles (admin, operator, ...) are constructor arguments, never the deployer.
set -euo pipefail

CRATE="${1:?usage: scripts/deploy.sh <crate> [-- <constructor args...>]}"
shift
if [[ "${1:-}" == "--" ]]; then shift; fi
CTOR_ARGS=("$@")

NETWORK="${NETWORK:-testnet}"
case "$NETWORK" in
  testnet)
    EXPLORER="https://stellar.expert/explorer/testnet"
    HORIZON="https://horizon-testnet.stellar.org"
    ;;
  mainnet|public)
    EXPLORER="https://stellar.expert/explorer/public"
    HORIZON="https://horizon.stellar.org"
    ;;
  *) echo "unknown network: $NETWORK" >&2; exit 1 ;;
esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/offering.sh
source "$ROOT/scripts/lib/offering.sh"
resolve_source "$NETWORK"

OFFERING="${OFFERING:-}"
ROLE=""
SALT=""
SALT_ARGS=()
ALIAS="$CRATE"
if ROLE="$(crate_role "$CRATE")"; then
  : "${OFFERING:?OFFERING is required for $CRATE: one instance per offering}"
  SALT="$(offering_salt "$OFFERING" "$ROLE")"
  SALT_ARGS=(--salt "$SALT")
  ALIAS="$CRATE-$OFFERING"
elif [[ -n "$OFFERING" ]]; then
  echo "$CRATE is not a per-offering contract; unset OFFERING" >&2
  exit 1
fi

MANIFEST="$ROOT/deployments/$NETWORK.json"
WASM="$ROOT/target/wasm32v1-none/release/${CRATE//-/_}.wasm"
ERR="$(mktemp)"
trap 'rm -f "$ERR"' EXIT

# Run a CLI command, keeping stdout; on failure, show what the CLI said.
cli() {
  local out
  if ! out="$("$@" 2>"$ERR")"; then
    echo "command failed: $*" >&2
    cat "$ERR" >&2
    exit 1
  fi
  printf '%s\n' "$out"
}

cd "$ROOT"

if [[ -n "$SALT" ]]; then
  SIBLING_ROLE="$([[ "$ROLE" == token ]] && echo sale || echo token)"
  SIBLING_SALT="$(offering_salt "$OFFERING" "$SIBLING_ROLE")"
  EXPECTED_ID="$(cli offering_address "$NETWORK" "$SOURCE" "$SALT" | tail -n1)"
  SIBLING_ID="$(cli offering_address "$NETWORK" "$SOURCE" "$SIBLING_SALT" | tail -n1)"
  echo ">> offering $OFFERING: $ROLE at $EXPECTED_ID, $SIBLING_ROLE at $SIBLING_ID"
  # Re-runnable: a step that already succeeded is not repeated, since
  # deploying to a taken address fails. A contract that exists but is not in
  # the manifest (the previous run died after deploying) is an error, not a
  # success: its deploy transaction has to be recorded by hand.
  if stellar contract info interface --network "$NETWORK" --contract-id "$EXPECTED_ID" >/dev/null 2>&1; then
    RECORDED="$(python3 - "$MANIFEST" "$OFFERING" "$ROLE" "$EXPECTED_ID" <<'PY'
import json, os, sys
path, offering, role, cid = sys.argv[1:]
m = json.load(open(path)) if os.path.exists(path) else {}
e = m.get("offerings", {}).get(offering, {}).get(role, {})
print("yes" if e.get("status") == "deployed" and e.get("contract_id") == cid else "no")
PY
)"
    if [[ "$RECORDED" == yes ]]; then
      echo "$CRATE for offering $OFFERING is already deployed at $EXPECTED_ID and recorded; nothing to do"
      exit 0
    fi
    echo "$CRATE for offering $OFFERING exists at $EXPECTED_ID but is not recorded in $MANIFEST;" \
      "find its deploy transaction and record it by hand" >&2
    exit 1
  fi
fi

STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo ">> building $CRATE"
cli stellar contract build --package "$CRATE" >/dev/null
[[ -f "$WASM" ]] || { echo "wasm not found: $WASM" >&2; exit 1; }

echo ">> uploading wasm"
WASM_HASH="$(cli stellar contract upload --wasm "$WASM" --network "$NETWORK" --source "$SOURCE" | tail -n1)"
echo "   wasm hash: $WASM_HASH"

echo ">> deploying"
# ${CTOR_ARGS[@]+...} expands to nothing when the array is empty; a plain
# "${CTOR_ARGS[@]}" is an unbound-variable error under set -u on bash < 4.4
# (macOS ships 3.2).
CONTRACT_ID="$(cli stellar contract deploy --wasm-hash "$WASM_HASH" --network "$NETWORK" --source "$SOURCE" --alias "$ALIAS" ${SALT_ARGS[@]+"${SALT_ARGS[@]}"} -- ${CTOR_ARGS[@]+"${CTOR_ARGS[@]}"} | tail -n1)"
echo "   contract id: $CONTRACT_ID"
if [[ -n "$SALT" && "$CONTRACT_ID" != "$EXPECTED_ID" ]]; then
  echo "deployed at $CONTRACT_ID but $EXPECTED_ID was precomputed; the manifest is not written" >&2
  exit 1
fi

DEPLOYER="$(stellar keys address "$SOURCE")"

# Find the deploy transaction. The CLI does not print the hash, and "most
# recent transaction" could be the WASM upload if Horizon lags the RPC. Match
# the newest successful create-contract host function from the deployer that
# was created after this script started. Horizon reports the deployer and the
# salt on that operation, not the resulting contract ID.
echo ">> locating deploy transaction"
DEPLOY_TX=""
for _ in $(seq 1 15); do
  DEPLOY_TX="$(curl -sf "$HORIZON/accounts/$DEPLOYER/operations?order=desc&limit=5" \
    | python3 -c '
import sys, json
deployer, started = sys.argv[1], sys.argv[2]
for op in json.load(sys.stdin)["_embedded"]["records"]:
    if op.get("type") != "invoke_host_function":
        continue
    if "CreateContract" not in op.get("function", ""):
        continue
    if not op.get("transaction_successful") or op.get("source_account") != deployer:
        continue
    if op.get("created_at", "") < started:
        continue
    print(op["transaction_hash"]); break
' "$DEPLOYER" "$STARTED_AT")"
  [[ -n "$DEPLOY_TX" ]] && break
  sleep 1
done
[[ -n "$DEPLOY_TX" ]] || { echo "could not find the deploy transaction for $CONTRACT_ID on Horizon" >&2; exit 1; }
echo "   tx: $DEPLOY_TX"

DEPLOYED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUSTC="$(rustc --version | awk '{print $2}')"
CLI="$(stellar --version | head -n1 | awk '{print $2}')"
SDK="$(grep -A1 '^name = "soroban-sdk"$' Cargo.lock | awk -F'"' '/version/{print $2}')"
PASSPHRASE="$(stellar network ls --long 2>/dev/null | awk -v n="$NETWORK" '/^Name: /{found=($2==n)} found && /^Network passphrase: /{sub(/^Network passphrase: /,"");print;exit}')"

echo ">> writing $MANIFEST"
mkdir -p "$(dirname "$MANIFEST")"
M_OFFERING="$OFFERING" M_ROLE="$ROLE" M_SALT="$SALT" \
M_SIBLING_ROLE="${SIBLING_ROLE:-}" M_SIBLING_SALT="${SIBLING_SALT:-}" M_SIBLING_ID="${SIBLING_ID:-}" \
python3 - "$MANIFEST" "$CRATE" "$CONTRACT_ID" "$WASM_HASH" "$DEPLOY_TX" "$DEPLOYER" "$DEPLOYED_AT" "$RUSTC" "$CLI" "$SDK" "$EXPLORER" "$NETWORK" "$PASSPHRASE" ${CTOR_ARGS[@]+"${CTOR_ARGS[@]}"} <<'PY'
import json, os, sys
(path, crate, cid, wasm_hash, tx, deployer, at, rustc, cli, sdk, explorer, network, passphrase, *ctor) = sys.argv[1:]
env = os.environ
m = json.load(open(path)) if os.path.exists(path) else {}
m.setdefault("network", network)
if passphrase:
    m.setdefault("passphrase", passphrase)
record = {
    "crate": crate,
    "contract_id": cid,
    "wasm_hash": wasm_hash,
    "deploy_tx": tx,
    "deployer": deployer,
    "deployed_at": at,
    "constructor_args": ctor,
    "toolchain": {"rustc": rustc, "soroban_sdk": sdk, "stellar_cli": cli},
    "explorer": {"contract": f"{explorer}/contract/{cid}", "tx": f"{explorer}/tx/{tx}"},
}
if env["M_OFFERING"]:
    # Per-offering contracts: offerings.<id>.<role>. The sibling's address is
    # recorded as precomputed until its own deploy overwrites the entry.
    offering = m.setdefault("offerings", {}).setdefault(env["M_OFFERING"], {})
    offering[env["M_ROLE"]] = {"status": "deployed", "salt": env["M_SALT"], **record}
    sibling = offering.get(env["M_SIBLING_ROLE"])
    if not sibling or sibling.get("status") != "deployed":
        offering[env["M_SIBLING_ROLE"]] = {
            "status": "precomputed",
            "salt": env["M_SIBLING_SALT"],
            "contract_id": env["M_SIBLING_ID"],
            "deployer": deployer,
        }
else:
    m.setdefault("contracts", {})[crate] = record
json.dump(m, open(path, "w"), indent=2)
open(path, "a").write("\n")
PY

echo
echo "$CRATE deployed on $NETWORK"
echo "  contract: $EXPLORER/contract/$CONTRACT_ID"
echo "  tx:       $EXPLORER/tx/$DEPLOY_TX"
