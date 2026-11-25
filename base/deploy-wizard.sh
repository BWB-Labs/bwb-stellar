#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
Usage: ./deploy-wizard.sh <environment> [--simulate]

Deploys the contract set for one environment, loading contracts/.env.<environment>
and keying every broadcast record to broadcast/<environment>.

  --simulate   Run every step against the RPC without broadcasting a
               transaction. Writes no deployment record and no summary file.
USAGE
  exit 1
}

ENVIRONMENT=""
SIMULATE="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --simulate)
      SIMULATE="true"
      ;;
    -*)
      echo "Error: unknown option: $1" >&2
      usage
      ;;
    *)
      if [[ -n "$ENVIRONMENT" ]]; then
        echo "Error: unexpected argument: $1" >&2
        usage
      fi
      ENVIRONMENT="$1"
      ;;
  esac
  shift
done

if [[ -z "$ENVIRONMENT" ]]; then
  echo "Error: an environment name is required" >&2
  usage
fi

ENVIRONMENT="$(printf '%s' "$ENVIRONMENT" | tr '[:upper:]' '[:lower:]')"

if [[ ! "$ENVIRONMENT" =~ ^[a-z0-9][a-z0-9-]*$ ]]; then
  echo "Error: invalid environment name: ${ENVIRONMENT}" >&2
  echo "Environment names hold lower-case letters, digits and hyphens only." >&2
  exit 1
fi

is_simulation() {
  [[ "$SIMULATE" == "true" ]]
}

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: 'jq' CLI is required" >&2
  exit 1
fi

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"
BASE_MAINNET_CHAIN_ID="8453"
BASE_SEPOLIA_CHAIN_ID="84532"
PRODUCTION_ENVIRONMENT="production"

ENV_FILE="${PROJECT_ROOT}/.env.${ENVIRONMENT}"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "Error: ${ENV_FILE} not found" >&2
  echo "Copy .env.staging.example to .env.${ENVIRONMENT} and configure it first." >&2
  exit 1
fi

set -a
# shellcheck disable=SC1091
source "$ENV_FILE"
set +a

export FOUNDRY_BROADCAST="broadcast/${ENVIRONMENT}"
BROADCAST_ROOT="${PROJECT_ROOT}/${FOUNDRY_BROADCAST}"

# -----------------------------------------------------------------------------
# Validation
# -----------------------------------------------------------------------------
require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Error: ${var_name} is required in ${ENV_FILE}" >&2
    exit 1
  fi
}

require_env "RPC_URL"
require_env "CHAIN_ID"
require_env "DEPLOYER_ADDRESS"
require_env "BRLA_ADDRESS"
require_env "CONTROLLER_ADDRESS"

if ! is_simulation; then
  require_env "ETHERSCAN_API_KEY"
fi

if [[ "$ENVIRONMENT" == "$PRODUCTION_ENVIRONMENT" && "$CHAIN_ID" != "$BASE_MAINNET_CHAIN_ID" ]]; then
  echo "Refusing to run the ${PRODUCTION_ENVIRONMENT} environment against CHAIN_ID=${CHAIN_ID} (expected ${BASE_MAINNET_CHAIN_ID})." >&2
  echo "Pass the environment name that matches the target chain." >&2
  exit 1
fi

if ! is_simulation; then
  KEYSTORE_DIR="${FOUNDRY_ETH_KEYSTORE:-$HOME/.foundry/keystores}"
  if [[ ! -f "${KEYSTORE_DIR}/deployer" && ! -f "${KEYSTORE_DIR}/deployer.json" ]]; then
    echo "Error: Deployer keystore not found at ${KEYSTORE_DIR}/deployer" >&2
    echo "Import it with: cast wallet import deployer --interactive" >&2
    exit 1
  fi
fi

VERIFIER="${VERIFIER:-etherscan}"
VERIFICATION_RETRIES="2"

