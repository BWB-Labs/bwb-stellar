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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"

# Load env vars when available
if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  # shellcheck disable=SC1091
  source "${PROJECT_ROOT}/.env"
fi

BROADCAST_ROOT="${PROJECT_ROOT}/${FOUNDRY_BROADCAST:-broadcast/production}"
DEPLOY_SCRIPT_FILE="${BROADCAST_ROOT}/DeployOfferTokenSale.s.sol"
DEPLOYMENT_ENVIRONMENT="$(printf '%s' "${DEPLOYMENT_ENVIRONMENT:-production}" | tr '[:upper:]' '[:lower:]')"

usage() {
  cat <<'EOF'
Usage: ./shells/create-offer-token-sale.sh --offer-token 0x... --price 1000000000000000000 [options]

Required flags:
  --offer-token    Address of the OfferERC20 token to sell
  --price          BRLA per offer token (18 decimals, integer string)
  --min-raise      Minimum BRLA (wei) required for the sale to be considered successful
  --hard-cap       Maximum BRLA (wei) accepted by the sale

Optional flags:
  --owner                  Override sale owner (defaults to deployer)
  --inventory-provider     Override detected inventory provider (auto-resolves from sale contract)
  --cooling-off-period     Cooling-off window in seconds (defaults to 432000 = 5 days)
  --success-percent        Minimum percentage of tokens that must be sold (defaults to 75)
  --skip-inventory-approval  Skip automatic approval (assumes allowance already granted)
  --factory                Override OfferTokenSaleFactory address
  --brla                   Override BRLA token address
  --allowlist              Override OfferSaleAllowlist address
  --config                 Explicit deployment JSON path (defaults to configs/deployment-$DEPLOYMENT_ENVIRONMENT.json)
  -h, --help               Show this help message

You can also set DEPLOYMENT_CONFIG_PATH to point at a summary file. When provided,
the script loads factory, BRLA, and allowlist addresses from that JSON before
falling back to broadcast data.
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
      echo "Deployment summary not found at ${CONFIG_FILE_PATH}. Provide overrides or re-run full deployment." >&2
      exit 1
    fi
    CONFIG_FILE_PATH=""
    return
  fi

  echo "Using deployment summary: ${CONFIG_FILE_PATH}"
}

resolve_from_config() {
  local config_key="$1"
  local label="$2"

  if [[ -z "$CONFIG_FILE_PATH" ]]; then
    return 1
  fi

  local value
  value=$(jq -r --arg key "$config_key" '.contracts[$key] // empty' "$CONFIG_FILE_PATH")

  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "${label} not found in ${CONFIG_FILE_PATH}." >&2
    return 1
  fi

  printf '%s' "$value"
  return 0
}

resolve_address() {
  local env_key="$1"
  local label="$2"
  local config_key="$3"
  local value="${!env_key:-}"

  if [[ -n "$value" ]]; then
    echo "Using configured ${label}: ${value}"
    export "${env_key}=${value}"
    return
  fi

  if [[ -n "$config_key" ]]; then
    if value=$(resolve_from_config "$config_key" "$label" 2>/dev/null); then
      echo "Resolved ${label} from config: ${value}"
      export "${env_key}=${value}"
      return
    fi
  fi

  echo "Provide ${env_key} for ${label}." >&2
  exit 1
}

determine_sale_owner() {
  if [[ -n "$SALE_OWNER_ARG" ]]; then
    SALE_OWNER_VALUE="$SALE_OWNER_ARG"
  elif [[ -n "${SALE_OWNER:-}" ]]; then
    SALE_OWNER_VALUE="$SALE_OWNER"
  elif [[ -n "${DEPLOYER_ADDRESS:-}" ]]; then
    SALE_OWNER_VALUE="$DEPLOYER_ADDRESS"
    echo "Defaulting sale owner to DEPLOYER_ADDRESS: ${SALE_OWNER_VALUE}"
  else
    local derived
    if derived=$(cast wallet address "$ACCOUNT_ALIAS" 2>/dev/null); then
      SALE_OWNER_VALUE="$derived"
      echo "Defaulting sale owner to signer alias (${ACCOUNT_ALIAS}): ${SALE_OWNER_VALUE}"
    else
      SALE_OWNER_VALUE=""
    fi
  fi

  if [[ -z "$SALE_OWNER_VALUE" ]]; then
    echo "Unable to determine sale owner. Provide --owner or set SALE_OWNER/DEPLOYER_ADDRESS." >&2
    exit 1
  fi

  export SALE_OWNER="$SALE_OWNER_VALUE"
}

