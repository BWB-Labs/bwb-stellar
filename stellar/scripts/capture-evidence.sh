#!/usr/bin/env bash
# Snapshot the demonstration's on-chain evidence so it survives a testnet
# reset and the RPC's short history (about seven days).
#
#   scripts/capture-evidence.sh [--out <dir>]
#
# Reads the README's "### L4 demonstration" table and, for every
# transaction link in every row, fetches the result and the events from
# the RPC (`stellar tx fetch result|events`) and the record from Horizon.
# One file per transaction, <out>/step-NN.json, or step-NN-k.json when a
# row holds several links; <out>/index.json lists them. The deploy
# transactions of the allowlist and of the demonstration's offerings go to
# <out>/../deploys/<name>.json in the same shape.
#
# Each file: {step, what, hash, explorer_url, horizon: {successful, ledger,
# created_at, source_account, fee_charged}, contract_error, result, events}.
# `events` holds contract_events (the events the ledger kept) and
# transaction_events; for a rejected step, whose contract events were
# rolled back, those come as rolled_back_events. The host's diagnostic
# trail (fn_call/fn_return, metrics) is not kept. A rejected step has no
# contract_events; its code is read from the host_fn_failed diagnostic
# into contract_error, null for a successful step. A hash the RPC no
# longer has gets fetch_errors instead of result/events.
#
# Default out dir: docs/scf/evidence/demo. Testnet only, read-only.
set -euo pipefail

NETWORK=testnet
EXPLORER="https://stellar.expert/explorer/testnet"
HORIZON="https://horizon-testnet.stellar.org"
# The demonstration's offerings, as in scripts/demo-offer-sale.sh.
DEMO_OFFERINGS="t1-demo-4 t1-demo-5"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
README="$ROOT/README.md"
MANIFEST="$ROOT/deployments/$NETWORK.json"
OUT="$ROOT/docs/scf/evidence/demo"

usage() { echo "usage: scripts/capture-evidence.sh [--out <dir>]" >&2; exit 1; }
while (($#)); do
  case "$1" in
    --out) [[ -n "${2:-}" ]] || usage; OUT="$2"; shift 2 ;;
    --out=*) OUT="${1#--out=}"; shift ;;
    *) usage ;;
  esac
done
DEPLOYS="$(dirname "$OUT")/deploys"
mkdir -p "$OUT" "$DEPLOYS"

ERR="$(mktemp)"
trap 'rm -f "$ERR"' EXIT
MISSING=()

# fetch_json <cmd...>: runs a CLI command that prints JSON; prints it, or
# prints nothing and returns 1 with the CLI's message in $ERR.
fetch_json() {
  local out
  if out="$("$@" 2>"$ERR")" && jq -e . >/dev/null 2>&1 <<<"$out"; then
    printf '%s\n' "$out"
  else
    return 1
  fi
}

