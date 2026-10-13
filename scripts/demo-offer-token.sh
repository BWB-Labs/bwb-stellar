#!/usr/bin/env bash
# Exercise a deployed offer token on testnet and leave a public trail: each
# step is a transaction, successful or rejected, with its explorer link.
#
#   OFFERING=<id> scripts/demo-offer-token.sh
#
# The token is read from deployments/testnet.json. Its supply sits at the
# sale contract's precomputed address, which has no code yet, so xfer_admin
# moves quotas out of it to two test investors first.
#
# Identities (stellar keys): bwb-testnet-treasury (admin and xfer_admin),
# bwb-testnet-platform (pauser), bwb-testnet-investor-a and -b, all funded.
# Testnet only: every command passes --network testnet.
set -euo pipefail

OFFERING="${OFFERING:?OFFERING is required}"
NETWORK=testnet
EXPLORER="https://stellar.expert/explorer/testnet"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/deployments/testnet.json"

TOKEN="$(jq -r --arg o "$OFFERING" '.offerings[$o].token.contract_id' "$MANIFEST")"
WASM_HASH="$(jq -r --arg o "$OFFERING" '.offerings[$o].token.wasm_hash' "$MANIFEST")"
SALE="$(jq -r --arg o "$OFFERING" '.offerings[$o].sale.contract_id' "$MANIFEST")"
[[ "$TOKEN" == C* ]] || { echo "no token recorded for offering $OFFERING" >&2; exit 1; }

TREASURY="$(stellar keys address bwb-testnet-treasury)"
PLATFORM="$(stellar keys address bwb-testnet-platform)"
INV_A="$(stellar keys address bwb-testnet-investor-a)"
INV_B="$(stellar keys address bwb-testnet-investor-b)"

step() { printf '\n### %s\n' "$*"; }

# ok <source> <fn> [args...]: a call that must succeed. Prints its hash,
# link and events.
ok() {
  local source="$1"; shift
  local out
  out="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$TOKEN" -- "$@" 2>&1)" \
    || { echo "$out" >&2; exit 1; }
  echo "tx: $(grep -o "$EXPLORER/tx/[0-9a-f]*" <<<"$out" | head -n1)"
  grep -o 'Event: .*' <<<"$out" || true
}

# rejected <source> <expected-code> <fn> [args...] [-- <storage keys>]:
# a call that must fail with the given contract error, recorded on chain.
rejected() {
  local source="$1" expect="$2"; shift 2
  local args=() keys=()
  while (($#)) && [[ "$1" != -- ]]; do args+=("$1"); shift; done
  if (($#)); then shift; for k in "$@"; do keys+=(--key "$k"); done; fi
  local hash
  hash="$(stellar contract invoke --network "$NETWORK" --source "$source" --id "$TOKEN" --build-only -- "${args[@]}" \
    | python3 "$ROOT/scripts/lib/send_rejected.py" --network "$NETWORK" --source "$source" \
      --wasm-hash "$WASM_HASH" --expect "$expect" ${keys[@]+"${keys[@]}"})"
  echo "tx: $EXPLORER/tx/$hash"
  echo "rejected on chain with Error(Contract, #$expect)"
}

whitelisted_key() { printf '{"vec":[{"symbol":"Whitelisted"},{"address":"%s"}]}' "$1"; }

balances() {
  for who in "sale:$SALE" "investor-a:$INV_A" "investor-b:$INV_B" "platform:$PLATFORM"; do
    printf '%s %s\n' "${who%%:*}" \
      "$(stellar contract invoke --network "$NETWORK" --source bwb-testnet-treasury --id "$TOKEN" --send=no -- balance --account "${who#*:}" 2>/dev/null)"
  done
}

echo "token $TOKEN ($EXPLORER/contract/$TOKEN)"
echo "sale (precomputed, initial holder) $SALE"

step "1. xfer_admin moves 100 quotas from the sale address to investor A"
ok bwb-testnet-treasury admin_transfer --from "$SALE" --to "$INV_A" --amount 100 --caller "$TREASURY"

step "2. Investor A to investor B: rejected, neither side is whitelisted (6000 TransferNotWhitelisted)"
rejected bwb-testnet-investor-a 6000 transfer --from "$INV_A" --to "$INV_B" --amount 10 \
  -- "$(whitelisted_key "$INV_A")" "$(whitelisted_key "$INV_B")"

step "3. Investor A to the sale address: allowed, the receiver is whitelisted"
ok bwb-testnet-investor-a transfer --from "$INV_A" --to "$SALE" --amount 10

step "4. admin whitelists an operational account (platform)"
ok bwb-testnet-treasury set_transfer_whitelist --account "$PLATFORM" --allowed true --caller "$TREASURY"

step "5. Investor B tries to whitelist itself: rejected (6003 NotAdmin)"
rejected bwb-testnet-investor-b 6003 set_transfer_whitelist --account "$INV_B" --allowed true --caller "$INV_B" \
  -- "$(whitelisted_key "$INV_B")"

step "6. pauser pauses the token"
ok bwb-testnet-platform pause --caller "$PLATFORM"

step "7. Investor A to the sale address while paused: rejected (1000 EnforcedPause)"
rejected bwb-testnet-investor-a 1000 transfer --from "$INV_A" --to "$SALE" --amount 5

step "8. xfer_admin moves 5 quotas to investor B while paused: allowed"
ok bwb-testnet-treasury admin_transfer --from "$SALE" --to "$INV_B" --amount 5 --caller "$TREASURY"

step "9. pauser tries to unpause: rejected, only admin unpauses (6003 NotAdmin)"
rejected bwb-testnet-platform 6003 unpause --caller "$PLATFORM"

step "10. admin unpauses"
ok bwb-testnet-treasury unpause --caller "$TREASURY"

step "11. Investor A to the sale address after unpause: allowed"
ok bwb-testnet-investor-a transfer --from "$INV_A" --to "$SALE" --amount 5

step "12. Investor B tries to burn: rejected, supply is fixed (6002 BurnDisabled)"
rejected bwb-testnet-investor-b 6002 burn --from "$INV_B" --amount 1

step "13. admin updates the offer URI"
ok bwb-testnet-treasury set_offer_uri --uri "ipfs://$OFFERING-v2" --caller "$TREASURY"

step "Final balances"
balances