determine_inventory_provider() {
  local provider_value="${INVENTORY_PROVIDER_ADDRESS:-}"

  if [[ -n "$INVENTORY_PROVIDER_ARG" ]]; then
    provider_value="$INVENTORY_PROVIDER_ARG"
    echo "Using provided inventory provider: ${provider_value}"
  elif [[ -n "$provider_value" ]]; then
    echo "Using INVENTORY_PROVIDER_ADDRESS env: ${provider_value}"
  else
    if [[ -z "${SALE_ADDRESS:-}" ]]; then
      echo "OfferTokenSale address required to detect inventory provider. Provide --inventory-provider." >&2
      exit 1
    fi

    local detected=""
    if detected=$(cast call "$SALE_ADDRESS" "inventoryProvider()(address)" --rpc-url "$RPC_URL" 2>/dev/null); then
      if [[ -n "$detected" && "$detected" != "0x0000000000000000000000000000000000000000" ]]; then
        provider_value="$detected"
        echo "Detected inventory provider from OfferTokenSale.inventoryProvider(): ${provider_value}"
      fi
    fi

    if [[ -z "$provider_value" && -n "${SALE_OWNER:-}" ]]; then
      provider_value="$SALE_OWNER"
      echo "Falling back to sale owner as inventory provider: ${provider_value}"
    fi

    if [[ -z "$provider_value" ]]; then
      echo "Unable to detect inventory provider from sale at ${SALE_ADDRESS}. Provide --inventory-provider." >&2
      exit 1
    fi
  fi

  INVENTORY_PROVIDER_VALUE="$provider_value"
  export INVENTORY_PROVIDER_ADDRESS="$INVENTORY_PROVIDER_VALUE"
}

determine_inventory_signer() {
  if [[ "$SKIP_INVENTORY_APPROVAL" == "true" ]]; then
    INVENTORY_SIGNER_ALIAS=""
    return
  fi

  INVENTORY_SIGNER_ALIAS="$ACCOUNT_ALIAS"

  if [[ -z "$INVENTORY_PROVIDER_ADDRESS" ]]; then
    echo "Inventory provider must be resolved before selecting a signer." >&2
    exit 1
  fi

  if [[ -z "${DEPLOYER_ADDRESS:-}" ]]; then
    echo "DEPLOYER_ADDRESS must be set to validate inventory approvals." >&2
    exit 1
  fi

  local deployer_lower="${DEPLOYER_ADDRESS,,}"
  local provider_lower="${INVENTORY_PROVIDER_ADDRESS,,}"
  if [[ "$deployer_lower" != "$provider_lower" ]]; then
    echo "Deployer address (${DEPLOYER_ADDRESS}) must match inventory provider ${INVENTORY_PROVIDER_ADDRESS}. Update the sale configuration or pass --skip-inventory-approval after approving manually." >&2
    exit 1
  fi
}

extract_sale_address() {
  local broadcast_file="${DEPLOY_SCRIPT_FILE}/${CHAIN_ID}/run-latest.json"

  if [[ ! -f "$broadcast_file" ]]; then
    echo "Broadcast file not found (expected ${broadcast_file})." >&2
    exit 1
  fi

  local sale_address
  sale_address=$(jq -r '.returns.sale.value // empty' "$broadcast_file")

  if [[ -z "$sale_address" || "$sale_address" == "null" ]]; then
    echo "Unable to read sale address from ${broadcast_file}." >&2
    exit 1
  fi

  printf '%s' "$sale_address"
}

log_step() {
  printf '\n==> %s\n' "$1"
}

