#!/usr/bin/env bash
# Exercise a deployed holdings distributor on testnet and leave a public
# trail: each step is a transaction, successful or rejected, with its
# explorer link. It doubles as an integration test of the distributor
# beside the real token, a real payment asset and classic accounts with
# trustlines.
#
#   scripts/demo-offer-holdings-distributor.sh
#
# The holdings distributor of offering `t1-demo-4` from
# deployments/testnet.json, whose 800 quotas are held by investor A (300)
# and investor B (500). The run walks: a stranger refused a deposit, a
# deposit of 1 USDC per quota, a claim sent by a third party, a second
# claim that pays only who was not paid yet, a second deposit, a pause with
# a refused deposit, a withdrawal that leaves the distributor unable to
# pay, a claim that skips both holders with `claim_skipped`, the money
# returned, the same claim paying both, and a stranger refused a
# withdrawal.
#
# Every run ends with nothing owed and the distributor's balance as it
# found it, so the script can be run again.
#
# Identities (stellar keys): bwb-testnet-issuer (owner, holds the demo USDC
# it deposits), bwb-testnet-platform (pauser), bwb-testnet-investor-a and
# -b (holders, trustlines open), bwb-testnet-deployer (reads, and sends the
# claims: any account can). Testnet only: every command passes --network
# testnet.
set -euo pipefail

NETWORK=testnet
OFFERING=t1-demo-4
EXPLORER="https://stellar.expert/explorer/testnet"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/deployments/testnet.json"

DISTRIBUTOR="$(jq -r --arg o "$OFFERING" '.offerings[$o].holdings_distributor.contract_id' "$MANIFEST")"
WASM_HASH="$(jq -r --arg o "$OFFERING" '.offerings[$o].holdings_distributor.wasm_hash' "$MANIFEST")"
TOKEN="$(jq -r --arg o "$OFFERING" '.offerings[$o].token.contract_id' "$MANIFEST")"
USDC="$(jq -r '.contracts["demo-usdc"].contract_id' "$MANIFEST")"
[[ "$DISTRIBUTOR" == C* && "$WASM_HASH" != null ]] || {
  echo "no deployed holdings distributor for offering $OFFERING" >&2
  exit 1
}

ISSUER="$(stellar keys address bwb-testnet-issuer)"
PLATFORM="$(stellar keys address bwb-testnet-platform)"
INV_A="$(stellar keys address bwb-testnet-investor-a)"
INV_B="$(stellar keys address bwb-testnet-investor-b)"

USDC_UNIT=10000000   # 1 USDC, in stroops

step() { printf '\n### %s\n' "$*"; }

# invoke_ok <contract> <source> <fn> [args...]: a call that must succeed.
# Prints its hash, link and events.
invoke_ok() {
  local id="$1" source="$2"; shift 2
  local out
  out="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$id" -- "$@" 2>&1)" \
    || { echo "$out" >&2; exit 1; }
  echo "tx: $(grep -o "$EXPLORER/tx/[0-9a-f]*" <<<"$out" | head -n1)"
  grep -o 'Event: .*' <<<"$out" || true
}

# ok <source> <fn> [args...]: invoke_ok on the distributor.
ok() { invoke_ok "$DISTRIBUTOR" "$@"; }

# read_fn <contract> <fn> [args...]: a simulated read. Sets READ to the
# value, the CLI's last line of output, unquoted. It sets a variable
# instead of printing, because a failure inside $(...) would only end the
# subshell: a failed read stops the script with the CLI's error.
read_fn() {
  local id="$1"; shift
  local out
  out="$(stellar contract invoke --network "$NETWORK" --source bwb-testnet-deployer --id "$id" --send=no -- "$@" 2>&1)" \
    || { echo "read failed: $*" >&2; echo "$out" >&2; exit 1; }
  READ="$(printf '%s\n' "$out" | tail -n1)"
  READ="${READ//\"/}"
}

