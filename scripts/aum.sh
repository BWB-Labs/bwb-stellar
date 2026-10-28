#!/usr/bin/env bash
# Tokenized AUM, read from the sale contracts: the grant's Tranche 1 query,
# reproducible by anyone with the Stellar CLI, jq and python3.
#
#   scripts/aum.sh [--json <file>] [offering-id ...]
#
# With no offering ids, every offering in deployments/testnet.json whose
# sale is "deployed". For each one the sale's `state`, `config` and
# `accounting` views are simulated (never sent, no fee, no key material
# beyond the bwb-testnet-deployer identity the simulation is built for).
#
# Definition: AUM counts the offerings whose state is not Failed. Per
# offering, units = sold_tokens and value = sold_tokens × config.price, in
# base units of the payment asset (demo USDC, 7 decimals: price 100000000
# is 10 USDC per token). The contract's `raised` is cross-checked against
# sold_tokens × price; a mismatch shows in the check column and in the
# JSON. Only counted offerings go into the totals.
#
# Prints a markdown table and the totals. --json also writes the same
# figures as a document, deterministic apart from the ledger and the
# timestamp. An offering whose sale lacks a view (older code) is shown with
# its error, not counted, and the exit status is 1.
#
# NETWORK defaults to testnet, the only network supported for now.
set -euo pipefail

NETWORK="${NETWORK:-testnet}"
case "$NETWORK" in
  testnet) ;;
  *) echo "only testnet is supported for now (NETWORK=$NETWORK)" >&2; exit 1 ;;
esac
SOURCE=bwb-testnet-deployer
PAYMENT_DECIMALS=7

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/deployments/$NETWORK.json"

usage() { echo "usage: scripts/aum.sh [--json <file>] [offering-id ...]" >&2; exit 1; }

JSON_OUT=""
IDS=()
while (($#)); do
  case "$1" in
    --json) [[ -n "${2:-}" ]] || usage; JSON_OUT="$2"; shift 2 ;;
    --json=*) JSON_OUT="${1#--json=}"; shift ;;
    -h|--help) usage ;;
    -*) echo "unknown option: $1" >&2; usage ;;
    *) IDS+=("$1"); shift ;;
  esac
done

