#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "The 'jq' CLI is required to run this script" >&2
  exit 1
fi

if ! command -v forge >/dev/null 2>&1; then
  echo "Foundry 'forge' CLI is required to run this script" >&2
  exit 1
fi

if ! command -v cast >/dev/null 2>&1; then
  echo "Foundry 'cast' CLI is required to run this script" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required to perform high-precision arithmetic" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  # shellcheck disable=SC1091
  source "${PROJECT_ROOT}/.env"
fi

BROADCAST_ROOT="${PROJECT_ROOT}/${FOUNDRY_BROADCAST:-broadcast/production}"
DEPLOY_SCRIPT_DIR="${BROADCAST_ROOT}/DeployOfferTokenSale.s.sol"

INVESTOR_ALIASES=("investor1" "investor2" "investor3")
DEFAULT_FINALIZER_ALIAS="deployer"
WITHDRAW_ACCOUNT_ALIAS="deployer"

usage() {
  cat <<'EOF'
Usage: ./shells/buyout-offer-token-sale.sh [--sale-address 0x...] [--finalizer-account deployer]

Automates purchasing every remaining OfferTokenSale allocation using accounts
investor1/investor2/investor3 before finalizing the sale. Run inside the
contracts nix-shell so forge and cast share the configured keystores.

Optional flags:
  --sale-address          Explicit OfferTokenSale address (defaults to env or last broadcast)
  --finalizer-account     Account alias that owns the sale (default: deployer)
  -h, --help              Show this help message

Environment variables consumed by this script:
  RPC_URL, CHAIN_ID, OFFER_TOKEN_SALE_ADDRESS (optional), INVESTOR*_ADDRESS (for logging)
EOF
}

log_step() {
  printf '\n==> %s\n' "$1"
}

require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Environment variable ${var_name} is required" >&2
    exit 1
  fi
}

normalize_bool() {
  local value="${1,,}"
  case "$value" in
  true|1|0x1)
    printf 'true'
    ;;
  false|0|0x0)
    printf 'false'
    ;;
  *)
    printf '%s' "$1"
    ;;
  esac
}

strip_cast_output() {
  local raw="$1"
  local first
  read -r first _ <<<"$raw"
  printf '%s' "$first"
}

call_sale_u256() {
  local signature="$1"
  local raw
  raw=$(cast call "$SALE_ADDRESS" "$signature" --rpc-url "$RPC_URL")
  strip_cast_output "$raw"
}

call_sale_bool() {
  local signature="$1"
  local raw
  raw=$(cast call "$SALE_ADDRESS" "$signature" --rpc-url "$RPC_URL")
  raw=$(strip_cast_output "$raw")
  normalize_bool "$raw"
}

call_sale_address() {
  local signature="$1"
  strip_cast_output "$(cast call "$SALE_ADDRESS" "$signature" --rpc-url "$RPC_URL")"
}

erc20_balance_of() {
  local token_address="$1"
  local holder="$2"
  strip_cast_output "$(cast call "$token_address" "balanceOf(address)(uint256)" "$holder" --rpc-url "$RPC_URL")"
}

compare_uint256() {
  local left="$1"
  local right="$2"
  python3 - "$left" "$right" <<'PY'
import sys
left = int(sys.argv[1])
right = int(sys.argv[2])
print('lt' if left < right else ('eq' if left == right else 'gt'))
PY
}

format_brla_amount() {
  local wei_value="$1"
  python3 - "$wei_value" <<'PY'
from decimal import Decimal, getcontext
import sys
getcontext().prec = 60
value = Decimal(int(sys.argv[1])) / Decimal(10 ** 18)
text = format(value, 'f')
if '.' in text:
    text = text.rstrip('0').rstrip('.')
print(text or '0')
PY
}

resolve_deployer_address() {
  if [[ -n "${DEPLOYER_ADDRESS:-}" ]]; then
    printf '%s' "$DEPLOYER_ADDRESS"
    return
  fi
  local derived
  if derived=$(cast wallet address deployer 2>/dev/null); then
    printf '%s' "$derived"
    return
  fi
  echo "DEPLOYER_ADDRESS environment variable is required to inspect balances." >&2
  exit 1
}

alias_to_env_key() {
  local alias="$1"
  local upper
  upper=$(printf '%s' "$alias" | tr '[:lower:]' '[:upper:]')
  printf '%s_ADDRESS' "$upper"
}

resolve_alias_address() {
  local alias="$1"
  local env_key
  env_key=$(alias_to_env_key "$alias")
  local env_value="${!env_key:-}"
  if [[ -n "$env_value" ]]; then
    printf '%s' "$env_value"
    return
  fi
  local derived
  if derived=$(cast wallet address "$alias" 2>/dev/null); then
    printf '%s' "$derived"
    return
  fi
  echo "Unable to resolve address for alias ${alias}. Set ${env_key}." >&2
  exit 1
}

