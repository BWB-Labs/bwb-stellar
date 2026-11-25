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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"
DEFAULT_TOTAL_BRLA="240" # 120 BRLA each for advisor and deployer
ACCOUNT_ALIAS="deployer"

if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  # shellcheck disable=SC1091
  source "${PROJECT_ROOT}/.env"
fi

DEPLOYMENT_ENVIRONMENT="$(printf '%s' "${DEPLOYMENT_ENVIRONMENT:-production}" | tr '[:upper:]' '[:lower:]')"

usage() {
  cat <<'EOF'
Usage: ./shells/create-token-distribution.sh [--total-brla 240] [--advisor 0x...] [--factory 0x...] [--brla 0x...] [--config path]

Creates a TokenDistributor clone via TokenDistributorFactory, funds it with BRLA
from the deployer account, splits the payout equally between deployer and advisor,
and performs the distribution immediately.

Options:
  --total-brla <value>   Total BRLA to distribute (default 240 => 120 each)
  --advisor <address>    Override advisor address (defaults to ADVISOR_ADDRESS env or 'advisor' wallet alias)
  --factory <address>    Override TokenDistributorFactory address
  --brla <address>       Override BRLA token address
  --config <path>        Explicit deployment JSON (defaults to configs/deployment-$DEPLOYMENT_ENVIRONMENT.json)
  --name <text>          Distributor name metadata
  --description <text>   Distributor description metadata
  -h, --help             Show this help message
EOF
}

require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Environment variable ${var_name} is required" >&2
    exit 1
  fi
}

resolve_address_or_alias() {
  local env_key="$1"
  local alias_name="$2"
  local current_value="${!env_key:-}"
  if [[ -n "$current_value" ]]; then
    printf '%s' "$current_value"
    return
  fi
  if derived=$(cast wallet address "$alias_name" 2>/dev/null); then
    printf '%s' "$derived"
    return
  fi
  echo "Unable to resolve ${env_key}. Provide --${alias_name} or set ${env_key}." >&2
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
wei = int(value * (10 ** 18))
print(wei)
PY
}

determine_config_path() {
  if [[ -n "$CONFIG_FILE_ARG" ]]; then
    CONFIG_FILE_PATH="$CONFIG_FILE_ARG"
  elif [[ -n "${DEPLOYMENT_CONFIG_PATH:-}" ]]; then
    CONFIG_FILE_PATH="${DEPLOYMENT_CONFIG_PATH}"
  else
    CONFIG_FILE_PATH="${CONFIGS_DIR}/deployment-${DEPLOYMENT_ENVIRONMENT}.json"
  fi

  if [[ -z "$CONFIG_FILE_PATH" ]]; then
    return
  fi

  if [[ ! -f "$CONFIG_FILE_PATH" ]]; then
    echo "Deployment summary not found at ${CONFIG_FILE_PATH}. Use --config or set DEPLOYMENT_CONFIG_PATH." >&2
    CONFIG_FILE_PATH=""
    return
  fi

  echo "Using deployment summary: ${CONFIG_FILE_PATH}"
}

maybe_resolve_factory_from_config() {
  if [[ -n "${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS:-}" ]]; then
    return
  fi
  if [[ -z "$CONFIG_FILE_PATH" ]]; then
    return
  fi
  local value
  value=$(jq -r '.contracts.tokenDistributorFactory // empty' "$CONFIG_FILE_PATH")
  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "tokenDistributorFactory not found in ${CONFIG_FILE_PATH}." >&2
    return
  fi
  TOKEN_DISTRIBUTOR_FACTORY_ADDRESS="$value"
  export TOKEN_DISTRIBUTOR_FACTORY_ADDRESS
  echo "Resolved TokenDistributorFactory from config: ${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS}"
}

maybe_resolve_brla_from_config() {
  if [[ -n "${BRLA_TOKEN_ADDRESS:-}" ]]; then
    return
  fi
  if [[ -z "$CONFIG_FILE_PATH" ]]; then
    return
  fi
  local value
  value=$(jq -r '.contracts.brlaToken // empty' "$CONFIG_FILE_PATH")
  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "brlaToken not found in ${CONFIG_FILE_PATH}." >&2
    return
  fi
  BRLA_TOKEN_ADDRESS="$value"
  export BRLA_TOKEN_ADDRESS
  echo "Resolved BRLA token from config: ${BRLA_TOKEN_ADDRESS}"
}