# capture_tx <step> <what> <hash> <file>: one transaction to one file.
# <step> is the row number, or a name for a deploy.
capture_tx() {
  local step="$1" what="$2" hash="$3" file="$4"
  local result events horizon errors='[]'
  if ! result="$(fetch_json stellar tx fetch result --hash "$hash" --network "$NETWORK" --output json)"; then
    result=null
    errors="$(jq -c --arg m "result: $(tail -n1 "$ERR")" '. + [$m]' <<<"$errors")"
  fi
  if ! events="$(fetch_json stellar tx fetch events --hash "$hash" --network "$NETWORK" --output json)"; then
    events=null
    errors="$(jq -c --arg m "events: $(tail -n1 "$ERR")" '. + [$m]' <<<"$errors")"
  fi
  if ! horizon="$(curl -sfS --retry 2 --max-time 30 "$HORIZON/transactions/$hash" 2>"$ERR")"; then
    horizon=null
    errors="$(jq -c --arg m "horizon: $(tail -n1 "$ERR")" '. + [$m]' <<<"$errors")"
  fi
  if [[ "$errors" != '[]' ]]; then
    MISSING+=("$hash ($step): $errors")
  fi
  jq -n --arg step "$step" --arg what "$what" --arg hash "$hash" --arg url "$EXPLORER/tx/$hash" \
    --argjson horizon "$horizon" --argjson result "$result" --argjson events "$events" --argjson errors "$errors" '
    {
      step: ($step | if test("^[0-9]+$") then tonumber else . end),
      what: $what,
      hash: $hash,
      explorer_url: $url,
      horizon: ($horizon | if . == null then null
                else {successful, ledger, created_at, source_account, fee_charged} end),
      # The contract error of a rejected call, from the host_fn_failed
      # diagnostic; null when the call went through.
      contract_error: ([$events.diagnostic_events[]?.event.body.v0.topics
                        | select(.[0].symbol? == "host_fn_failed") | .[1].error
                        | if type == "object" and has("contract") then .contract else . end]
                       | first // null)
    }
    + (if $errors == [] then {} else {fetch_errors: $errors} end)
    + {result: $result}
    # Keep what a reader needs: the events the ledger kept and the
    # transaction-level ones. The fn_call/fn_return trail of the host and its
    # metrics are dropped; for a rejected call, whose contract events were
    # rolled back, those events are kept from the trail as rolled_back_events.
    + {events: ($events | if . == null then null else {
        contract_events: ([.contract_events[]?] | flatten),
        transaction_events: ([.transaction_events[]?] | flatten),
        rolled_back_events: ([.diagnostic_events[]? | select(.in_successful_contract_call == false and .event.type == "contract") | .event])
      } | if .rolled_back_events == [] then del(.rolled_back_events) else . end end)}' > "$file"
  printf '%s  %s\n' "$file" "$hash"
}

# demo_rows: the README table as "step<TAB>what<TAB>hash hash ...", one
# line per row. The table runs from "### L4 demonstration" to the next
# level-2 heading; a row is "| <n> | <what> | <links> |".
demo_rows() {
  awk '/^### L4 demonstration/ {on=1; next} on && /^## / {exit} on && /^\| [0-9]+ \|/' "$README" \
  | while IFS= read -r line; do
      local step what hashes
      step="$(cut -d'|' -f2 <<<"$line" | xargs)"
      what="$(cut -d'|' -f3 <<<"$line" | sed 's/^ *//; s/ *$//')"
      hashes="$(grep -oE 'tx/[0-9a-f]{64}' <<<"$line" | cut -d/ -f2 | tr '\n' ' ')"
      printf '%s\t%s\t%s\n' "$step" "$what" "${hashes% }"
    done
}

echo "demonstration steps from README.md -> $OUT"
ROWS=0 FILES=0
while IFS=$'\t' read -r step what hashes; do
  ROWS=$((ROWS + 1))
  read -ra list <<<"$hashes"
  ((${#list[@]})) || { echo "step $step has no transaction link" >&2; continue; }
  k=0
  for hash in "${list[@]}"; do
    k=$((k + 1))
    if ((${#list[@]} == 1)); then
      file="$(printf '%s/step-%02d.json' "$OUT" "$step")"
    else
      file="$(printf '%s/step-%02d-%d.json' "$OUT" "$step" "$k")"
    fi
    capture_tx "$step" "$what" "$hash" "$file"
    FILES=$((FILES + 1))
  done
done < <(demo_rows)
((ROWS)) || { echo "no demonstration table found in $README" >&2; exit 1; }

# The index: one line per file, in file order.
jq -n --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg network "$NETWORK" '
  {
    source: "README.md, section \"L4 demonstration\"",
    network: $network,
    captured_at: $at,
    transactions: [inputs | {
      file: (input_filename | sub(".*/"; "")), step, what, hash,
      successful: (if .horizon != null then .horizon.successful
                   elif .result != null then (.result.result | has("tx_success")) else null end),
      ledger: .horizon.ledger, created_at: .horizon.created_at, contract_error
    }]
  }' "$OUT"/step-*.json > "$OUT/index.json"
echo "$ROWS rows, $FILES transaction files, index $OUT/index.json"

echo "deploy transactions -> $DEPLOYS"
capture_tx offer-allowlist "Deploy: offer-allowlist $(jq -r '.contracts["offer-allowlist"].contract_id' "$MANIFEST")" \
  "$(jq -r '.contracts["offer-allowlist"].deploy_tx' "$MANIFEST")" "$DEPLOYS/offer-allowlist.json"
for o in $DEMO_OFFERINGS; do
  for role in token sale; do
    capture_tx "$o-$role" "Deploy: $o $role $(jq -r --arg o "$o" --arg r "$role" '.offerings[$o][$r].contract_id' "$MANIFEST")" \
      "$(jq -r --arg o "$o" --arg r "$role" '.offerings[$o][$r].deploy_tx' "$MANIFEST")" "$DEPLOYS/$o-$role.json"
    # An in-place upgrade, when the manifest records one.
    up="$(jq -r --arg o "$o" --arg r "$role" '.offerings[$o][$r].upgrade_tx // empty' "$MANIFEST")"
    [[ -n "$up" ]] && capture_tx "$o-$role-upgrade" "Upgrade: $o $role to $(jq -r --arg o "$o" --arg r "$role" '.offerings[$o][$r].wasm_hash' "$MANIFEST")" \
      "$up" "$DEPLOYS/$o-$role-upgrade.json"
  done
done

if ((${#MISSING[@]})); then
  echo "could not fetch completely:" >&2
  printf '  %s\n' "${MISSING[@]}" >&2
  exit 1
fi
