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
  echo "python3 is required to convert BRLA amounts" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"
DEFAULT_AMOUNT_BRLA="100"
ACCOUNT_ALIAS="deployer"

if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  # shellcheck disable=SC1091
  source "${PROJECT_ROOT}/.env"
fi

DEPLOYMENT_ENVIRONMENT="$(printf '%s' "${DEPLOYMENT_ENVIRONMENT:-production}" | tr '[:upper:]' '[:lower:]')"

usage() {
  cat <<'EOF'
Usage: ./shells/fund-investors-brla.sh [--amount-brla 1000] [--token 0x...] [--config path]

Transfers BRLA from the deployer account to investor1/investor2/investor3 using
script/FundInvestorsWithBrla.s.sol. Run inside the contracts nix-shell so forge
and cast share the configured keystores.

Options:
  --amount-brla <value>   Amount of BRLA (per investor) to transfer (default 1000)
  --token <address>       Override BRLA token address (otherwise resolves from env/config)
  --config <path>         Explicit deployment JSON (defaults to configs/deployment-$DEPLOYMENT_ENVIRONMENT.json)
  -h, --help              Show this help message
EOF
}

require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Environment variable ${var_name} is required" >&2
    exit 1
  fi
}

determine_config_path() {
  if [[ -n "$CONFIG_ARG" ]]; then
    CONFIG_PATH="$CONFIG_ARG"
  elif [[ -n "${DEPLOYMENT_CONFIG_PATH:-}" ]]; then
    CONFIG_PATH="$DEPLOYMENT_CONFIG_PATH"
  else
    CONFIG_PATH="${CONFIGS_DIR}/deployment-${DEPLOYMENT_ENVIRONMENT}.json"
  fi

  if [[ -z "$CONFIG_PATH" ]]; then
    return
  fi

  if [[ ! -f "$CONFIG_PATH" ]]; then
    echo "Deployment summary not found at ${CONFIG_PATH}. Provide --token or regenerate the summary." >&2
    exit 1
  fi

  echo "Using deployment summary: ${CONFIG_PATH}"
}

resolve_from_config() {
  local key="$1"
  if [[ -z "$CONFIG_PATH" ]]; then
    return 1
  fi
  local value
  value=$(jq -r --arg k "$key" '.contracts[$k] // empty' "$CONFIG_PATH")
  if [[ -z "$value" || "$value" == "null" ]]; then
    return 1
  fi
  printf '%s' "$value"
}

resolve_brla_address() {
  if [[ -n "$TOKEN_ARG" ]]; then
    BRLA_TOKEN_ADDRESS="$TOKEN_ARG"
  elif [[ -n "${BRLA_TOKEN_ADDRESS:-}" ]]; then
    BRLA_TOKEN_ADDRESS="$BRLA_TOKEN_ADDRESS"
  elif value=$(resolve_from_config "brlaToken" 2>/dev/null); then
    BRLA_TOKEN_ADDRESS="$value"
  else
    echo "Unable to determine BRLA token address. Provide --token or set BRLA_TOKEN_ADDRESS." >&2
    exit 1
  fi
  export BRLA_TOKEN_ADDRESS
  echo "Using BRLA token: ${BRLA_TOKEN_ADDRESS}"
}

resolve_account_env() {
  local env_key="$1"
  local alias="$2"
  local current_value="${!env_key:-}"
  if [[ -n "$current_value" ]]; then
    export "${env_key}=${current_value}"
    return
  fi

  local derived
  if derived=$(cast wallet address "$alias" 2>/dev/null); then
    export "${env_key}=${derived}"
    return
  fi

  echo "Unable to resolve address for ${alias}. Set ${env_key}." >&2
  exit 1
}

resolve_funder_address() {
  if [[ -n "${DEPLOYER_ADDRESS:-}" ]]; then
    printf '%s' "$DEPLOYER_ADDRESS"
    return
  fi
  local derived
  if derived=$(cast wallet address "$ACCOUNT_ALIAS" 2>/dev/null); then
    printf '%s' "$derived"
    return
  fi
  echo "Unable to resolve funder address. Set DEPLOYER_ADDRESS or import the ${ACCOUNT_ALIAS} keystore." >&2
  exit 1
}

erc20_balance_of() {
  local token_address="$1"
  local holder="$2"
  cast call "$token_address" "balanceOf(address)(uint256)" "$holder" --rpc-url "$RPC_URL" | {
    read -r first _
    printf '%s' "$first"
  }
}

compare_uint256() {
  local left="$1"
  local right="$2"
  python3 - "$left" "$right" <<'PY'
import sys
l = int(sys.argv[1])
r = int(sys.argv[2])
print('lt' if l < r else ('eq' if l == r else 'gt'))
PY
}

format_brla_amount() {
  local wei_value="$1"
  python3 - "$wei_value" <<'PY'
from decimal import Decimal, getcontext
import sys
getcontext().prec = 80
value = Decimal(int(sys.argv[1])) / Decimal(10 ** 18)
text = format(value, 'f')
if '.' in text:
    text = text.rstrip('0').rstrip('.')
print(text or '0')
PY
}