OFFER_TOKEN_ARG=""
PRICE_ARG=""
HARD_CAP_ARG=""
COOLING_OFF_ARG=""
SUCCESS_PERCENT_ARG=""
SALE_OWNER_ARG=""
FACTORY_OVERRIDE_ARG=""
ALLOWLIST_OVERRIDE_ARG=""
BRLA_OVERRIDE_ARG=""
INVENTORY_PROVIDER_ARG=""
CONFIG_FILE_ARG=""
CONFIG_FILE_PATH=""
readonly ACCOUNT_ALIAS="deployer"
SKIP_INVENTORY_APPROVAL="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
  --offer-token)
    OFFER_TOKEN_ARG="$2"
    shift 2
    ;;
  --price)
    PRICE_ARG="$2"
    shift 2
    ;;
  --hard-cap)
    HARD_CAP_ARG="$2"
    shift 2
    ;;
  --cooling-off-period)
    COOLING_OFF_ARG="$2"
    shift 2
    ;;
  --success-percent)
    SUCCESS_PERCENT_ARG="$2"
    shift 2
    ;;
  --owner)
    SALE_OWNER_ARG="$2"
    shift 2
    ;;
  --factory)
    FACTORY_OVERRIDE_ARG="$2"
    shift 2
    ;;
  --allowlist)
    ALLOWLIST_OVERRIDE_ARG="$2"
    shift 2
    ;;
  --brla)
    BRLA_OVERRIDE_ARG="$2"
    shift 2
    ;;
  --config)
    CONFIG_FILE_ARG="$2"
    shift 2
    ;;
  --inventory-provider)
    INVENTORY_PROVIDER_ARG="$2"
    shift 2
    ;;
  --skip-inventory-approval)
    SKIP_INVENTORY_APPROVAL="true"
    shift 1
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

if [[ -n "$OFFER_TOKEN_ARG" ]]; then
  export OFFER_TOKEN_ADDRESS="$OFFER_TOKEN_ARG"
fi

if [[ -n "$FACTORY_OVERRIDE_ARG" ]]; then
  export TOKEN_SALE_FACTORY_ADDRESS="$FACTORY_OVERRIDE_ARG"
fi

if [[ -n "$ALLOWLIST_OVERRIDE_ARG" ]]; then
  export ALLOWLIST_ADDRESS="$ALLOWLIST_OVERRIDE_ARG"
fi

if [[ -n "$BRLA_OVERRIDE_ARG" ]]; then
  export BRLA_TOKEN_ADDRESS="$BRLA_OVERRIDE_ARG"
fi

if [[ -n "$SALE_OWNER_ARG" ]]; then
  export SALE_OWNER="$SALE_OWNER_ARG"
fi

if [[ -n "$HARD_CAP_ARG" ]]; then
  export SALE_HARD_CAP="$HARD_CAP_ARG"
fi

if [[ -n "$COOLING_OFF_ARG" ]]; then
  export SALE_COOLING_OFF_PERIOD="$COOLING_OFF_ARG"
fi

if [[ -n "$SUCCESS_PERCENT_ARG" ]]; then
  export SALE_MIN_SUCCESS_PERCENT="$SUCCESS_PERCENT_ARG"
fi

BRLA_PER_TOKEN_VALUE="${PRICE_ARG:-${BRLA_PER_TOKEN:-}}"
OFFER_TOKEN_VALUE="${OFFER_TOKEN_ARG:-${OFFER_TOKEN_ADDRESS:-}}"

if [[ -z "$OFFER_TOKEN_VALUE" ]]; then
  echo "--offer-token or OFFER_TOKEN_ADDRESS env variable is required" >&2
  exit 1
fi

if [[ -z "$BRLA_PER_TOKEN_VALUE" ]]; then
  echo "--price or BRLA_PER_TOKEN env variable is required" >&2
  exit 1
fi

if [[ ! "$BRLA_PER_TOKEN_VALUE" =~ ^[0-9]+$ ]]; then
  echo "BRLA price must be a numeric string (wei)" >&2
  exit 1
fi

require_env "RPC_URL"
require_env "CHAIN_ID"
require_env "SALE_HARD_CAP"

export SALE_HARD_CAP

SALE_COOLING_OFF_PERIOD="${SALE_COOLING_OFF_PERIOD:-432000}"
SALE_MIN_SUCCESS_PERCENT="${SALE_MIN_SUCCESS_PERCENT:-75}"
export SALE_COOLING_OFF_PERIOD
export SALE_MIN_SUCCESS_PERCENT

NUMERIC_PATTERN='^[0-9]+$'

if [[ ! "${SALE_HARD_CAP}" =~ ${NUMERIC_PATTERN} ]]; then
  echo "SALE_HARD_CAP must be a numeric string (wei)" >&2
  exit 1
fi

if [[ ! "${SALE_COOLING_OFF_PERIOD}" =~ ${NUMERIC_PATTERN} ]]; then
  echo "SALE_COOLING_OFF_PERIOD must be provided in seconds" >&2
  exit 1
fi

if [[ ! "${SALE_MIN_SUCCESS_PERCENT}" =~ ${NUMERIC_PATTERN} ]]; then
  echo "SALE_MIN_SUCCESS_PERCENT must be numeric" >&2
  exit 1
fi

if (( SALE_COOLING_OFF_PERIOD == 0 )); then
  echo "SALE_COOLING_OFF_PERIOD must be greater than zero" >&2
  exit 1
