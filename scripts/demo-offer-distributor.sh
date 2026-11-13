#!/usr/bin/env bash
# Exercise a deployed distributor on testnet and leave a public trail: each
# step is a transaction, successful or rejected, with its explorer link.
# It doubles as an integration test of the distributor beside a real
# payment asset and classic accounts with trustlines.
#
#   scripts/demo-offer-distributor.sh
#
# The distributor of offering `t1-demo-4` from deployments/testnet.json.
# The run walks: a distribution to two investors, the read that says it was
# paid, the same id refused, a stranger refused, a correction under a new
# id, a pause with a refused distribution, a list with a recipient that has
# no trustline, which fails in simulation and pays nobody, and a payout to
# the asset's own contract, refused.
#
# A distribution id is paid once for the life of the contract, so every run
# derives fresh ids from RUN (the current time by default), the way the
# backend derives them from its own record: sha256("bwb:<offering>:<run>:<n>").
#
# Identities (stellar keys): bwb-testnet-treasury (owner, holds the demo
# USDC it pays with), bwb-testnet-platform (pauser; it has no USDC
# trustline, which the last step relies on), bwb-testnet-investor-a and -b
# (recipients, trustlines open), bwb-testnet-deployer (reads). Testnet
# only: every command passes --network testnet.
set -euo pipefail

NETWORK=testnet
OFFERING=t1-demo-4
RUN="${RUN:-$(date -u +%Y%m%dT%H%M%SZ)}"
EXPLORER="https://stellar.expert/explorer/testnet"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/deployments/testnet.json"

DISTRIBUTOR="$(jq -r --arg o "$OFFERING" '.offerings[$o].distributor.contract_id' "$MANIFEST")"
WASM_HASH="$(jq -r --arg o "$OFFERING" '.offerings[$o].distributor.wasm_hash' "$MANIFEST")"
USDC="$(jq -r '.contracts["demo-usdc"].contract_id' "$MANIFEST")"
[[ "$DISTRIBUTOR" == C* && "$WASM_HASH" != null ]] || {
  echo "no deployed distributor for offering $OFFERING" >&2
  exit 1
}

TREASURY="$(stellar keys address bwb-testnet-treasury)"
PLATFORM="$(stellar keys address bwb-testnet-platform)"
INV_A="$(stellar keys address bwb-testnet-investor-a)"
INV_B="$(stellar keys address bwb-testnet-investor-b)"

USDC_UNIT=10000000   # 1 USDC, in stroops

step() { printf '\n### %s\n' "$*"; }

# dist_id <n>: the n-th distribution id of this run, 64 hex characters.
dist_id() {
  python3 -c 'import hashlib, sys; print(hashlib.sha256(sys.argv[1].encode()).hexdigest())' \
    "bwb:$OFFERING:$RUN:$1"
}

