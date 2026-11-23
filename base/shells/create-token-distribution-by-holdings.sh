#!/usr/bin/env bash
set -euo pipefail

if ! command -v forge >/dev/null 2>&1; then
  echo "Foundry 'forge' CLI is required to run this script" >&2
  exit 1
fi

if ! command -v cast >/dev/null 2>&1; then
  echo "Foundry 'cast' CLI is required to run this script" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "The 'jq' CLI is required to read deployment summaries" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required for BRLA math" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"
ACCOUNT_ALIAS="deployer"
DEFAULT_TOTAL_BRLA="120" # distributes 120 BRLA total unless overridden
DEFAULT_HOLDER_ALIASES=("investor1" "investor2" "investor3")

if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  # shellcheck disable=SC1091
  source "${PROJECT_ROOT}/.env"
fi

DEPLOYMENT_ENVIRONMENT="$(printf '%s' "${DEPLOYMENT_ENVIRONMENT:-production}" | tr '[:upper:]' '[:lower:]')"

usage() {
  cat <<'EOF'
Usage: ./shells/create-token-distribution-by-holdings.sh --offer-token 0x... [options]

Deploys a TokenDistributorByHoldings via its factory, funds it with BRLA from the deployer,
then runs distributeByHoldings for investor1/2/3 (or custom holders) to verify payouts.

Options:
  --offer-token <address>   Offer token used to measure holdings (required)
  --total-brla <value>      Total BRLA to distribute (default 120)
  --holders <a,b,c>         Comma-separated list of aliases or addresses (default investor1,investor2,investor3)
  --factory <address>       Override TokenDistributorByHoldingsFactory address
  --brla <address>          Override BRLA token address
  --config <path>           Deployment JSON to resolve defaults (defaults to configs/deployment-$DEPLOYMENT_ENVIRONMENT.json)
  -h, --help                Show this help message
EOF
}

require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Environment variable ${var_name} is required" >&2
    exit 1
  fi
}

alias_env_key() {
  local alias="$1"
  local upper
  upper=$(printf '%s' "$alias" | tr '[:lower:]' '[:upper:]')
  printf '%s_ADDRESS' "$upper"
}

resolve_address_or_alias() {
  local value="$1"
  local alias="$2"
  if [[ -n "$value" ]]; then
    printf '%s' "$value"
    return
  fi
  local env_key
  env_key=$(alias_env_key "$alias")
  local env_value="${!env_key:-}"
  if [[ -n "$env_value" ]]; then
    printf '%s' "$env_value"
    return
  fi
  if derived=$(cast wallet address "$alias" 2>/dev/null); then
    printf '%s' "$derived"
    return
  fi
  echo "Unable to resolve address for alias ${alias}. Provide explicit address or set ${env_key}." >&2
  exit 1
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
print(int(value * (10 ** 18)))
PY
}

CONFIG_FILE_ARG=""
CONFIG_FILE_PATH=""
TOTAL_BRLA_STR="$DEFAULT_TOTAL_BRLA"
HOLDER_LIST=""
FACTORY_OVERRIDE=""
BRLA_OVERRIDE=""
OFFER_TOKEN_OVERRIDE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --offer-token)
    OFFER_TOKEN_OVERRIDE="$2"
    shift 2
    ;;
  --total-brla)
    TOTAL_BRLA_STR="$2"
    shift 2
    ;;
  --holders)
    HOLDER_LIST="$2"
    shift 2
    ;;
  --factory)
    FACTORY_OVERRIDE="$2"
    shift 2
    ;;
  --brla)
    BRLA_OVERRIDE="$2"
    shift 2
    ;;
  --config)
    CONFIG_FILE_ARG="$2"
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
require_env "DEPLOYER_ADDRESS"

determine_config_path() {
  if [[ -n "$CONFIG_FILE_ARG" ]]; then
    CONFIG_FILE_PATH="$CONFIG_FILE_ARG"
  elif [[ -n "${DEPLOYMENT_CONFIG_PATH:-}" ]]; then
    CONFIG_FILE_PATH="${DEPLOYMENT_CONFIG_PATH}"
  else
    CONFIG_FILE_PATH="${CONFIGS_DIR}/deployment-${DEPLOYMENT_ENVIRONMENT}.json"
  fi
  if [[ -f "$CONFIG_FILE_PATH" ]]; then
    echo "Using deployment summary: ${CONFIG_FILE_PATH}"
  else
    CONFIG_FILE_PATH=""
  fi
}