fi

if (( SALE_MIN_SUCCESS_PERCENT > 100 )); then
  echo "SALE_MIN_SUCCESS_PERCENT must be between 0 and 100" >&2
  exit 1
fi

determine_deployment_config_path

resolve_address "TOKEN_SALE_FACTORY_ADDRESS" "OfferTokenSaleFactory" "offerTokenSaleFactory"
resolve_address "ALLOWLIST_ADDRESS" "OfferSaleAllowlist" "offerSaleAllowlist"
resolve_address "BRLA_TOKEN_ADDRESS" "BRLA token" "brlaToken"

determine_sale_owner

log_step "Preparing OfferTokenSale deployment"
echo "Offer token:   ${OFFER_TOKEN_VALUE}"
echo "BRLA token:    ${BRLA_TOKEN_ADDRESS}"
echo "Allowlist:     ${ALLOWLIST_ADDRESS}"
echo "Price (wei):   ${BRLA_PER_TOKEN_VALUE}"
echo "Hard cap:      ${SALE_HARD_CAP}"
echo "Cooling-off:   ${SALE_COOLING_OFF_PERIOD} seconds"
echo "Success pct:   ${SALE_MIN_SUCCESS_PERCENT}%"
echo "Sale owner:    ${SALE_OWNER}"
echo "Inventory provider: resolving after deployment"
if [[ "$SKIP_INVENTORY_APPROVAL" == "true" ]]; then
  echo "Inventory approval: skipped (ensure allowance exists)"
else
  echo "Inventory approval signer: ${ACCOUNT_ALIAS}"
fi
echo "Factory:       ${TOKEN_SALE_FACTORY_ADDRESS}"
echo "Signer alias:  ${ACCOUNT_ALIAS}"

export OFFER_TOKEN_ADDRESS="$OFFER_TOKEN_VALUE"
export BRLA_PER_TOKEN="$BRLA_PER_TOKEN_VALUE"

log_step "Running DeployOfferTokenSale script"
(
  cd "$PROJECT_ROOT"
  forge script script/DeployOfferTokenSale.s.sol:DeployOfferTokenSale \
    --rpc-url "$RPC_URL" \
    --chain "$CHAIN_ID" \
    --broadcast \
    --account "$ACCOUNT_ALIAS"
)

SALE_ADDRESS=$(extract_sale_address)
export OFFER_TOKEN_SALE_ADDRESS="$SALE_ADDRESS"

log_step "Granting allowlist controller access"
(
  cd "$PROJECT_ROOT"
  forge script script/SetOfferSaleAllowlistController.s.sol:SetOfferSaleAllowlistController \
    --rpc-url "$RPC_URL" \
    --chain "$CHAIN_ID" \
    --broadcast \
    --account "$ACCOUNT_ALIAS"
)

determine_inventory_provider
determine_inventory_signer

echo "\nSale deployed at: ${SALE_ADDRESS}"
echo "Inventory provider: ${INVENTORY_PROVIDER_ADDRESS}"
if [[ "$SKIP_INVENTORY_APPROVAL" != "true" ]]; then
  echo "Inventory signer: ${INVENTORY_SIGNER_ALIAS}"
fi

log_step "Preparing inventory approval"
TOTAL_SUPPLY=$(cast call "$OFFER_TOKEN_ADDRESS" "totalSupply()(uint256)" --rpc-url "$RPC_URL")
echo "Total supply detected: ${TOTAL_SUPPLY}"
if [[ "$SKIP_INVENTORY_APPROVAL" == "true" ]]; then
  echo "Skipping automatic approval. Ensure ${INVENTORY_PROVIDER_ADDRESS} has approved ${SALE_ADDRESS}."
else
  (
    cd "$PROJECT_ROOT"
    forge script script/ApproveOfferTokenInventory.s.sol:ApproveOfferTokenInventory \
      --rpc-url "$RPC_URL" \
      --chain "$CHAIN_ID" \
      --broadcast \
      --account "$INVENTORY_SIGNER_ALIAS"
  )
fi

log_step "Seeding OfferTokenSale inventory"
(
  cd "$PROJECT_ROOT"
  forge script script/SeedOfferTokenSale.s.sol:SeedOfferTokenSale \
    --rpc-url "$RPC_URL" \
    --chain "$CHAIN_ID" \
    --broadcast \
    --account "$ACCOUNT_ALIAS"
)

echo "\nOfferTokenSale ready at ${SALE_ADDRESS}"