resolve_advisor_address() {
  if [[ -n "${ADVISOR_ADDRESS:-}" ]]; then
    printf '%s' "$ADVISOR_ADDRESS"
    return
  fi
  if derived=$(cast wallet address advisor 2>/dev/null); then
    ADVISOR_ADDRESS="$derived"
    export ADVISOR_ADDRESS
    printf '%s' "$derived"
    return
  fi
  echo "Set ADVISOR_ADDRESS or configure an 'advisor' wallet alias to receive BRLA." >&2
  exit 1
}

build_release_recipients_json() {
  local entries=()
  for alias in "${INVESTOR_ALIASES[@]}"; do
    local addr
    addr=$(resolve_alias_address "$alias")
    entries+=("\"${addr}\"")
  done
  local joined
  local IFS=','
  joined="${entries[*]}"
  printf '[%s]' "$joined"
}

withdraw_brla_to_advisor() {
  if [[ -z "$BRLA_TOKEN_ADDRESS" ]]; then
    echo "Cannot withdraw BRLA: token address unknown." >&2
    return
  fi
  local advisor_address
  advisor_address=$(resolve_advisor_address)
  local sale_balance
  sale_balance=$(erc20_balance_of "$BRLA_TOKEN_ADDRESS" "$SALE_ADDRESS")
  if [[ "$sale_balance" == "0" ]]; then
    echo "Sale holds no BRLA to withdraw; skipping transfer."
    return
  fi
  log_step "Withdrawing BRLA proceeds to advisor"
  (
    cd "$PROJECT_ROOT"
    OFFER_TOKEN_SALE_ADDRESS="$SALE_ADDRESS" \
    ADVISOR_ADDRESS="$advisor_address" \
    BRLA_WITHDRAW_AMOUNT="$sale_balance" \
    forge script script/WithdrawOfferSaleBrla.s.sol:WithdrawOfferSaleBrla \
      --rpc-url "$RPC_URL" \
      --chain "$CHAIN_ID" \
      --broadcast \
      --account "$WITHDRAW_ACCOUNT_ALIAS"
  )
}

release_tokens_to_investors() {
  local recipients_json
  recipients_json=$(build_release_recipients_json)
  log_step "Releasing offer tokens to investors"
  (
    cd "$PROJECT_ROOT"
    OFFER_TOKEN_SALE_ADDRESS="$SALE_ADDRESS" \
    RELEASE_RECIPIENTS_JSON="$recipients_json" \
    forge script script/ReleaseOfferTokens.s.sol:ReleaseOfferTokens \
      --rpc-url "$RPC_URL" \
      --chain "$CHAIN_ID" \
      --broadcast \
      --account "$FINALIZER_ALIAS"
  )
}

read_latest_sale_from_broadcast() {
  if [[ -z "${CHAIN_ID:-}" ]]; then
    echo "CHAIN_ID must be set to read sale broadcast data" >&2
    exit 1
  fi

  local broadcast_file="${DEPLOY_SCRIPT_DIR}/${CHAIN_ID}/run-latest.json"
  if [[ ! -f "$broadcast_file" ]]; then
    echo "Broadcast file not found for DeployOfferTokenSale (expected ${broadcast_file}). Provide --sale-address." >&2
    exit 1
  fi

  local sale_address
  sale_address=$(jq -r '.returns.sale.value // empty' "$broadcast_file")
  if [[ -z "$sale_address" || "$sale_address" == "null" ]]; then
    echo "Unable to read sale address from ${broadcast_file}. Provide --sale-address." >&2
    exit 1
  fi

  printf '%s' "$sale_address"
}

resolve_sale_address() {
  if [[ -n "$SALE_ADDRESS_ARG" ]]; then
    SALE_ADDRESS="$SALE_ADDRESS_ARG"
  elif [[ -n "${OFFER_TOKEN_SALE_ADDRESS:-}" ]]; then
    SALE_ADDRESS="$OFFER_TOKEN_SALE_ADDRESS"
  else
    SALE_ADDRESS=$(read_latest_sale_from_broadcast)
  fi

  export OFFER_TOKEN_SALE_ADDRESS="$SALE_ADDRESS"
  echo "Using OfferTokenSale: ${SALE_ADDRESS}"
}