determine_config_path

maybe_resolve_from_config() {
  local key="$1"
  if [[ -z "$CONFIG_FILE_PATH" ]]; then
    return
  fi
  jq -r --arg key "$key" '.contracts[$key] // empty' "$CONFIG_FILE_PATH"
}

if [[ -n "$FACTORY_OVERRIDE" ]]; then
  TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS="$FACTORY_OVERRIDE"
elif [[ -z "${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS:-}" ]]; then
  TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS=$(maybe_resolve_from_config "tokenDistributorByHoldingsFactory")
fi

if [[ -n "$BRLA_OVERRIDE" ]]; then
  BRLA_TOKEN_ADDRESS="$BRLA_OVERRIDE"
elif [[ -z "${BRLA_TOKEN_ADDRESS:-}" ]]; then
  BRLA_TOKEN_ADDRESS=$(maybe_resolve_from_config "brlaToken")
fi

if [[ -n "$OFFER_TOKEN_OVERRIDE" ]]; then
  OFFER_TOKEN_ADDRESS="$OFFER_TOKEN_OVERRIDE"
fi

if [[ -z "${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS:-}" ]]; then
  echo "TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS is required." >&2
  exit 1
fi

if [[ -z "${BRLA_TOKEN_ADDRESS:-}" ]]; then
  echo "BRLA_TOKEN_ADDRESS is required." >&2
  exit 1
fi

if [[ -z "${OFFER_TOKEN_ADDRESS:-}" ]]; then
  echo "--offer-token or OFFER_TOKEN_ADDRESS env variable is required." >&2
  exit 1
fi

if [[ -n "$HOLDER_LIST" ]]; then
  IFS=',' read -r -a holder_entries <<<"$HOLDER_LIST"
else
  holder_entries=("${DEFAULT_HOLDER_ALIASES[@]}")
fi

RESOLVED_HOLDERS=()
for entry in "${holder_entries[@]}"; do
  trimmed="${entry//[[:space:]]/}"
  [[ -z "$trimmed" ]] && continue
  if [[ "$trimmed" =~ ^0x[0-9a-fA-F]{40}$ ]]; then
    RESOLVED_HOLDERS+=("$trimmed")
  else
    RESOLVED_HOLDERS+=("$(resolve_address_or_alias "" "$trimmed")")
  fi
done

if [[ ${#RESOLVED_HOLDERS[@]} -eq 0 ]]; then
  echo "No holder addresses resolved" >&2
  exit 1
fi

TOTAL_BRLA_WEI=$(convert_brla_to_wei "$TOTAL_BRLA_STR")

HOLDERS_JSON="["
for addr in "${RESOLVED_HOLDERS[@]}"; do
  if [[ "$HOLDERS_JSON" != "[" ]]; then
    HOLDERS_JSON+=","
  fi
  HOLDERS_JSON+="\"${addr}\""
done
HOLDERS_JSON+="]"

cat <<EOF

==> TokenDistributorByHoldings configuration
Factory:      ${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS}
Offer token:  ${OFFER_TOKEN_ADDRESS}
BRLA token:   ${BRLA_TOKEN_ADDRESS}
Deployer:     ${DEPLOYER_ADDRESS}
Holders:      ${RESOLVED_HOLDERS[*]}
Total BRLA:   ${TOTAL_BRLA_STR} (${TOTAL_BRLA_WEI} wei)
EOF

(
  cd "$PROJECT_ROOT"
  TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS="$TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS" \
  BRLA_TOKEN_ADDRESS="$BRLA_TOKEN_ADDRESS" \
  OFFER_TOKEN_ADDRESS="$OFFER_TOKEN_ADDRESS" \
  DEPLOYER_ADDRESS="$DEPLOYER_ADDRESS" \
  BRLA_DISTRIBUTION_TOTAL="$TOTAL_BRLA_WEI" \
  HOLDERS_JSON="$HOLDERS_JSON" \
  forge script script/CreateAndRunTokenDistributorByHoldings.s.sol:CreateAndRunTokenDistributorByHoldings \
    --rpc-url "$RPC_URL" \
    --chain "$CHAIN_ID" \
    --broadcast \
    --account "$ACCOUNT_ALIAS"
)

printf '\nTokenDistributorByHoldings deployment and distribution complete.\n'
