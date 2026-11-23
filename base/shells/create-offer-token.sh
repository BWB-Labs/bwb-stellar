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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"

# Load environment variables when available
if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  # shellcheck disable=SC1091
  source "${PROJECT_ROOT}/.env"
fi

BROADCAST_ROOT="${PROJECT_ROOT}/${FOUNDRY_BROADCAST:-broadcast/production}"
DEPLOYMENT_ENVIRONMENT="$(printf '%s' "${DEPLOYMENT_ENVIRONMENT:-production}" | tr '[:upper:]' '[:lower:]')"

usage() {
  cat <<'EOF'
Usage: ./shells/create-offer-token.sh --name "Tower One" --symbol TOWER --supply 120 --offer-uri https://example.com/offers/tower.json [options]

Required flags:
  --name        Token name (string)
  --symbol      Token symbol (string)
  --supply      Initial supply using 18 decimals (integer string)
  --offer-uri   Offer metadata URI

Optional flags:
  --owner       Address receiving the initial supply (defaults to deployer)
  --factory     Override OfferTokenFactory address (otherwise reuse last deployment)
  --config      Explicit path to deployment JSON (defaults to configs/deployment-$DEPLOYMENT_ENVIRONMENT.json)
  -h, --help    Show this message and exit

You can also set DEPLOYMENT_CONFIG_PATH to point at a summary file. When present,
the OfferTokenFactory address is loaded from that JSON before falling back to
broadcast files. Run inside the nix-shell defined at contracts/shell.nix so forge
and cast are available.
EOF
}

require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Environment variable ${var_name} is required" >&2
    exit 1
  fi
}

determine_deployment_config_path() {
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
    if [[ -n "$CONFIG_FILE_ARG" || -n "${DEPLOYMENT_CONFIG_PATH:-}" ]]; then
      echo "Deployment summary not found at ${CONFIG_FILE_PATH}. Run shells/full-deploy.sh or provide --factory." >&2
      exit 1
    fi
    CONFIG_FILE_PATH=""
    return
  fi

  echo "Using deployment summary: ${CONFIG_FILE_PATH}"
}

read_broadcast_value() {
  local script_file="$1"
  local return_key="$2"
  local label="$3"

  if [[ -z "${CHAIN_ID:-}" ]]; then
    echo "CHAIN_ID must be set to read ${label} broadcast data" >&2
    exit 1
  fi

  local broadcast_file="${BROADCAST_ROOT}/${script_file}/${CHAIN_ID}/run-latest.json"

  if [[ ! -f "$broadcast_file" ]]; then
    echo "Broadcast file not found for ${label} (expected ${broadcast_file}). Provide the address manually." >&2
    exit 1
  fi

  local value
  value=$(jq -r --arg key "$return_key" '.returns[$key].value // empty' "$broadcast_file")

  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "Unable to read ${return_key} for ${label} from ${broadcast_file}" >&2
    exit 1
  fi

  printf '%s' "$value"
}

resolve_contract_address() {
  local env_key="$1"
  local script_file="$2"
  local return_key="$3"
  local label="$4"
  local config_key="${5:-}"

  local current_value="${!env_key:-}"
  if [[ -n "$current_value" ]]; then
    echo "Using configured ${label}: ${current_value}"
    export "${env_key}=${current_value}"
    return
  fi

  if [[ -n "$config_key" && -n "$CONFIG_FILE_PATH" ]]; then
    local config_value
    config_value=$(jq -r --arg key "$config_key" '.contracts[$key] // empty' "$CONFIG_FILE_PATH")

    if [[ -z "$config_value" || "$config_value" == "null" ]]; then
      echo "${label} not found in ${CONFIG_FILE_PATH}. Provide --factory or regenerate the summary." >&2
      exit 1
    fi

    export "${env_key}=${config_value}"
    echo "Resolved ${label} from ${CONFIG_FILE_PATH}: ${config_value}"
    return
  fi

  local broadcast_value
  if broadcast_value=$(read_broadcast_value "$script_file" "$return_key" "$label" 2>/dev/null); then
    export "${env_key}=${broadcast_value}"
    echo "Resolved ${label} from broadcast: ${broadcast_value}"
    return
  fi

  echo "Provide ${env_key} in the environment to continue." >&2
  exit 1
}

log_step() {
  printf '\n==> %s\n' "$1"
}