# expect <what> <actual> <expected>: stops unless they are equal.
expect() {
  [[ "$2" == "$3" ]] || { echo "$1 is $2, expected $3" >&2; exit 1; }
}

# balance_of <address>: sets READ to the USDC balance in stroops.
balance_of() { read_fn "$USDC" balance --id "$1"; }

# expect_claimable <holder> <usdc>: reads claimable and stops unless it is
# what the step claims.
expect_claimable() {
  read_fn "$DISTRIBUTOR" claimable --holder "$1"
  echo "claimable($1): $READ"
  expect "claimable" "$READ" "$(($2 * USDC_UNIT))"
}

# rejected <source> <expected-code> <fn> [args...]: a call that must fail
# with the given contract error, recorded on chain.
rejected() {
  local source="$1" expect="$2"; shift 2
  local hash
  hash="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$DISTRIBUTOR" --build-only -- "$@" \
    | python3 "$ROOT/scripts/lib/send_rejected.py" --network "$NETWORK" --source "$source" \
      --wasm-hash "$WASM_HASH" --expect "$expect")"
  echo "tx: $EXPLORER/tx/$hash"
  echo "rejected on chain with Error(Contract, #$expect)"
}

balances() {
  for who in "issuer:$ISSUER" "investor-a:$INV_A" "investor-b:$INV_B" "distributor:$DISTRIBUTOR"; do
    balance_of "${who#*:}"
    printf '%s: %s USDC stroops\n' "${who%%:*}" "$READ"
  done
}

# The run pauses a distributor other runs share. If it stops between the
# pause and the unpause, lift the pause on the way out.
PAUSED_BY_RUN=0
restore_pause() {
  if [[ "$PAUSED_BY_RUN" == 1 ]]; then
    echo "the run stopped while paused: unpausing" >&2
    stellar contract invoke --network "$NETWORK" --source bwb-testnet-issuer --id "$DISTRIBUTOR" -- \
      unpause --caller "$ISSUER" >/dev/null 2>&1 \
      || echo "could not unpause $DISTRIBUTOR; the issuer must do it by hand" >&2
  fi
}
trap restore_pause EXIT

HOLDERS="[\"$INV_A\",\"$INV_B\"]"

echo "offering $OFFERING: holdings distributor $DISTRIBUTOR ($EXPLORER/contract/$DISTRIBUTOR)"
read_fn "$DISTRIBUTOR" powers
echo "powers: $READ"
read_fn "$DISTRIBUTOR" contracts
echo "contracts: $READ"
read_fn "$DISTRIBUTOR" paused
expect "paused" "$READ" false
read_fn "$TOKEN" total_supply
expect "the token's supply" "$READ" 800
read_fn "$TOKEN" balance --account "$INV_A"
expect "investor A's quotas" "$READ" 300
read_fn "$TOKEN" balance --account "$INV_B"
expect "investor B's quotas" "$READ" 500
# A run that stopped halfway may have left shares unclaimed; start clean.
expect_claimable "$INV_A" 0
expect_claimable "$INV_B" 0

step "Before the run"
balances
POT_BEFORE="$READ"
balance_of "$INV_A"; A_BEFORE="$READ"
balance_of "$INV_B"; B_BEFORE="$READ"
balance_of "$ISSUER"; ISSUER_BEFORE="$READ"

step "1. Only the owner deposits: investor A tries with its own money (6400 NotOwner)"
rejected bwb-testnet-investor-a 6400 deposit --amount "$USDC_UNIT" --caller "$INV_A"

step "2. The issuer deposits 800 USDC: 1 USDC per quota, and no holder is touched"
ok bwb-testnet-issuer deposit --amount "$((800 * USDC_UNIT))" --caller "$ISSUER"
expect_claimable "$INV_A" 300
expect_claimable "$INV_B" 500
balance_of "$INV_A"
expect "investor A's balance" "$READ" "$A_BEFORE"

