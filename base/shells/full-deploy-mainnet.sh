#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "The 'jq' CLI is required to run this script" >&2
  exit 1
fi

# Usage: ./full-deploy-mainnet.sh
# Deploys production contracts without seeding investor accounts or deploying mocks.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"

# Load environment variables before running deployments
source "${PROJECT_ROOT}/.env"

BROADCAST_ROOT="${PROJECT_ROOT}/${FOUNDRY_BROADCAST:-broadcast/production}"
DEPLOYMENT_ENVIRONMENT="$(printf '%s' "${DEPLOYMENT_ENVIRONMENT:-production}" | tr '[:upper:]' '[:lower:]')"

# Location of Foundry keystore entries; allow override via FOUNDRY_ETH_KEYSTORE
KEYSTORE_DIR="${FOUNDRY_ETH_KEYSTORE:-$HOME/.foundry/keystores}"
REQUIRED_ACCOUNTS=(
  "deployer"
)

require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Environment variable ${var_name} is required" >&2
    exit 1
  fi
}

log_step() {
  printf '\n==> %s\n' "$1"
}

run_context() {
  local target="$1"
  (cd "$PROJECT_ROOT" && forge script --quiet --slow "$target" \
    --rpc-url "${RPC_URL}" \
    --chain "${CHAIN_ID}" \
    --broadcast \
    --account deployer)
}

run_script_and_capture() {
  local script_file="$1"
  local contract_name="$2"
  local return_key="$3"
  local env_target="$4"

  log_step "Running ${contract_name}"
  run_context "script/${script_file}:${contract_name}"
  extract_broadcast_return "$script_file" "$return_key" "$env_target"
}

read_optional_broadcast_value() {
  local script_file="$1"
  local return_key="$2"
  local broadcast_file="${BROADCAST_ROOT}/${script_file}/${CHAIN_ID}/run-latest.json"

  if [[ ! -f "$broadcast_file" ]]; then
    return 1
  fi

  local value
  value=$(jq -r --arg key "$return_key" '.returns[$key].value // empty' "$broadcast_file")

  if [[ -z "$value" || "$value" == "null" ]]; then
    return 1
  fi

  printf '%s' "$value"
}

extract_broadcast_return() {
  local script_file="$1"
  local return_key="$2"
  local env_target="$3"
  local broadcast_file="${BROADCAST_ROOT}/${script_file}/${CHAIN_ID}/run-latest.json"

  if [[ ! -f "$broadcast_file" ]]; then
    echo "Broadcast file not found for ${script_file} (expected ${broadcast_file})" >&2
    exit 1
  fi

  local value
  value=$(jq -r --arg key "$return_key" '.returns[$key].value // empty' "$broadcast_file")

  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "Unable to read ${return_key} from ${broadcast_file}" >&2
    exit 1
  fi

  export "${env_target}=${value}"
  echo "${env_target}=${value}"
}

write_deployment_summary() {
  if [[ "$DEPLOYMENT_ENVIRONMENT" == "production" && "$CHAIN_ID" != "8453" ]]; then
    echo "Refusing to write the production deployment record: DEPLOYMENT_ENVIRONMENT=production but CHAIN_ID=${CHAIN_ID} (expected 8453). Set DEPLOYMENT_ENVIRONMENT to match the target chain." >&2
    exit 1
  fi

  local timestamp summary_file
  timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  summary_file="${CONFIGS_DIR}/deployment-${DEPLOYMENT_ENVIRONMENT}.json"

  mkdir -p "$CONFIGS_DIR"

  jq -n \
    --arg environment "$DEPLOYMENT_ENVIRONMENT" \
    --arg chainId "$CHAIN_ID" \
    --arg generatedAt "$timestamp" \
    --arg offerTokenSaleFactory "$OFFER_TOKEN_SALE_FACTORY_ADDRESS" \
    --arg offerTokenFactory "$OFFER_TOKEN_FACTORY_ADDRESS" \
    --arg tokenDistributorFactory "$TOKEN_DISTRIBUTOR_FACTORY_ADDRESS" \
    --arg tokenDistributorByHoldingsFactory "$TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS" \
    --arg offerSaleAllowlist "$ALLOWLIST_ADDRESS" \
    --arg brlaToken "$BRLA_ADDRESS" \
    '{
      environment: $environment,
      chainId: $chainId,
      generatedAt: $generatedAt,
      contracts: {
        offerTokenSaleFactory: $offerTokenSaleFactory,
        offerTokenFactory: $offerTokenFactory,
        tokenDistributorFactory: $tokenDistributorFactory,
        tokenDistributorByHoldingsFactory: $tokenDistributorByHoldingsFactory,
        offerSaleAllowlist: $offerSaleAllowlist,
        brlaToken: $brlaToken
      }
    }' >"$summary_file"

  chmod 644 "$summary_file"
  echo "Deployment summary written to $summary_file"
}