FORGE_ARGS=(--rpc-url "$RPC_URL" --chain "$CHAIN_ID")

if is_simulation; then
  FORGE_ARGS+=(--sender "$DEPLOYER_ADDRESS")
  RUN_ARGS=("${FORGE_ARGS[@]}")
else
  FORGE_ARGS+=(--account deployer)
  RUN_ARGS=("${FORGE_ARGS[@]}" --broadcast)
fi

# -----------------------------------------------------------------------------
# Helper Functions
# -----------------------------------------------------------------------------
log_step() {
  printf '\n==> %s\n' "$1"
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
    echo "Error: Broadcast file not found: ${broadcast_file}" >&2
    exit 1
  fi

  local value
  value=$(jq -r --arg key "$return_key" '.returns[$key].value // empty' "$broadcast_file")

  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "Error: Unable to read ${return_key} from ${broadcast_file}" >&2
    exit 1
  fi

  export "${env_target}=${value}"
  echo "${env_target}=${value}"
}

add_estimated_cost() {
  local amount="${1%% *}"

  if [[ ! "$amount" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
    ESTIMATE_COMPLETE="false"
    return
  fi

  TOTAL_ESTIMATED_ETH="$(awk -v total="$TOTAL_ESTIMATED_ETH" -v amount="$amount" \
    'BEGIN { printf "%.9f", total + amount }')"
}

report_estimated_cost() {
  if [[ ${#CONTRACTS_TO_DEPLOY[@]} -eq 0 ]]; then
    echo "Estimated cost:      nothing to deploy (the controller step's gas is not estimated)"
    return
  fi

  if [[ "$ESTIMATE_COMPLETE" != "true" || ! "$DEPLOYER_BALANCE_ETH" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
    echo "Estimated cost:      unknown — at least one figure could not be read."
    echo "                     Check the deployer balance yourself before proceeding."
    return
  fi

  echo "Estimated cost:      ${TOTAL_ESTIMATED_ETH} ETH (deploys only; the controller step is extra)"

  if awk -v cost="$TOTAL_ESTIMATED_ETH" -v balance="$DEPLOYER_BALANCE_ETH" \
    'BEGIN { exit !(cost > balance) }'; then
    echo ""
    echo "WARNING: the estimate exceeds the deployer balance. A run that stops part-way"
    echo "         leaves contracts deployed with no controller set on them."
  fi
}

estimate_if_needed() {
  local script_file="$1"
  local contract_name="$2"
  local return_key="$3"
  local env_target="$4"
  local label="$5"
  local existing_value="${!env_target:-}"

  if [[ -n "$existing_value" ]]; then
    echo "  ${label}: Already configured (${existing_value})"
    return
  fi

  local broadcast_value
  if broadcast_value=$(read_optional_broadcast_value "$script_file" "$return_key"); then
    echo "  ${label}: Already deployed (${broadcast_value})"
    return
  fi

  CONTRACTS_TO_DEPLOY+=("$label")
  echo ""
  echo "  ${label}:"

  local estimate_output
  estimate_output=$(cd "$PROJECT_ROOT" && forge script "script/${script_file}:${contract_name}" \
    "${FORGE_ARGS[@]}" 2>&1 || true)

  local gas_used eth_required
  gas_used=$(echo "$estimate_output" | awk -F': ' 'tolower($0) ~ /estimated total gas used/ { print $NF; exit }')
  eth_required=$(echo "$estimate_output" | awk -F': ' 'tolower($0) ~ /estimated amount required/ { print $NF; exit }')

  if [[ -n "$gas_used" ]]; then
    echo "    Gas:  ${gas_used}"
    echo "    Cost: ${eth_required}"
    add_estimated_cost "$eth_required"
  else
    echo "    Unable to estimate"
    ESTIMATE_COMPLETE="false"
  fi
}

run_deploy() {
  local target="$1"
  (cd "$PROJECT_ROOT" && forge script --slow "$target" "${RUN_ARGS[@]}")
}

deploy_contract() {
  local script_file="$1"
  local contract_name="$2"
  local return_key="$3"
  local env_target="$4"
  local label="$5"
  local existing_value="${!env_target:-}"

  if [[ -n "$existing_value" ]]; then
    echo "  [SKIP] ${label}: Using existing (${existing_value})"
    return
  fi

  local broadcast_value
  if broadcast_value=$(read_optional_broadcast_value "$script_file" "$return_key"); then
    export "${env_target}=${broadcast_value}"
    echo "  [SKIP] ${label}: Reusing previous deployment (${broadcast_value})"
    return
  fi

  log_step "Deploying ${label}"
  run_deploy "script/${script_file}:${contract_name}"

  if is_simulation; then
    echo "  [SIMULATED] ${label}: no address recorded"
    return
  fi

  extract_broadcast_return "$script_file" "$return_key" "$env_target"
  echo "  [DONE] ${label} deployed successfully"
}

first_missing_controller_input() {
  local var_name
  for var_name in \
    OFFER_TOKEN_FACTORY_ADDRESS \
    OFFER_TOKEN_SALE_FACTORY_ADDRESS \
    TOKEN_DISTRIBUTOR_FACTORY_ADDRESS \
    TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS \
    ALLOWLIST_ADDRESS; do
    if [[ -z "${!var_name:-}" ]]; then
      printf '%s' "$var_name"
      return 0
    fi
  done

  return 1
}

configure_controller() {
  export OFFER_SALE_ALLOWLIST_ADDRESS="$ALLOWLIST_ADDRESS"
  export CONTROLLER_ENABLED=true

  (cd "$PROJECT_ROOT" && forge script --slow \
    "script/ConfigureFactoriesAndAllowList.s.sol:ConfigureFactoriesAndAllowList" \
    "${RUN_ARGS[@]}")
}

write_deployment_summary() {
  local timestamp summary_file
  timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  summary_file="${CONFIGS_DIR}/deployment-${ENVIRONMENT}.json"

  mkdir -p "$CONFIGS_DIR"

  jq -n \
    --arg environment "$ENVIRONMENT" \
    --arg chainId "$CHAIN_ID" \
    --arg generatedAt "$timestamp" \
    --arg offerTokenFactory "$OFFER_TOKEN_FACTORY_ADDRESS" \
    --arg offerTokenSaleFactory "$OFFER_TOKEN_SALE_FACTORY_ADDRESS" \
    --arg tokenDistributorFactory "$TOKEN_DISTRIBUTOR_FACTORY_ADDRESS" \
    --arg tokenDistributorByHoldingsFactory "$TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS" \
    --arg offerSaleAllowlist "$ALLOWLIST_ADDRESS" \
    --arg brlaToken "$BRLA_ADDRESS" \
    '{
      environment: $environment,
      chainId: $chainId,
      generatedAt: $generatedAt,
      contracts: {
        offerTokenFactory: $offerTokenFactory,
        offerTokenSaleFactory: $offerTokenSaleFactory,
        tokenDistributorFactory: $tokenDistributorFactory,
        tokenDistributorByHoldingsFactory: $tokenDistributorByHoldingsFactory,
        offerSaleAllowlist: $offerSaleAllowlist,
        brlaToken: $brlaToken
      }
    }' >"$summary_file"

  chmod 644 "$summary_file"
  echo "Deployment record written to: $summary_file"
}

verification_targets() {
  local broadcast_file="$1"

  jq -r '
    .transactions[]
    | select(.transactionType == "CREATE" or .transactionType == "CREATE2")
    | select(.contractName != null and .contractAddress != null)
    | "\(.contractName)\t\(.contractAddress)"
  ' "$broadcast_file"
}

verify_contract_at() {
  local contract_name="$1"
  local contract_address="$2"
  local verify_args=(
    --chain "$CHAIN_ID"
    --rpc-url "$RPC_URL"
    --verifier "$VERIFIER"
    --etherscan-api-key "$ETHERSCAN_API_KEY"
    --guess-constructor-args
    --retries "$VERIFICATION_RETRIES"
    --watch
  )

  if [[ -n "${VERIFIER_URL:-}" ]]; then
    verify_args+=(--verifier-url "$VERIFIER_URL")
  fi

  (cd "$PROJECT_ROOT" && forge verify-contract "$contract_address" "$contract_name" "${verify_args[@]}")
}

report_unverified() {
  local entry contract_name contract_address

  echo "The deployment itself is complete: the contracts are on chain, the controller"
  echo "is set and the record is written. Only the published source is missing."
  echo ""
  echo "Retry by hand from ${PROJECT_ROOT}, with ${ENV_FILE} sourced:"
  echo ""

  for entry in "$@"; do
    contract_name="${entry%% *}"
    contract_address="${entry##* }"
    echo "  forge verify-contract ${contract_address} ${contract_name} \\"
    echo "    --chain ${CHAIN_ID} --rpc-url \"\$RPC_URL\" --verifier ${VERIFIER} \\"
    echo "    --guess-constructor-args --watch --etherscan-api-key \"\$ETHERSCAN_API_KEY\""
  done

  echo ""
  echo "The V2 endpoint takes keys issued by etherscan.io. A legacy BaseScan-only key"
  echo "is rejected there even though it still works against basescan.org."
}

verify_deployed_contracts() {
  local broadcast_file contract_name contract_address
  local unverified=()

  echo "=============================================="
  echo "  Verifying Source Code"
  echo "=============================================="
  echo "The contracts are deployed and configured. Verification only publishes their"
  echo "source, so nothing below can undo or damage the deployment."

  if [[ -n "${VERIFIER_URL:-}" ]]; then
    echo ""
    echo "Verifier URL: ${VERIFIER_URL} (set in the environment)"
    echo "Etherscan retired its V1 endpoints. If that host is api.basescan.org, drop"
    echo "VERIFIER_URL from ${ENV_FILE} and let forge derive the V2 endpoint itself."
  else
    echo ""
    echo "Verifier URL: derived by forge from --chain ${CHAIN_ID} (Etherscan V2)"
  fi

  for broadcast_file in "${BROADCAST_ROOT}"/*.s.sol/"${CHAIN_ID}"/run-latest.json; do
    if [[ ! -f "$broadcast_file" ]]; then
      continue
    fi

    while IFS=$'\t' read -r contract_name contract_address; do
      if [[ -z "$contract_name" ]]; then
        continue
      fi

      echo ""
      echo "--> ${contract_name} @ ${contract_address}"
      if verify_contract_at "$contract_name" "$contract_address"; then
        echo "  [VERIFIED] ${contract_name}"
      else
        echo "  [UNVERIFIED] ${contract_name}"
        unverified+=("${contract_name} ${contract_address}")
      fi
    done < <(verification_targets "$broadcast_file")
  done

  echo ""
  echo "=============================================="
  if [[ ${#unverified[@]} -eq 0 ]]; then
    echo "  Source verified for every deployed contract"
    echo "=============================================="
    return
  fi

  echo "  ${#unverified[@]} contract(s) left unverified"
  echo "=============================================="
  report_unverified "${unverified[@]}"
}

# -----------------------------------------------------------------------------
# Display Configuration
# -----------------------------------------------------------------------------
echo "=============================================="
echo "  Contract Deployment Wizard"
echo "=============================================="
if is_simulation; then
  echo "Mode:            SIMULATION (nothing will be broadcast)"
fi
echo "Environment:     ${ENVIRONMENT}"
echo "Chain ID:        ${CHAIN_ID}"
echo "RPC URL:         ${RPC_URL}"
echo "Deployer:        ${DEPLOYER_ADDRESS}"
echo "Controller:      ${CONTROLLER_ADDRESS}"
echo "BRLA Token:      ${BRLA_ADDRESS}"
echo "Broadcast:       ${FOUNDRY_BROADCAST}"
echo "=============================================="
echo ""

echo "Checking deployer balance..."
DEPLOYER_BALANCE=$(cast balance "$DEPLOYER_ADDRESS" --rpc-url "$RPC_URL" 2>/dev/null || echo "0")
DEPLOYER_BALANCE_ETH=$(cast from-wei "$DEPLOYER_BALANCE" 2>/dev/null || echo "unknown")
echo "Deployer balance: ${DEPLOYER_BALANCE_ETH} ETH"
echo ""

# -----------------------------------------------------------------------------
# Estimate Gas Costs
# -----------------------------------------------------------------------------
echo "=============================================="
echo "  Estimating Gas Costs (this may take a moment)"
echo "=============================================="

CONTRACTS_TO_DEPLOY=()
TOTAL_ESTIMATED_ETH="0"
ESTIMATE_COMPLETE="true"

estimate_if_needed "DeployOfferTokenFactory.s.sol" "DeployOfferTokenFactory" "factory" "OFFER_TOKEN_FACTORY_ADDRESS" "OfferTokenFactory"
estimate_if_needed "DeployOfferTokenSaleFactory.s.sol" "DeployOfferTokenSaleFactory" "factory" "OFFER_TOKEN_SALE_FACTORY_ADDRESS" "OfferTokenSaleFactory"
estimate_if_needed "DeployTokenDistributorFactory.s.sol" "DeployTokenDistributorFactory" "factory" "TOKEN_DISTRIBUTOR_FACTORY_ADDRESS" "TokenDistributorFactory"
estimate_if_needed "DeployTokenDistributorByHoldingsFactory.s.sol" "DeployTokenDistributorByHoldingsFactory" "factory" "TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS" "TokenDistributorByHoldingsFactory"
estimate_if_needed "DeployOfferSaleAllowlist.s.sol" "DeployOfferSaleAllowlist" "allowlist" "ALLOWLIST_ADDRESS" "OfferSaleAllowlist"

echo ""
echo "=============================================="
echo "  Plan"
echo "=============================================="
echo "Environment:         ${ENVIRONMENT}"
echo "Contracts to deploy: ${#CONTRACTS_TO_DEPLOY[@]}"
if [[ ${#CONTRACTS_TO_DEPLOY[@]} -gt 0 ]]; then
  for contract in "${CONTRACTS_TO_DEPLOY[@]}"; do
    echo "  - ${contract}"
  done
else
  echo "  (none: every contract is already recorded for this environment)"
fi
echo ""
echo "The controller step then grants ${CONTROLLER_ADDRESS} control of the"
echo "factories and the allowlist. It runs on every deployment and cannot be skipped."
echo ""
echo "Your balance:        ${DEPLOYER_BALANCE_ETH} ETH"
report_estimated_cost
echo ""
echo "Note: Actual costs may vary slightly based on network conditions."
echo "=============================================="
echo ""

if is_simulation; then
  echo "Simulating: no transaction will be broadcast."
else
  read -p "Proceed with deployment? (y/N) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Deployment cancelled."
    exit 0
  fi
fi

# -----------------------------------------------------------------------------
# Deploy Contracts
# -----------------------------------------------------------------------------
deploy_contract "DeployOfferTokenFactory.s.sol" "DeployOfferTokenFactory" "factory" "OFFER_TOKEN_FACTORY_ADDRESS" "OfferTokenFactory"
deploy_contract "DeployOfferTokenSaleFactory.s.sol" "DeployOfferTokenSaleFactory" "factory" "OFFER_TOKEN_SALE_FACTORY_ADDRESS" "OfferTokenSaleFactory"
deploy_contract "DeployTokenDistributorFactory.s.sol" "DeployTokenDistributorFactory" "factory" "TOKEN_DISTRIBUTOR_FACTORY_ADDRESS" "TokenDistributorFactory"
deploy_contract "DeployTokenDistributorByHoldingsFactory.s.sol" "DeployTokenDistributorByHoldingsFactory" "factory" "TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS" "TokenDistributorByHoldingsFactory"
deploy_contract "DeployOfferSaleAllowlist.s.sol" "DeployOfferSaleAllowlist" "allowlist" "ALLOWLIST_ADDRESS" "OfferSaleAllowlist"

# -----------------------------------------------------------------------------
# Configure The Controller
# -----------------------------------------------------------------------------
log_step "Configuring ${CONTROLLER_ADDRESS} as controller"

if MISSING_CONTROLLER_INPUT=$(first_missing_controller_input); then
  if ! is_simulation; then
    echo "Error: ${MISSING_CONTROLLER_INPUT} is unknown, so the controller cannot be configured." >&2
    echo "The deployed contracts reject every call the application makes until it is." >&2
    exit 1
  fi
  echo "  [SKIP] The controller step cannot be simulated before the contracts exist"
  echo "         on chain (${MISSING_CONTROLLER_INPUT} is unknown). Its first run is live."
else
  configure_controller
fi

if is_simulation; then
  echo ""
  echo "=============================================="
  echo "  Simulation Complete"
  echo "=============================================="
  echo "Nothing was broadcast: no transaction was sent, no deployment record was"
  echo "written, and no summary file was created."
  echo ""
  echo "Re-run without --simulate to deploy for real."
  echo "=============================================="
  exit 0
fi

# -----------------------------------------------------------------------------
# Write Deployment Record
# -----------------------------------------------------------------------------
log_step "Writing deployment record"
write_deployment_summary

echo ""
echo "=============================================="
echo "  Deployment Complete"
echo "=============================================="
echo "Environment: ${ENVIRONMENT} (chain ${CHAIN_ID})"
echo ""
echo "Factory Addresses:"
echo "  OfferTokenFactory:                 ${OFFER_TOKEN_FACTORY_ADDRESS}"
echo "  OfferTokenSaleFactory:             ${OFFER_TOKEN_SALE_FACTORY_ADDRESS}"
echo "  TokenDistributorFactory:           ${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS}"
echo "  TokenDistributorByHoldingsFactory: ${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS}"
echo "  OfferSaleAllowlist:                ${ALLOWLIST_ADDRESS}"
echo ""
echo "BRLA Token (configured):             ${BRLA_ADDRESS}"
echo ""
echo "Point a Convex deployment at this generation:"
echo ""
echo "bunx convex env set OFFER_TOKEN_FACTORY_ADDRESS ${OFFER_TOKEN_FACTORY_ADDRESS}"
echo "bunx convex env set OFFER_TOKEN_SALE_FACTORY_ADDRESS ${OFFER_TOKEN_SALE_FACTORY_ADDRESS}"
echo "bunx convex env set TOKEN_DISTRIBUTOR_FACTORY_ADDRESS ${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS}"
echo "bunx convex env set TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS ${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS}"
echo "bunx convex env set OFFER_SALE_ALLOWLIST_ADDRESS ${ALLOWLIST_ADDRESS}"
echo "bunx convex env set BRLA_ADDRESS ${BRLA_ADDRESS}"
echo ""
EXPLORER_URL="https://basescan.org"
if [[ "$CHAIN_ID" == "$BASE_SEPOLIA_CHAIN_ID" ]]; then
  EXPLORER_URL="https://sepolia.basescan.org"
fi
echo "View on BaseScan: ${EXPLORER_URL}/address/${OFFER_TOKEN_FACTORY_ADDRESS}"
echo "=============================================="
echo ""

verify_deployed_contracts || true

exit 0