convert_brla_to_wei() {
  local amount="$1"
  python3 - "$amount" <<'PY'
from decimal import Decimal, getcontext
import sys
getcontext().prec = 80
value = Decimal(sys.argv[1])
if value <= 0:
    raise SystemExit("BRLA amount must be positive")
wei = int(value * (10 ** 18))
print(wei)
PY
}

print_summary() {
  local amount_wei="$1"
  local total_wei="$2"
  local funder_balance_wei="$3"
  local funder_balance_units
  funder_balance_units=$(format_brla_amount "$funder_balance_wei")
  local total_units
  total_units=$(format_brla_amount "$total_wei")
  echo "\n==> Funding configuration"
  echo "BRLA token: ${BRLA_TOKEN_ADDRESS}"
  echo "Amount per investor: ${AMOUNT_BRLA} BRLA (${amount_wei} wei)"
  echo "Investors:"
  echo "  investor1 -> ${INVESTOR1_ADDRESS}"
  echo "  investor2 -> ${INVESTOR2_ADDRESS}"
  echo "  investor3 -> ${INVESTOR3_ADDRESS}"
  echo "Funder (${ACCOUNT_ALIAS}): ${FUNDER_ADDRESS}"
  echo "  Balance: ${funder_balance_wei} wei (${funder_balance_units} BRLA)"
  echo "  Total required: ${total_wei} wei (${total_units} BRLA)"
}

enforce_funder_balance() {
  local cmp
  cmp=$(compare_uint256 "$FUNDER_BRLA_BALANCE" "$TOTAL_REQUIRED_BRLA_WEI")
  if [[ "$cmp" == "lt" ]]; then
    local shortfall
    shortfall=$(
      python3 - "$TOTAL_REQUIRED_BRLA_WEI" "$FUNDER_BRLA_BALANCE" <<'PY'
import sys
required = int(sys.argv[1])
balance = int(sys.argv[2])
print(required - balance)
PY
    )
    local shortfall_units
    shortfall_units=$(format_brla_amount "$shortfall")
    echo "Funder ${FUNDER_ADDRESS} lacks sufficient BRLA." >&2
    echo "  Required: ${TOTAL_REQUIRED_BRLA_WEI} wei ($(format_brla_amount "$TOTAL_REQUIRED_BRLA_WEI") BRLA)" >&2
    echo "  Current:  ${FUNDER_BRLA_BALANCE} wei ($(format_brla_amount "$FUNDER_BRLA_BALANCE") BRLA)" >&2
    echo "  Shortfall: ${shortfall} wei (${shortfall_units} BRLA)" >&2
    exit 1
  fi
}

AMOUNT_BRLA="$DEFAULT_AMOUNT_BRLA"
TOKEN_ARG=""
CONFIG_ARG=""
CONFIG_PATH=""
NUM_INVESTORS=3
FUNDER_ADDRESS=""
FUNDER_BRLA_BALANCE=""
TOTAL_REQUIRED_BRLA_WEI=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --amount-brla)
    AMOUNT_BRLA="$2"
    shift 2
    ;;
  --token)
    TOKEN_ARG="$2"
    shift 2
    ;;
  --config)
    CONFIG_ARG="$2"
    shift 2
    ;;
  -h | --help)
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

determine_config_path
resolve_brla_address
resolve_account_env "INVESTOR1_ADDRESS" "investor1"
resolve_account_env "INVESTOR2_ADDRESS" "investor2"
resolve_account_env "INVESTOR3_ADDRESS" "investor3"
FUNDER_ADDRESS=$(resolve_funder_address)

BRLA_FUND_PER_INVESTOR_WEI=$(convert_brla_to_wei "$AMOUNT_BRLA")
TOTAL_REQUIRED_BRLA_WEI=$(
  python3 - "$BRLA_FUND_PER_INVESTOR_WEI" "$NUM_INVESTORS" <<'PY'
import sys
per = int(sys.argv[1])
count = int(sys.argv[2])
print(per * count)
PY
)
FUNDER_BRLA_BALANCE=$(erc20_balance_of "$BRLA_TOKEN_ADDRESS" "$FUNDER_ADDRESS")

print_summary "$BRLA_FUND_PER_INVESTOR_WEI" "$TOTAL_REQUIRED_BRLA_WEI" "$FUNDER_BRLA_BALANCE"
enforce_funder_balance

export BRLA_FUND_PER_INVESTOR_WEI

(
  cd "$PROJECT_ROOT"
  forge script script/FundInvestorsWithBrla.s.sol:FundInvestorsWithBrla \
    --rpc-url "$RPC_URL" \
    --chain "$CHAIN_ID" \
    --broadcast \
    --account "$ACCOUNT_ALIAS"
)

echo "\nBRLA funding complete."