fetch_sale_state() {
  TOTAL_TOKENS=$(call_sale_u256 "totalTokensForSale()(uint256)")
  SOLD_TOKENS=$(call_sale_u256 "soldTokens()(uint256)")
  BRLA_PER_TOKEN=$(call_sale_u256 "brlaPerToken()(uint256)")
  BRLA_TOKEN_ADDRESS=$(call_sale_address "brlaToken()(address)")
  INVENTORY_SEEDED=$(call_sale_bool "inventorySeeded()(bool)")
  IS_FINALIZED=$(call_sale_bool "isFinalized()(bool)")
  REMAINING_TOKENS=$(python3 - "$TOTAL_TOKENS" "$SOLD_TOKENS" <<'PY'
import sys
print(int(sys.argv[1]) - int(sys.argv[2]))
PY
  )
}


print_sale_summary() {
  log_step "OfferTokenSale snapshot"
  local brla_per_token_units
  brla_per_token_units=$(format_brla_amount "$BRLA_PER_TOKEN")
  echo "Sale address: ${SALE_ADDRESS}"
  echo "Total tokens for sale: ${TOTAL_TOKENS}"
  echo "Sold tokens: ${SOLD_TOKENS}"
  echo "Remaining tokens: ${REMAINING_TOKENS}"
  echo "BRLA per token: ${BRLA_PER_TOKEN} wei (${brla_per_token_units} BRLA)"
  echo "Inventory seeded: ${INVENTORY_SEEDED}"
  echo "Sale finalized: ${IS_FINALIZED}"
}

build_purchase_plan() {
  if [[ "$REMAINING_TOKENS" == "0" ]]; then
    PURCHASE_PLAN=()
    return
  fi

  mapfile -t PURCHASE_PLAN < <(python3 - "$REMAINING_TOKENS" "$BRLA_PER_TOKEN" "${INVESTOR_ALIASES[@]}" <<'PY'
import sys
remaining = int(sys.argv[1])
price = int(sys.argv[2])
aliases = sys.argv[3:]
if remaining <= 0 or not aliases:
    sys.exit(0)
base = remaining // len(aliases)
remainder = remaining % len(aliases)
for idx, alias in enumerate(aliases):
    share = base + (1 if idx < remainder else 0)
    if share == 0:
        continue
    brla_amount = share * price
    print(f"{alias}:{share}:{brla_amount}")
PY
  )
}