use_existing_or_deploy() {
  local script_file="$1"
  local contract_name="$2"
  local return_key="$3"
  local env_target="$4"
  local label="$5"
  local existing_value="${!env_target:-}"

  if [[ -n "$existing_value" ]]; then
    echo "Using configured ${label}: ${existing_value}"
    return
  fi

  local broadcast_value
  if broadcast_value=$(read_optional_broadcast_value "$script_file" "$return_key"); then
    export "${env_target}=${broadcast_value}"
    echo "Reusing previously deployed ${label}: ${broadcast_value}"
    return
  fi

  run_script_and_capture "$script_file" "$contract_name" "$return_key" "$env_target"
}

missing_accounts=()

for account in "${REQUIRED_ACCOUNTS[@]}"; do
  keystore_file_plain="${KEYSTORE_DIR}/${account}"
  keystore_file_json="${KEYSTORE_DIR}/${account}.json"

  if [[ ! -f "${keystore_file_plain}" && ! -f "${keystore_file_json}" ]]; then
    missing_accounts+=("${account}")
  fi

done

if [[ ${#missing_accounts[@]} -gt 0 ]]; then
  echo "Missing required Foundry keystore entries: ${missing_accounts[*]}" >&2
  echo "Import them with: cast wallet import --private-key <key> --name <alias>" >&2
  exit 1
fi

echo "All required accounts are available in ${KEYSTORE_DIR}."

require_env "RPC_URL"
require_env "CHAIN_ID"
require_env "SAFE_ADDRESS"
require_env "BRLA_ADDRESS"

use_existing_or_deploy "DeployOfferERC20Implementation.s.sol" "DeployOfferERC20Implementation" "implementation" "OFFER_ERC20_IMPLEMENTATION_ADDRESS" "OfferERC20 implementation"
use_existing_or_deploy "DeployOfferTokenSaleImplementation.s.sol" "DeployOfferTokenSaleImplementation" "implementation" "OFFER_TOKEN_SALE_IMPLEMENTATION_ADDRESS" "OfferTokenSale implementation"
use_existing_or_deploy "DeployTokenDistributorImplementation.s.sol" "DeployTokenDistributorImplementation" "implementation" "TOKEN_DISTRIBUTOR_IMPLEMENTATION_ADDRESS" "TokenDistributor implementation"
use_existing_or_deploy "DeployTokenDistributorByHoldingsImplementation.s.sol" "DeployTokenDistributorByHoldingsImplementation" "implementation" "TOKEN_DISTRIBUTOR_BY_HOLDINGS_IMPLEMENTATION_ADDRESS" "TokenDistributorByHoldings implementation"
use_existing_or_deploy "DeployOfferSaleAllowlistImplementation.s.sol" "DeployOfferSaleAllowlistImplementation" "implementation" "ALLOWLIST_IMPLEMENTATION_ADDRESS" "OfferSaleAllowlist implementation"

use_existing_or_deploy "DeployOfferSaleAllowlist.s.sol" "DeployOfferSaleAllowlist" "allowlist" "ALLOWLIST_ADDRESS" "OfferSaleAllowlist"
use_existing_or_deploy "DeployOfferTokenSaleFactory.s.sol" "DeployOfferTokenSaleFactory" "factory" "OFFER_TOKEN_SALE_FACTORY_ADDRESS" "OfferTokenSaleFactory"
use_existing_or_deploy "DeployOfferTokenFactory.s.sol" "DeployOfferTokenFactory" "factory" "OFFER_TOKEN_FACTORY_ADDRESS" "OfferTokenFactory"
use_existing_or_deploy "DeployTokenDistributorFactory.s.sol" "DeployTokenDistributorFactory" "factory" "TOKEN_DISTRIBUTOR_FACTORY_ADDRESS" "TokenDistributorFactory"
export TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_OWNER="${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_OWNER:-$SAFE_ADDRESS}"
use_existing_or_deploy "DeployTokenDistributorByHoldingsFactory.s.sol" "DeployTokenDistributorByHoldingsFactory" "factory" "TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS" "TokenDistributorByHoldingsFactory"

log_step "Writing deployment summary JSON"
write_deployment_summary