step "3. A third party claims for investor A: the money goes to A, nobody signs for A"
ok bwb-testnet-deployer claim --holders "[\"$INV_A\"]"
balance_of "$INV_A"
expect "investor A's balance" "$READ" "$((A_BEFORE + 300 * USDC_UNIT))"
read_fn "$DISTRIBUTOR" credited --holder "$INV_A"
echo "credited(A): $READ"

step "4. A claim for both: A was paid and is passed over, B receives 500"
ok bwb-testnet-deployer claim --holders "$HOLDERS"
balance_of "$INV_A"
expect "investor A's balance" "$READ" "$((A_BEFORE + 300 * USDC_UNIT))"
balance_of "$INV_B"
expect "investor B's balance" "$READ" "$((B_BEFORE + 500 * USDC_UNIT))"
expect_claimable "$INV_A" 0
expect_claimable "$INV_B" 0

step "5. A second deposit, 80 USDC: each holder is owed only the difference"
ok bwb-testnet-issuer deposit --amount "$((80 * USDC_UNIT))" --caller "$ISSUER"
expect_claimable "$INV_A" 30
expect_claimable "$INV_B" 50

step "6. The platform pauses"
ok bwb-testnet-platform pause --caller "$PLATFORM"
PAUSED_BY_RUN=1

step "7. A deposit while paused (1000 EnforcedPause)"
rejected bwb-testnet-issuer 1000 deposit --amount "$USDC_UNIT" --caller "$ISSUER"

step "8. A claim while paused fails in simulation; nothing is sent"
if out="$(stellar contract invoke --network "$NETWORK" --source bwb-testnet-deployer --id "$DISTRIBUTOR" --send=no -- \
  claim --holders "$HOLDERS" 2>&1)"; then
  echo "the simulation passed but was expected to fail" >&2
  exit 1
fi
grep -q 'Error(Contract, #1000)' <<<"$out" || { echo "$out" >&2; exit 1; }
echo "simulation failed with Error(Contract, #1000) EnforcedPause"

step "9. The issuer withdraws the 80 USDC while paused: it can only go to the issuer's own account"
ok bwb-testnet-issuer withdraw --asset "$USDC" --amount "$((80 * USDC_UNIT))" --caller "$ISSUER"

step "10. Only the owner unpauses"
ok bwb-testnet-issuer unpause --caller "$ISSUER"
PAUSED_BY_RUN=0

step "11. A claim the distributor cannot pay: both holders are skipped, the call succeeds"
ok bwb-testnet-deployer claim --holders "$HOLDERS"
# Skipped, not charged: what they earned is still owed.
expect_claimable "$INV_A" 30
expect_claimable "$INV_B" 50

step "12. The issuer puts the 80 USDC back with an ordinary payment"
invoke_ok "$USDC" bwb-testnet-issuer transfer --from "$ISSUER" --to "$DISTRIBUTOR" --amount "$((80 * USDC_UNIT))"

step "13. The same claim now pays both: 30 to A, 50 to B"
ok bwb-testnet-deployer claim --holders "$HOLDERS"
expect_claimable "$INV_A" 0
expect_claimable "$INV_B" 0

step "14. Only the owner withdraws: investor A tries (6400 NotOwner)"
rejected bwb-testnet-investor-a 6400 withdraw --asset "$USDC" --amount "$USDC_UNIT" --caller "$INV_A"

step "After the run"
balances
# What the run paid, and nothing else: 330 to A, 550 to B, 880 from the
# issuer; the distributor holds what it held before.
expect "the distributor's balance" "$READ" "$POT_BEFORE"
balance_of "$INV_A"
expect "investor A's balance" "$READ" "$((A_BEFORE + 330 * USDC_UNIT))"
balance_of "$INV_B"
expect "investor B's balance" "$READ" "$((B_BEFORE + 550 * USDC_UNIT))"
balance_of "$ISSUER"
expect "the issuer's balance" "$READ" "$((ISSUER_BEFORE - 880 * USDC_UNIT))"
echo "balances moved by exactly what the run paid"