# Without ids: every offering with a deployed sale, in id order.
if ((${#IDS[@]} == 0)); then
  while IFS= read -r id; do IDS+=("$id"); done \
    < <(jq -r '.offerings | to_entries[] | select(.value.sale.status == "deployed") | .key' "$MANIFEST" | sort)
  ((${#IDS[@]})) || { echo "no deployed sale in $MANIFEST" >&2; exit 1; }
fi

# read_view <contract> <fn>: simulates a view and prints its value (the
# CLI's last line). On failure prints the CLI's last line to stderr and
# returns 1, so the caller can record the error instead of stopping.
read_view() {
  local id="$1" fn="$2" out
  if out="$(stellar contract invoke --network "$NETWORK" --source "$SOURCE" --id "$id" --send=no -- "$fn" 2>&1)"; then
    printf '%s\n' "$out" | tail -n1
  else
    printf '%s\n' "$out" | tail -n1 >&2
    return 1
  fi
}

# offering_row <offering-id>: one JSON object with the manifest's sale entry
# and the three views, or an error string when a view can't be read.
offering_row() {
  local o="$1" sale wasm state config accounting err=""
  sale="$(jq -r --arg o "$o" '.offerings[$o].sale.contract_id // empty' "$MANIFEST")"
  wasm="$(jq -r --arg o "$o" '.offerings[$o].sale.wasm_hash // empty' "$MANIFEST")"
  if [[ -z "$sale" || -z "$wasm" ]]; then
    jq -n --arg id "$o" '{id: $id, error: "no deployed sale in the manifest"}'
    return
  fi
  state="$(read_view "$sale" state 2>/dev/null)" || err="state: not readable"
  config="$(read_view "$sale" config 2>/dev/null)" || err="${err:-config: not readable}"
  accounting="$(read_view "$sale" accounting 2>/dev/null)" || err="${err:-accounting: no such view on this code}"
  jq -n --arg id "$o" --arg sale "$sale" --arg wasm "$wasm" --arg err "$err" \
    --argjson state "${state:-null}" --argjson config "${config:-null}" --argjson accounting "${accounting:-null}" \
    '{id: $id, sale: $sale, wasm: $wasm, state: $state, config: $config, accounting: $accounting,
      error: (if $err == "" then null else $err end)}'
}

LEDGER="$(stellar ledger latest --network "$NETWORK" --output json)"
QUERIED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PASSPHRASE="$(jq -r '.passphrase' "$MANIFEST")"

ROWS="$(for o in "${IDS[@]}"; do offering_row "$o"; done | jq -s '.')"

# The arithmetic is exact in python (i128 amounts), which also prints the
# table and writes the document.
python3 - "$NETWORK" "$PASSPHRASE" "$LEDGER" "$QUERIED_AT" "$PAYMENT_DECIMALS" "$JSON_OUT" "$ROWS" <<'PY'
import json, os, sys

network, passphrase, ledger_json, queried_at, decimals, json_out, rows_json = sys.argv[1:8]
decimals = int(decimals)
ledger = json.loads(ledger_json)
rows = json.loads(rows_json)

DEFINITION = (
    "Tokenized AUM is the sum, over the sale contracts whose state is not Failed, "
    "of sold_tokens (units) and of sold_tokens x config.price (value, in base units "
    f"of the payment asset, {decimals} decimals); raised is cross-checked against it."
)

def human(v):
    sign = "-" if v < 0 else ""
    v = abs(v)
    return f"{sign}{v // 10**decimals}.{v % 10**decimals:0{decimals}d}"

offerings, total_units, total_value, counted_n, failed_reads = [], 0, 0, 0, 0
for r in rows:
    row = {"id": r["id"], "sale_contract_id": r.get("sale"), "wasm_hash": r.get("wasm"),
           "state": r.get("state"), "counted": False}
    if r.get("error"):
        failed_reads += 1
        row.update({"error": r["error"]})
        offerings.append(row)
        continue
    acc, cfg = r["accounting"], r["config"]
    supply, sold, released = int(acc["supply"]), int(acc["sold_tokens"]), int(acc["released_tokens"])
    raised, withdrawn, price = int(acc["raised"]), int(acc["withdrawn"]), int(cfg["price"])
    value = sold * price
    counted = r["state"] != "Failed"
    matches = raised == value
    if counted:
        counted_n += 1
        total_units += sold
        total_value += value
    row.update({
        "counted": counted,
        "supply": str(supply), "sold_tokens": str(sold), "released_tokens": str(released),
        "price": str(price), "raised": str(raised), "withdrawn": str(withdrawn),
        "value": str(value), "value_human": human(value),
        "raised_matches": matches,
        "warning": None if matches else f"raised {raised} != sold_tokens x price {value}",
        "config": {"price": str(price), "cooling_off_secs": cfg["cooling_off_secs"],
                   "min_success_percent": cfg["min_success_percent"]},
    })
    offerings.append(row)

# The table. Amounts in base units; value also with the asset's decimals.
cols = ["offering", "state", "counted", "supply", "sold (units)", "released", "price",
        "value", f"value ({decimals} dp)", "check", "sale contract", "wasm hash"]
print("| " + " | ".join(cols) + " |")
print("|" + "---|" * len(cols))
for row in offerings:
    if "error" in row:
        cells = [row["id"], row["state"] or "?", "no", "", "", "", "", "", "",
                 f"not readable: {row['error']}",
                 f"`{row['sale_contract_id']}`" if row["sale_contract_id"] else "",
                 f"`{row['wasm_hash']}`" if row["wasm_hash"] else ""]
    else:
        cells = [row["id"], row["state"], "yes" if row["counted"] else "no", row["supply"],
                 row["sold_tokens"], row["released_tokens"], row["price"], row["value"],
                 row["value_human"], "ok" if row["raised_matches"] else "WARNING: " + row["warning"],
                 f"`{row['sale_contract_id']}`", f"`{row['wasm_hash']}`"]
    print("| " + " | ".join(cells) + " |")
print()
print(f"Total ({counted_n} of {len(offerings)} offerings counted): "
      f"{total_units} units, value {total_value} base units = {human(total_value)}")
print(f"Ledger {ledger['sequence']} on {network}, queried {queried_at}.")

if json_out:
    doc = {
        "network": network,
        "passphrase": passphrase,
        "ledger": {"sequence": ledger["sequence"], "id": ledger.get("id"),
                   "protocol_version": ledger.get("protocolVersion")},
        "queried_at": queried_at,
        "definition": DEFINITION,
        "payment_decimals": decimals,
        "offerings": offerings,
        "totals": {"counted_offerings": counted_n, "offerings": len(offerings),
                   "units": str(total_units), "value": str(total_value), "value_human": human(total_value)},
    }
    os.makedirs(os.path.dirname(os.path.abspath(json_out)), exist_ok=True)
    with open(json_out, "w") as f:
        json.dump(doc, f, indent=2)
        f.write("\n")
    print(f"Written {json_out}.")

sys.exit(1 if failed_reads else 0)
PY