# payouts <recipient> <usdc> [<recipient> <usdc> ...] -> the JSON list.
payouts() {
  local out="" sep=""
  while (($#)); do
    out+="$sep{\"recipient\":\"$1\",\"amount\":\"$(($2 * USDC_UNIT))\"}"
    sep=","
    shift 2
  done
  printf '[%s]' "$out"
}

# ok <source> <fn> [args...]: a call that must succeed. Prints its hash,
# link and events.
ok() {
  local source="$1"; shift
  local out
  out="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$DISTRIBUTOR" -- "$@" 2>&1)" \
    || { echo "$out" >&2; exit 1; }
  echo "tx: $(grep -o "$EXPLORER/tx/[0-9a-f]*" <<<"$out" | head -n1)"
  grep -o 'Event: .*' <<<"$out" || true
}

# read_fn <contract> <fn> [args...]: a simulated read. Sets READ to the
# value, the CLI's last line of output. It sets a variable instead of
# printing, because a failure inside $(...) would only end the subshell: a
# failed read stops the script with the CLI's error.
read_fn() {
  local id="$1"; shift
  local out
  out="$(stellar contract invoke --network "$NETWORK" --source bwb-testnet-deployer --id "$id" --send=no -- "$@" 2>&1)" \
    || { echo "read failed: $*" >&2; echo "$out" >&2; exit 1; }
  READ="$(printf '%s\n' "$out" | tail -n1)"
}

# expect_paid <id> <true|false>: reads is_paid and stops unless it is what
# the step claims.
expect_paid() {
  read_fn "$DISTRIBUTOR" is_paid --id "$1"
  echo "is_paid($1): $READ"
  [[ "$READ" == "$2" ]] || { echo "expected is_paid to be $2" >&2; exit 1; }
}

# balance_of <address>: sets READ to the USDC balance in stroops, unquoted.
balance_of() {
  read_fn "$USDC" balance --id "$1"
  READ="${READ//\"/}"
}

# rejected <source> <expected-code> <fn> [args...] [-- <storage keys>]:
# a call that must fail with the given contract error, recorded on chain.
rejected() {
  local source="$1" expect="$2"; shift 2
  local args=() keys=()
  while (($#)) && [[ "$1" != -- ]]; do args+=("$1"); shift; done
  if (($#)); then shift; for k in "$@"; do keys+=(--key "$k"); done; fi
  local hash
  hash="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$DISTRIBUTOR" --build-only -- "${args[@]}" \
    | python3 "$ROOT/scripts/lib/send_rejected.py" --network "$NETWORK" --source "$source" \
      --wasm-hash "$WASM_HASH" --expect "$expect" ${keys[@]+"${keys[@]}"})"
  echo "tx: $EXPLORER/tx/$hash"
  echo "rejected on chain with Error(Contract, #$expect)"
}

# The storage key of a paid id, which a call that reads it must declare.
paid_key() { printf '{"vec":[{"symbol":"Paid"},{"bytes":"%s"}]}' "$1"; }

balances() {
  for who in "treasury:$TREASURY" "investor-a:$INV_A" "investor-b:$INV_B" "distributor:$DISTRIBUTOR"; do
    balance_of "${who#*:}"
    printf '%s: %s USDC stroops\n' "${who%%:*}" "$READ"
  done
  # The last one read is the distributor's: it never holds money.
  [[ "$READ" == 0 ]] || { echo "the distributor holds $READ stroops; it should hold none" >&2; exit 1; }
}

# The run pauses a distributor other runs share. If it stops between the
# pause and the unpause, lift the pause on the way out.
PAUSED_BY_RUN=0
restore_pause() {
  if [[ "$PAUSED_BY_RUN" == 1 ]]; then
    echo "the run stopped while paused: unpausing" >&2
    stellar contract invoke --network "$NETWORK" --source bwb-testnet-treasury --id "$DISTRIBUTOR" -- \
      unpause --caller "$TREASURY" >/dev/null 2>&1 \
      || echo "could not unpause $DISTRIBUTOR; the treasury must do it by hand" >&2
  fi
}
trap restore_pause EXIT

FIRST="$(dist_id 1)"
CORRECTION="$(dist_id 2)"
PAUSED="$(dist_id 3)"
NO_TRUSTLINE="$(dist_id 4)"
TO_ASSET="$(dist_id 5)"

echo "offering $OFFERING: distributor $DISTRIBUTOR ($EXPLORER/contract/$DISTRIBUTOR), run $RUN"
read_fn "$DISTRIBUTOR" powers
echo "powers: $READ"
read_fn "$DISTRIBUTOR" paused
[[ "$READ" == false ]] || { echo "the distributor is paused; the treasury must unpause it before a run" >&2; exit 1; }
balances
balance_of "$INV_A"; A_BEFORE="$READ"
balance_of "$INV_B"; B_BEFORE="$READ"
FIRST_LIST="$(payouts "$INV_A" 100 "$INV_B" 50)"

step "1. The treasury distributes 150 USDC: 100 to investor A, 50 to investor B"
echo "id: $FIRST"
ok bwb-testnet-treasury distribute --id "$FIRST" --asset "$USDC" \
  --payouts "$FIRST_LIST" --caller "$TREASURY"

step "2. The reads: the id is paid, the next one is not, and the distributor holds nothing"
expect_paid "$FIRST" true
expect_paid "$CORRECTION" false
balances

step "3. Idempotency: the same distribution sent again (6307 AlreadyPaid)"
rejected bwb-testnet-treasury 6307 distribute --id "$FIRST" --asset "$USDC" \
  --payouts "$FIRST_LIST" --caller "$TREASURY" \
  -- "$(paid_key "$FIRST")"

step "4. Authorization: investor A, with money of its own, is not the owner (6300 NotOwner)"
rejected bwb-testnet-investor-a 6300 distribute --id "$CORRECTION" --asset "$USDC" \
  --payouts "$(payouts "$INV_B" 1)" --caller "$INV_A"

step "5. A correction is a new distribution: investor B was owed 75, so 25 more under a new id"
echo "id: $CORRECTION"
ok bwb-testnet-treasury distribute --id "$CORRECTION" --asset "$USDC" \
  --payouts "$(payouts "$INV_B" 25)" --caller "$TREASURY"

step "6. The platform pauses the distributor"
PAUSED_BY_RUN=1
ok bwb-testnet-platform pause --caller "$PLATFORM"

step "7. Pause: a distribution while paused (1000 EnforcedPause)"
rejected bwb-testnet-treasury 1000 distribute --id "$PAUSED" --asset "$USDC" \
  --payouts "$(payouts "$INV_A" 1)" --caller "$TREASURY"

step "8. Only the owner unpauses"
ok bwb-testnet-treasury unpause --caller "$TREASURY"
PAUSED_BY_RUN=0

step "9. All or nothing: a list with a recipient that has no USDC trustline fails in simulation"
# The platform account never opened a trustline for the demo USDC. The
# simulation fails with the asset's own code, so nothing is signed or sent:
# this is where the backend learns to take the recipient out of the list.
if out="$(stellar contract invoke --network "$NETWORK" --source bwb-testnet-treasury --id "$DISTRIBUTOR" --send=no -- \
  distribute --id "$NO_TRUSTLINE" --asset "$USDC" \
  --payouts "$(payouts "$INV_A" 10 "$PLATFORM" 10)" --caller "$TREASURY" 2>&1)"; then
  echo "the simulation passed but was expected to fail" >&2
  exit 1
fi
grep -q 'Error(Contract, #13)' <<<"$out" || { echo "$out" >&2; exit 1; }
echo "simulation failed with Error(Contract, #13) TrustlineMissingError; nothing was sent"
expect_paid "$NO_TRUSTLINE" false

step "10. A payout to the asset's own contract would be stranded there (6306 InvalidRecipient)"
rejected bwb-testnet-treasury 6306 distribute --id "$TO_ASSET" --asset "$USDC" \
  --payouts "$(payouts "$INV_A" 1 "$USDC" 1)" --caller "$TREASURY" \
  -- "$(paid_key "$TO_ASSET")"

step "After the run"
balances
# What the run paid, and nothing else: 100 to A, 50 + 25 to B.
balance_of "$INV_A"
[[ "$READ" == "$((A_BEFORE + 100 * USDC_UNIT))" ]] || { echo "investor A holds $READ, expected $((A_BEFORE + 100 * USDC_UNIT))" >&2; exit 1; }
balance_of "$INV_B"
[[ "$READ" == "$((B_BEFORE + 75 * USDC_UNIT))" ]] || { echo "investor B holds $READ, expected $((B_BEFORE + 75 * USDC_UNIT))" >&2; exit 1; }
echo "balances moved by exactly what the run paid"