print_purchase_plan() {
  if [[ ${#PURCHASE_PLAN[@]} -eq 0 ]]; then
    if [[ "$REMAINING_TOKENS" != "0" ]]; then
      echo "Unable to build purchase plan for remaining tokens" >&2
      exit 1
    fi
    echo "No purchases required; sale already sold out."
    return
  fi

  echo
  echo "Purchase plan (alias : tokens : BRLA)"
  for entry in "${PURCHASE_PLAN[@]}"; do
    IFS=':' read -r alias tokens brla <<<"$entry"
    local brla_units
    brla_units=$(format_brla_amount "$brla")
    printf '  %-10s %-12s %s wei (%s BRLA)\n' "$alias" "$tokens" "$brla" "$brla_units"
  done
}

print_deployer_brla_balance() {
  if [[ -z "$BRLA_TOKEN_ADDRESS" ]]; then
    echo "Unable to resolve BRLA token address from sale contract." >&2
    return
  fi
  local deployer_address
  deployer_address=$(resolve_deployer_address)
  local deployer_balance
  deployer_balance=$(erc20_balance_of "$BRLA_TOKEN_ADDRESS" "$deployer_address")
  local deployer_balance_units
  deployer_balance_units=$(format_brla_amount "$deployer_balance")
  echo
  echo "BRLA balance summary"
  printf '  Deployer (%s): %s wei (%s BRLA)\n' "$deployer_address" "$deployer_balance" "$deployer_balance_units"
}

print_investor_brla_balances() {
  if [[ -z "$BRLA_TOKEN_ADDRESS" ]]; then
    return
  fi
  echo "  Investors:"
  for alias in "${INVESTOR_ALIASES[@]}"; do
    local alias_address
    alias_address=$(resolve_alias_address "$alias")
    local balance
    balance=$(erc20_balance_of "$BRLA_TOKEN_ADDRESS" "$alias_address")
    local balance_units
    balance_units=$(format_brla_amount "$balance")
    printf '    %-9s (%s): %s wei (%s BRLA)\n' "$alias" "$alias_address" "$balance" "$balance_units"
  done
}

enforce_investor_balances() {
  if [[ -z "$BRLA_TOKEN_ADDRESS" ]]; then
    return
  fi
  local missing_funds="false"
  for entry in "${PURCHASE_PLAN[@]}"; do
    IFS=':' read -r alias _requiredTokens required_brla <<<"$entry"
    local alias_address
    alias_address=$(resolve_alias_address "$alias")
    local balance
    balance=$(erc20_balance_of "$BRLA_TOKEN_ADDRESS" "$alias_address")
    local cmp
    cmp=$(compare_uint256 "$balance" "$required_brla")
    if [[ "$cmp" == "lt" ]]; then
      local required_units balance_units deficit_units
      required_units=$(format_brla_amount "$required_brla")
      balance_units=$(format_brla_amount "$balance")
      echo "Alias ${alias} at ${alias_address} has insufficient BRLA." >&2
      echo "  Required: ${required_brla} wei (${required_units} BRLA)" >&2
      echo "  Current:  ${balance} wei (${balance_units} BRLA)" >&2
      missing_funds="true"
    fi
  done

  if [[ "$missing_funds" == "true" ]]; then
    echo "Fund the investor accounts with enough BRLA before re-running." >&2
    exit 1
  fi
}

run_purchase_for_alias() {
  local alias="$1"
  local tokens="$2"
  local brla_amount="$3"

  if [[ "$tokens" == "0" ]]; then
    return
  fi

  local alias_address
  alias_address=$(resolve_alias_address "$alias")

  log_step "Buying ${tokens} tokens using ${alias}"
  (
    cd "$PROJECT_ROOT"
    OFFER_TOKEN_SALE_ADDRESS="$SALE_ADDRESS" \
    BRLA_AMOUNT="$brla_amount" \
    forge script script/BuyOfferTokens.s.sol:BuyOfferTokens \
      --rpc-url "$RPC_URL" \
      --chain "$CHAIN_ID" \
      --broadcast \
      --sender "$alias_address" \
      --account "$alias"
  )
}

run_purchases() {
  for entry in "${PURCHASE_PLAN[@]}"; do
    IFS=':' read -r alias tokens brla <<<"$entry"
    run_purchase_for_alias "$alias" "$tokens" "$brla"
  done
}

finalize_sale() {
  if [[ "$IS_FINALIZED" == "true" ]]; then
    echo "Sale already finalized; skipping finalize transaction."
    return
  fi

  log_step "Finalizing OfferTokenSale via ${FINALIZER_ALIAS}"
  (
    cd "$PROJECT_ROOT"
    OFFER_TOKEN_SALE_ADDRESS="$SALE_ADDRESS" \
    forge script script/FinalizeOfferTokenSale.s.sol:FinalizeOfferTokenSale \
      --rpc-url "$RPC_URL" \
      --chain "$CHAIN_ID" \
      --broadcast \
      --account "$FINALIZER_ALIAS"
  )
}

SALE_ADDRESS_ARG=""
FINALIZER_ALIAS="$DEFAULT_FINALIZER_ALIAS"
BRLA_TOKEN_ADDRESS=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --sale-address|--sale)
    SALE_ADDRESS_ARG="$2"
    shift 2
    ;;
  --finalizer-account)
    FINALIZER_ALIAS="$2"
    shift 2
    ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    echo "Unknown option: $1" >&2
    usage >&2
    exit 1
    ;;
  esac
done

require_env "RPC_URL"
require_env "CHAIN_ID"
resolve_sale_address

fetch_sale_state
print_sale_summary
print_deployer_brla_balance
print_investor_brla_balances

if [[ "$INVENTORY_SEEDED" != "true" ]]; then
  echo "Inventory must be seeded before running purchases." >&2
  exit 1
fi

if [[ "$IS_FINALIZED" == "true" ]]; then
  if [[ "$REMAINING_TOKENS" == "0" ]]; then
    echo "Sale is already finalized and sold out. Running post-finalization tasks."
    withdraw_brla_to_advisor
    release_tokens_to_investors
    exit 0
  fi
  echo "Sale is finalized but tokens remain recorded. Investigate manually." >&2
  exit 1
fi

build_purchase_plan
print_purchase_plan
enforce_investor_balances

if [[ "$REMAINING_TOKENS" != "0" ]]; then
  run_purchases
  fetch_sale_state
  echo
  echo "Post-purchase sold tokens: ${SOLD_TOKENS}"
  echo "Post-purchase remaining tokens: ${REMAINING_TOKENS}"
  if [[ "$REMAINING_TOKENS" != "0" ]]; then
    echo "Sale still has remaining tokens after purchases. Adjust allowlist allocations or balances." >&2
    exit 1
  fi
fi

finalize_sale
fetch_sale_state

if [[ "$IS_FINALIZED" != "true" ]]; then
  echo "Finalize transaction did not complete successfully. Check logs." >&2
  exit 1
fi

withdraw_brla_to_advisor
release_tokens_to_investors

echo
echo "OfferTokenSale is fully sold out and finalized."