TOTAL_BRLA_AMOUNT="$DEFAULT_TOTAL_BRLA"
ADVISOR_OVERRIDE=""
FACTORY_OVERRIDE=""
BRLA_OVERRIDE=""
NAME_OVERRIDE=""
DESCRIPTION_OVERRIDE=""
CONFIG_FILE_ARG=""
CONFIG_FILE_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --total-brla)
    TOTAL_BRLA_AMOUNT="$2"
    shift 2
    ;;
  --advisor)
    ADVISOR_OVERRIDE="$2"
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
  --name)
    NAME_OVERRIDE="$2"
    shift 2
    ;;
  --description)
    DESCRIPTION_OVERRIDE="$2"
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

determine_config_path

if [[ -n "$FACTORY_OVERRIDE" ]]; then
  TOKEN_DISTRIBUTOR_FACTORY_ADDRESS="$FACTORY_OVERRIDE"
fi
if [[ -n "$BRLA_OVERRIDE" ]]; then
  BRLA_TOKEN_ADDRESS="$BRLA_OVERRIDE"
fi
if [[ -n "$ADVISOR_OVERRIDE" ]]; then
  ADVISOR_ADDRESS="$ADVISOR_OVERRIDE"
fi
if [[ -n "$NAME_OVERRIDE" ]]; then
  TOKEN_DISTRIBUTOR_NAME="$NAME_OVERRIDE"
fi
if [[ -n "$DESCRIPTION_OVERRIDE" ]]; then
  TOKEN_DISTRIBUTOR_DESCRIPTION="$DESCRIPTION_OVERRIDE"
fi

maybe_resolve_factory_from_config
maybe_resolve_brla_from_config

TOKEN_DISTRIBUTOR_FACTORY_ADDRESS=$(resolve_address_or_alias "TOKEN_DISTRIBUTOR_FACTORY_ADDRESS" "factory")
BRLA_TOKEN_ADDRESS=$(resolve_address_or_alias "BRLA_TOKEN_ADDRESS" "brla")
ADVISOR_ADDRESS=$(resolve_address_or_alias "ADVISOR_ADDRESS" "advisor")

TOTAL_BRLA_WEI=$(convert_brla_to_wei "$TOTAL_BRLA_AMOUNT")

cat <<EOF

==> Token distribution configuration
Factory:      ${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS}
BRLA token:   ${BRLA_TOKEN_ADDRESS}
Deployer:     ${DEPLOYER_ADDRESS}
Advisor:      ${ADVISOR_ADDRESS}
Total BRLA:   ${TOTAL_BRLA_AMOUNT} (${TOTAL_BRLA_WEI} wei)
Name:         ${TOKEN_DISTRIBUTOR_NAME:-Advisor/Deployer Distribution}
Description:  ${TOKEN_DISTRIBUTOR_DESCRIPTION:-One-off BRLA split between advisor and deployer}
EOF

(
  cd "$PROJECT_ROOT"
  TOKEN_DISTRIBUTOR_FACTORY_ADDRESS="$TOKEN_DISTRIBUTOR_FACTORY_ADDRESS" \
  BRLA_TOKEN_ADDRESS="$BRLA_TOKEN_ADDRESS" \
  DEPLOYER_ADDRESS="$DEPLOYER_ADDRESS" \
  ADVISOR_ADDRESS="$ADVISOR_ADDRESS" \
  BRLA_DISTRIBUTION_TOTAL="$TOTAL_BRLA_WEI" \
  TOKEN_DISTRIBUTOR_NAME="${TOKEN_DISTRIBUTOR_NAME:-Advisor/Deployer Distribution}" \
  TOKEN_DISTRIBUTOR_DESCRIPTION="${TOKEN_DISTRIBUTOR_DESCRIPTION:-One-off BRLA split between advisor and deployer}" \
  forge script script/CreateAndRunTokenDistributor.s.sol:CreateAndRunTokenDistributor \
    --rpc-url "$RPC_URL" \
    --chain "$CHAIN_ID" \
    --broadcast \
    --account "$ACCOUNT_ALIAS"
)

printf '\nBRLA distribution complete.\n'
