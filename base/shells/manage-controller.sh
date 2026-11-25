#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"

usage() {
  cat <<'EOF'
Grant, revoke or inspect controller rights for an address.

Usage:
  ./manage-controller.sh <environment> status <address>
  ./manage-controller.sh <environment> grant  <address>
  ./manage-controller.sh <environment> revoke <address>

Controller rights are what the platform server wallet needs to act on behalf of
the application. Granting covers all five contracts at once:

  OfferTokenFactory                  createOfferToken
  OfferTokenSaleFactory              createOfferTokenSale
  TokenDistributorFactory            createTokenDistributor
  TokenDistributorByHoldingsFactory  createDistributor
  OfferSaleAllowlist                 setAllocations, resetConsumed, and registering
                                     each new sale contract as a controller of the
                                     allowlist

Deploying an organization treasury needs no controller rights: the Safe proxy
factory is permissionless.

setController is onlyOwner on all five contracts, so this must run with the
keystore of the account that owns them. Addresses come from
configs/deployment-<environment>.json and RPC_URL from .env.<environment>.
EOF
}

if [[ $# -lt 2 ]]; then
  usage
  exit 1
fi

ENVIRONMENT="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
COMMAND="$2"
shift 2

ENV_FILE="${PROJECT_ROOT}/.env.${ENVIRONMENT}"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "Error: ${ENV_FILE} not found" >&2
  exit 1
fi

set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

require_env() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    echo "Error: ${var_name} is required in ${ENV_FILE}" >&2
    exit 1
  fi
}

require_env "RPC_URL"

RECORD="${CONFIGS_DIR}/deployment-${ENVIRONMENT}.json"
if [[ ! -f "$RECORD" ]]; then
  echo "Error: deployment record not found at ${RECORD}" >&2
  echo "Run ./deploy-wizard.sh ${ENVIRONMENT} first." >&2
  exit 1
fi

OFFER_TOKEN_FACTORY_ADDRESS="$(jq -r '.contracts.offerTokenFactory' "$RECORD")"
OFFER_TOKEN_SALE_FACTORY_ADDRESS="$(jq -r '.contracts.offerTokenSaleFactory' "$RECORD")"
TOKEN_DISTRIBUTOR_FACTORY_ADDRESS="$(jq -r '.contracts.tokenDistributorFactory' "$RECORD")"
TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS="$(jq -r '.contracts.tokenDistributorByHoldingsFactory' "$RECORD")"
OFFER_SALE_ALLOWLIST_ADDRESS="$(jq -r '.contracts.offerSaleAllowlist' "$RECORD")"
RECORD_CHAIN="$(jq -r '.chainId' "$RECORD")"
export OFFER_TOKEN_FACTORY_ADDRESS OFFER_TOKEN_SALE_FACTORY_ADDRESS
export TOKEN_DISTRIBUTOR_FACTORY_ADDRESS TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS
export OFFER_SALE_ALLOWLIST_ADDRESS

for pair in \
  "offerTokenFactory:${OFFER_TOKEN_FACTORY_ADDRESS}" \
  "offerTokenSaleFactory:${OFFER_TOKEN_SALE_FACTORY_ADDRESS}" \
  "tokenDistributorFactory:${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS}" \
  "tokenDistributorByHoldingsFactory:${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS}" \
  "offerSaleAllowlist:${OFFER_SALE_ALLOWLIST_ADDRESS}"; do
  if [[ -z "${pair##*:}" || "${pair##*:}" == "null" ]]; then
    echo "Error: no ${pair%%:*} address in ${RECORD}" >&2
    exit 1
  fi
done

if [[ "${CHAIN_ID:-}" != "$RECORD_CHAIN" ]]; then
  echo "Error: CHAIN_ID is ${CHAIN_ID:-unset} but ${RECORD} records chain ${RECORD_CHAIN}" >&2
  exit 1
fi

require_address() {
  if [[ ! "${1:-}" =~ ^0x[0-9a-fA-F]{40}$ ]]; then
    echo "Error: '${1:-}' is not an address" >&2
    exit 1
  fi
}

read_status() {
  local address="$1"
  echo "Environment: ${ENVIRONMENT} (chain ${CHAIN_ID})"
  echo "Address:     ${address}"
  echo ""
  printf '  %-36s %-44s %s\n' "CONTRACT" "ADDRESS" "CONTROLLER"
  for pair in \
    "OfferTokenFactory:${OFFER_TOKEN_FACTORY_ADDRESS}:isController(address)(bool)" \
    "OfferTokenSaleFactory:${OFFER_TOKEN_SALE_FACTORY_ADDRESS}:isController(address)(bool)" \
    "TokenDistributorFactory:${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS}:isController(address)(bool)" \
    "TokenDistributorByHoldingsFactory:${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS}:isController(address)(bool)" \
    "OfferSaleAllowlist:${OFFER_SALE_ALLOWLIST_ADDRESS}:controllers(address)(bool)"; do
    local name contract getter answer
    name="${pair%%:*}"
    contract="$(printf '%s' "$pair" | cut -d: -f2)"
    getter="$(printf '%s' "$pair" | cut -d: -f3-)"
    answer="$(cast call "$contract" "$getter" "$address" --rpc-url "$RPC_URL")"
    printf '  %-36s %-44s %s\n' "$name" "$contract" "${answer%% *}"
  done
}

require_owner() {
  require_env "DEPLOYER_ADDRESS"
  local signer owner
  signer="$(printf '%s' "$DEPLOYER_ADDRESS" | tr '[:upper:]' '[:lower:]')"
  for pair in \
    "OfferTokenFactory:${OFFER_TOKEN_FACTORY_ADDRESS}" \
    "OfferTokenSaleFactory:${OFFER_TOKEN_SALE_FACTORY_ADDRESS}" \
    "TokenDistributorFactory:${TOKEN_DISTRIBUTOR_FACTORY_ADDRESS}" \
    "TokenDistributorByHoldingsFactory:${TOKEN_DISTRIBUTOR_BY_HOLDINGS_FACTORY_ADDRESS}" \
    "OfferSaleAllowlist:${OFFER_SALE_ALLOWLIST_ADDRESS}"; do
    owner="$(cast call "${pair##*:}" 'owner()(address)' --rpc-url "$RPC_URL")"
    owner="$(printf '%s' "${owner%% *}" | tr '[:upper:]' '[:lower:]')"
    if [[ "$owner" != "$signer" ]]; then
      echo "Error: ${DEPLOYER_ADDRESS} does not own ${pair%%:*} (${pair##*:})" >&2
      echo "Owner is ${owner}. setController is onlyOwner, so this would revert." >&2
      exit 1
    fi
  done
}

configure() {
  local enabled="$1"
  export CONTROLLER_ADDRESS="$2"
  export CONTROLLER_ENABLED="$enabled"
  export FOUNDRY_BROADCAST="broadcast/${ENVIRONMENT}"

  (cd "$PROJECT_ROOT" && forge script --slow \
    "script/ConfigureFactoriesAndAllowList.s.sol:ConfigureFactoriesAndAllowList" \
    --rpc-url "$RPC_URL" --chain "$CHAIN_ID" --broadcast --account deployer)
}

confirm() {
  require_owner
  echo "Environment: ${ENVIRONMENT} (chain ${CHAIN_ID})"
  echo "Action:      $1"
  echo "Contracts:   OfferTokenFactory, OfferTokenSaleFactory,"
  echo "             TokenDistributorFactory, TokenDistributorByHoldingsFactory,"
  echo "             OfferSaleAllowlist"
  echo ""
  read -p "Send these transactions? (y/N) " -n 1 -r
  echo
  [[ $REPLY =~ ^[Yy]$ ]] || { echo "Cancelled."; exit 0; }
}

case "$COMMAND" in
  status)
    require_address "${1:-}"
    read_status "$1"
    ;;

  grant)
    require_address "${1:-}"
    confirm "grant controller rights to ${1}"
    configure true "$1"
    echo ""
    read_status "$1"
    ;;

  revoke)
    require_address "${1:-}"
    confirm "revoke controller rights from ${1}"
    configure false "$1"
    echo ""
    read_status "$1"
    ;;

  *)
    echo "Error: unknown command '${COMMAND}'" >&2
    echo ""
    usage
    exit 1
    ;;
esac
