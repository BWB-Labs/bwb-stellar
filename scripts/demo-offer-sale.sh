#!/usr/bin/env bash
# Exercise two deployed sale contracts on testnet and leave a public trail:
# each step is a transaction, successful or rejected, with its explorer link.
# This is the grant's demonstration (D2) and doubles as an integration test
# of the sale beside the real allowlist, token and a payment asset.
#
#   STAGE=1 scripts/demo-offer-sale.sh    # today
#   STAGE=2 scripts/demo-offer-sale.sh    # after the cooling-off window
#
# DEMOS selects the offerings of stage 1 ("1 2" by default): DEMOS=2 reruns
# the failure path alone. Wiring is idempotent, so a rerun skips a grant
# or an activation already done.
#
# Two offerings from deployments/testnet.json. `l4-demo-1` walks the success
# path: activation, an eligibility rejection, an allocation rejection, a
# purchase, pause and unpause (with a rejected purchase in between), a
# cooling-off refund, more purchases, close, reopen, close, a too-early
# finalize; then, in stage 2, finalize with the burn, release, the payout
# and a refused late cooling-off. `l4-demo-2` walks the failure path in
# stage 1: a purchase, a cancel refused to a stranger, the owner's cancel,
# the refund and the token withdrawal.
#
# Identities (stellar keys): bwb-testnet-deployer (the platform hot key:
# allocator, and the anyone who activates, finalizes, releases and
# refunds), bwb-testnet-treasury (owner), bwb-testnet-platform (pauser),
# bwb-testnet-investor-a and -b (funded with the demo USDC, trustlines
# open). Testnet only: every command passes --network testnet.
set -euo pipefail

STAGE="${STAGE:-1}"
DEMOS="${DEMOS:-1 2}"
case "$STAGE" in 1|2) ;; *) echo "STAGE must be 1 or 2" >&2; exit 1 ;; esac
NETWORK=testnet
EXPLORER="https://stellar.expert/explorer/testnet"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/deployments/testnet.json"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

ALLOWLIST="$(jq -r '.contracts["offer-allowlist"].contract_id' "$MANIFEST")"
ALLOWLIST_WASM="$(jq -r '.contracts["offer-allowlist"].wasm_hash' "$MANIFEST")"
USDC="$(jq -r '.contracts["demo-usdc"].contract_id' "$MANIFEST")"

DEPLOYER="$(stellar keys address bwb-testnet-deployer)"
TREASURY="$(stellar keys address bwb-testnet-treasury)"
PLATFORM="$(stellar keys address bwb-testnet-platform)"
INV_A="$(stellar keys address bwb-testnet-investor-a)"
INV_B="$(stellar keys address bwb-testnet-investor-b)"

PRICE=100000000   # 10 USDC per token, in stroops
SALE_ROLE='"sale"'

step() { printf '\n### %s\n' "$*"; }

# ok <source> <contract> <fn> [args...]: a call that must succeed. Prints
# its hash, link and events.
ok() {
  local source="$1" id="$2"; shift 2
  local out
  out="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$id" -- "$@" 2>&1)" \
    || { echo "$out" >&2; exit 1; }
  echo "tx: $(grep -o "$EXPLORER/tx/[0-9a-f]*" <<<"$out" | head -n1)"
  grep -o 'Event: .*' <<<"$out" || true
}

# read <contract> <fn> [args...]: a simulated read, printed.
# A failed read stops the script with the CLI's error instead of printing
# an empty value. The value is the CLI's last line of output.
read_fn() {
  local id="$1"; shift
  local out
  out="$(stellar contract invoke --network "$NETWORK" --source bwb-testnet-deployer --id "$id" --send=no -- "$@" 2>&1)" \
    || { echo "read failed: $*" >&2; echo "$out" >&2; exit 1; }
  printf '%s\n' "$out" | tail -n1
}

# twin <file> <source> <contract> <fn> [args...]: simulates a passing call
# and keeps its envelope, whose footprint a rejection reuses.
twin() {
  local file="$1" source="$2" id="$3"; shift 3
  stellar contract invoke --network "$NETWORK" --source "$source" --id "$id" --build-only -- "$@" \
    | stellar tx simulate --network "$NETWORK" --source "$source" > "$file"
}