TOKEN_NAME_ARG=""
TOKEN_SYMBOL_ARG=""
INITIAL_SUPPLY_ARG=""
OFFER_URI_ARG=""
TOKEN_OWNER_ARG=""
FACTORY_OVERRIDE_ARG=""
CONFIG_FILE_ARG=""
CONFIG_FILE_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
  --name)
    TOKEN_NAME_ARG="$2"
    shift 2
    ;;
  --symbol)
    TOKEN_SYMBOL_ARG="$2"
    shift 2
    ;;
  --supply)
    INITIAL_SUPPLY_ARG="$2"
    shift 2
    ;;
  --offer-uri)
    OFFER_URI_ARG="$2"
    shift 2
    ;;
  --owner)
    TOKEN_OWNER_ARG="$2"
    shift 2
    ;;
  --factory)
    FACTORY_OVERRIDE_ARG="$2"
    shift 2
    ;;
  --config)
    CONFIG_FILE_ARG="$2"
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

TOKEN_NAME_VALUE="${TOKEN_NAME_ARG:-${TOKEN_NAME:-}}"
TOKEN_SYMBOL_VALUE="${TOKEN_SYMBOL_ARG:-${TOKEN_SYMBOL:-}}"
INITIAL_SUPPLY_VALUE="${INITIAL_SUPPLY_ARG:-${INITIAL_SUPPLY:-}}"
OFFER_URI_VALUE="${OFFER_URI_ARG:-${OFFER_URI:-}}"
TOKEN_OWNER_VALUE="${TOKEN_OWNER_ARG:-${TOKEN_OWNER:-}}"

if [[ -z "$TOKEN_NAME_VALUE" ]]; then
  echo "--name or TOKEN_NAME env variable is required" >&2
  exit 1
fi

if [[ -z "$TOKEN_SYMBOL_VALUE" ]]; then
  echo "--symbol or TOKEN_SYMBOL env variable is required" >&2
  exit 1
fi

if [[ -z "$INITIAL_SUPPLY_VALUE" ]]; then
  echo "--supply or INITIAL_SUPPLY env variable is required" >&2
  exit 1
fi

if [[ ! "$INITIAL_SUPPLY_VALUE" =~ ^[0-9]+$ ]]; then
  echo "Initial supply must be a numeric string representing 18-decimal units" >&2
  exit 1
fi

if [[ -z "$OFFER_URI_VALUE" ]]; then
  echo "--offer-uri or OFFER_URI env variable is required" >&2
  exit 1
fi

if [[ -n "$FACTORY_OVERRIDE_ARG" ]]; then
  export OFFER_TOKEN_FACTORY_ADDRESS="$FACTORY_OVERRIDE_ARG"
fi

require_env "RPC_URL"
require_env "CHAIN_ID"

determine_deployment_config_path

resolve_contract_address "OFFER_TOKEN_FACTORY_ADDRESS" "DeployOfferTokenFactory.s.sol" "factory" "OfferTokenFactory" "offerTokenFactory"

log_step "Preparing OfferERC20 deployment"
echo "Token name:    ${TOKEN_NAME_VALUE}"
echo "Token symbol:  ${TOKEN_SYMBOL_VALUE}"
echo "Initial supply: ${INITIAL_SUPPLY_VALUE}"
echo "Offer URI:     ${OFFER_URI_VALUE}"
echo "Owner:         ${TOKEN_OWNER_VALUE:-(deployer fallback)}"
echo "Factory:       ${OFFER_TOKEN_FACTORY_ADDRESS}"

export FACTORY_ADDRESS="${OFFER_TOKEN_FACTORY_ADDRESS}"
export TOKEN_NAME="$TOKEN_NAME_VALUE"
export TOKEN_SYMBOL="$TOKEN_SYMBOL_VALUE"
export INITIAL_SUPPLY="$INITIAL_SUPPLY_VALUE"
export OFFER_URI="$OFFER_URI_VALUE"

if [[ -n "$TOKEN_OWNER_VALUE" ]]; then
  export TOKEN_OWNER="$TOKEN_OWNER_VALUE"
else
  unset TOKEN_OWNER || true
fi

log_step "Running DeployOfferERC20 script"
(
  cd "$PROJECT_ROOT"
  forge script script/DeployOfferERC20.s.sol:DeployOfferERC20 \
    --rpc-url "$RPC_URL" \
    --chain "$CHAIN_ID" \
    --broadcast \
    --account deployer
)

log_step "OfferERC20 deployment complete"
echo "Factory used:   ${OFFER_TOKEN_FACTORY_ADDRESS}"
echo "Token details:  ${TOKEN_NAME_VALUE} (${TOKEN_SYMBOL_VALUE})"
echo "Offer URI:      ${OFFER_URI_VALUE}"