# rejected <source> <contract> <wasm-hash> <expected-code> [sender options] -- <fn> [args...]:
# a call that must fail with the given contract error, recorded on chain.
# Sender options are passed to send_rejected.py (--template-file, --swap,
# --payment-sub, --key).
rejected() {
  local source="$1" id="$2" wasm="$3" expect="$4"; shift 4
  local opts=()
  while (($#)) && [[ "$1" != -- ]]; do opts+=("$1"); shift; done
  shift
  local hash
  hash="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$id" --build-only -- "$@" \
    | python3 "$ROOT/scripts/lib/send_rejected.py" --network "$NETWORK" --source "$source" \
      --wasm-hash "$wasm" --expect "$expect" ${opts[@]+"${opts[@]}"})"
  echo "tx: $EXPLORER/tx/$hash"
  echo "rejected on chain with Error(Contract, #$expect)"
}

offering() {
  local o="$1"
  TOKEN="$(jq -r --arg o "$o" '.offerings[$o].token.contract_id' "$MANIFEST")"
  SALE="$(jq -r --arg o "$o" '.offerings[$o].sale.contract_id' "$MANIFEST")"
  SALE_WASM="$(jq -r --arg o "$o" '.offerings[$o].sale.wasm_hash' "$MANIFEST")"
  [[ "$SALE" == C* && "$SALE_WASM" != null ]] || { echo "no deployed sale for offering $o" >&2; exit 1; }
  echo "offering $o: token $TOKEN, sale $SALE ($EXPLORER/contract/$SALE)"
}

allocation() { printf '[{"investor":"%s","max":"%s","enabled":true}]' "$1" "$2"; }

balances() {
  printf 'sale state %s, totals %s\n' "$(read_fn "$SALE" state)" "$(read_fn "$SALE" totals)"
  for who in "investor-a:$INV_A" "investor-b:$INV_B" "treasury:$TREASURY" "sale:$SALE"; do
    printf '%s: %s USDC stroops, %s tokens\n' "${who%%:*}" \
      "$(read_fn "$USDC" balance --id "${who#*:}")" "$(read_fn "$TOKEN" balance --account "${who#*:}")"
  done
}

# wire <first step number>: the grant and the activation, two steps.
wire() {
  step "$1. The allocator grants the sale contract the allowlist's sale role"
  ok bwb-testnet-deployer "$ALLOWLIST" grant_role --account "$SALE" --role "$SALE_ROLE" --caller "$DEPLOYER"
  step "$(($1 + 1)). Anyone activates: the sale holds the whole supply and the role"
  if [[ "$(read_fn "$SALE" state)" == '"Preparing"' ]]; then
    ok bwb-testnet-deployer "$SALE" activate
  else
    echo "already activated"
  fi
}

if [[ "$STAGE" == 1 && " $DEMOS " == *" 1 "* ]]; then
  offering l4-demo-1
  wire 1

  step "3. The allocator allows investor A up to 3,000 USDC this month"
  ok bwb-testnet-deployer "$ALLOWLIST" set_allocations --entries "$(allocation "$INV_A" 30000000000)" --caller "$DEPLOYER"

  # A passing purchase by A, simulated and kept: the rejected purchases
  # below reuse its footprint.
  twin "$SCRATCH/buy-a.xdr" bwb-testnet-investor-a "$SALE" buy --investor "$INV_A" --amount $((10 * PRICE))

  step "4. Eligibility rejection: investor B, not on the allowlist, buys 10 tokens (6100 InvestorNotEnabled)"
  rejected bwb-testnet-investor-b "$SALE" "$SALE_WASM" 6100 \
    --template-file "$SCRATCH/buy-a.xdr" --swap "$INV_A:$INV_B" --payment-sub "$USDC" \
    -- buy --investor "$INV_B" --amount $((10 * PRICE))

  step "5. Allocation rejection: investor A buys 400 tokens, 4,000 USDC over a 3,000 cap (6101 AllocationExceeded)"
  rejected bwb-testnet-investor-a "$SALE" "$SALE_WASM" 6101 \
    --template-file "$SCRATCH/buy-a.xdr" --payment-sub "$USDC" \
    -- buy --investor "$INV_A" --amount $((400 * PRICE))

  step "6. Purchase: investor A buys 50 tokens for 500 USDC"
  ok bwb-testnet-investor-a "$SALE" buy --investor "$INV_A" --amount $((50 * PRICE))

  step "7. The platform pauses the sale"
  ok bwb-testnet-platform "$SALE" pause --caller "$PLATFORM"

  step "8. Purchase while paused: rejected (1000 EnforcedPause)"
  rejected bwb-testnet-investor-a "$SALE" "$SALE_WASM" 1000 \
    --template-file "$SCRATCH/buy-a.xdr" --payment-sub "$USDC" \
    -- buy --investor "$INV_A" --amount $((10 * PRICE))

  step "9. The platform tries to unpause: rejected, only the owner unpauses (6222 NotOwner)"
  rejected bwb-testnet-platform "$SALE" "$SALE_WASM" 6222 -- unpause --caller "$PLATFORM"

  step "10. The owner unpauses"
  ok bwb-testnet-treasury "$SALE" unpause --caller "$TREASURY"

  step "11. Cooling-off: investor A undoes the purchase and gets the 500 USDC back; the allowlist room returns"
  ok bwb-testnet-investor-a "$SALE" cooling_off_refund --investor "$INV_A"
  echo "A's room now: $(read_fn "$ALLOWLIST" remaining --investor "$INV_A")"

  step "12. Purchases: investor A buys 300 tokens; the allocator allows investor B, who buys 500"
  ok bwb-testnet-investor-a "$SALE" buy --investor "$INV_A" --amount $((300 * PRICE))
  ok bwb-testnet-deployer "$ALLOWLIST" set_allocations --entries "$(allocation "$INV_B" 50000000000)" --caller "$DEPLOYER"
  ok bwb-testnet-investor-b "$SALE" buy --investor "$INV_B" --amount $((500 * PRICE))

  step "13. The owner closes buying, reopens, and closes again"
  ok bwb-testnet-treasury "$SALE" close --caller "$TREASURY"
  ok bwb-testnet-treasury "$SALE" reopen --caller "$TREASURY"
  ok bwb-testnet-treasury "$SALE" close --caller "$TREASURY"

  step "14. Purchase after close: rejected (6201 NotActive)"
  rejected bwb-testnet-investor-a "$SALE" "$SALE_WASM" 6201 \
    --template-file "$SCRATCH/buy-a.xdr" --payment-sub "$USDC" \
    -- buy --investor "$INV_A" --amount $((10 * PRICE))

  step "15. Finalize before the windows run out: rejected (6214 CoolingOffOpen)"
  rejected bwb-testnet-deployer "$SALE" "$SALE_WASM" 6214 -- finalize

  step "l4-demo-1 after stage 1"
  balances
  echo "finalizable_at: $(read_fn "$SALE" finalizable_at) (unix seconds); run STAGE=2 after it"
fi

if [[ "$STAGE" == 1 && " $DEMOS " == *" 2 "* ]]; then
  offering l4-demo-2
  wire 16

  # The allowlist's cap is per investor per month across offerings, and A
  # used 3,000 USDC of room in l4-demo-1: the cap goes to 4,000 so that
  # 1,000 is left for this purchase.
  step "18. The allocator raises investor A's cap to 4,000 USDC; A buys 100 tokens"
  ok bwb-testnet-deployer "$ALLOWLIST" set_allocations --entries "$(allocation "$INV_A" 40000000000)" --caller "$DEPLOYER"
  ok bwb-testnet-investor-a "$SALE" buy --investor "$INV_A" --amount $((100 * PRICE))

  step "19. Investor B tries to cancel the offering: rejected (6222 NotOwner)"
  rejected bwb-testnet-investor-b "$SALE" "$SALE_WASM" 6222 -- cancel --caller "$INV_B"

  step "20. The owner cancels the offering"
  ok bwb-testnet-treasury "$SALE" cancel --caller "$TREASURY"

  step "21. Refund: anyone refunds investor A; the money and the allowlist room come back"
  ok bwb-testnet-deployer "$SALE" refund --investors "[\"$INV_A\"]"

  step "22. The owner withdraws the tokens of the failed offering"
  held="$(read_fn "$TOKEN" balance --account "$SALE" | tr -d '"')"
  ok bwb-testnet-treasury "$SALE" withdraw_tokens --to "$TREASURY" --amount "$held" --caller "$TREASURY"

  step "l4-demo-2 after stage 1"
  balances
fi

if [[ "$STAGE" == 2 ]]; then
  offering l4-demo-1
  now="$(date +%s)"
  at="$(read_fn "$SALE" finalizable_at | tr -d '"')"
  if (( now < at )); then
    echo "the last cooling-off window is still open: finalizable at $at, now $now" >&2
    exit 1
  fi

  step "23. Anyone finalizes: 800 of 1,000 sold, the 200 unsold tokens are burned"
  ok bwb-testnet-deployer "$SALE" finalize
  echo "token supply now: $(read_fn "$TOKEN" total_supply)"

  step "24. Release: anyone delivers the tokens to investors A and B"
  ok bwb-testnet-deployer "$SALE" release --investors "[\"$INV_A\",\"$INV_B\"]"

  step "25. Late cooling-off by investor A: rejected, the offering is decided (6225 NotOpen)"
  rejected bwb-testnet-investor-a "$SALE" "$SALE_WASM" 6225 -- cooling_off_refund --investor "$INV_A"

  step "26. The owner withdraws the 8,000 USDC raised, in two legs"
  ok bwb-testnet-treasury "$SALE" withdraw_payment \
    --payouts "[{\"recipient\":\"$TREASURY\",\"amount\":\"$((7000 * 10000000))\"},{\"recipient\":\"$INV_B\",\"amount\":\"$((1000 * 10000000))\"}]" \
    --caller "$TREASURY"

  step "l4-demo-1 after stage 2"
  balances
fi
