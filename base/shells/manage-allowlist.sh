#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIGS_DIR="${PROJECT_ROOT}/configs"

usage() {
  cat <<'EOF'
Manage the OfferSaleAllowlist for a deployment environment.

Usage:
  ./manage-allowlist.sh <environment> status  <address>
  ./manage-allowlist.sh <environment> add     <address> <limitBRL>
  ./manage-allowlist.sh <environment> disable <address>
  ./manage-allowlist.sh <environment> reset   <address>

  status   read the remaining allocation and whether the address may purchase
  add      grant or raise an allocation, in whole BRL, and enable the address
  disable  set the allocation to zero and disable the address
  reset    clear the consumed amount, restoring the full allocation

The allowlist address is read from configs/deployment-<environment>.json.
RPC_URL is read from .env.<environment>. Writes are signed with the `deployer`
keystore and sent to the chain named in that environment file.

This writes to the same allowlist the application reads. It does not update the
application's own records, so an address added here is allowlisted on chain but
will not appear as allowlisted in the platform admin interface.
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

ALLOWLIST="$(jq -r '.contracts.offerSaleAllowlist' "$RECORD")"
RECORD_CHAIN="$(jq -r '.chainId' "$RECORD")"

if [[ -z "$ALLOWLIST" || "$ALLOWLIST" == "null" ]]; then
  echo "Error: no offerSaleAllowlist address in ${RECORD}" >&2
  exit 1
fi

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

brl_to_units() {
  local brl="$1"
  if [[ ! "$brl" =~ ^[0-9]+$ ]]; then
    echo "Error: limit must be a whole number of BRL, got '${brl}'" >&2
    exit 1
  fi
  printf '%s000000000000000000' "$brl"
}

read_status() {
  local address="$1"
  local remaining
  remaining="$(cast call "$ALLOWLIST" 'remainingAllocation(address)(uint256)' "$address" --rpc-url "$RPC_URL")"
  remaining="${remaining%% *}"
  echo "Allowlist:  ${ALLOWLIST}"
  echo "Address:    ${address}"
  echo "Remaining:  ${remaining} units"
  if [[ "$remaining" != "0" ]]; then
    echo "            ~R$ $(cast from-wei "$remaining")"
  fi
  echo "Can buy 1:  $(cast call "$ALLOWLIST" 'canPurchase(address,uint256)(bool)' "$address" 1000000000000000000 --rpc-url "$RPC_URL")"
}

require_controller() {
  require_env "DEPLOYER_ADDRESS"
  local is_controller
  is_controller="$(cast call "$ALLOWLIST" 'controllers(address)(bool)' "$DEPLOYER_ADDRESS" --rpc-url "$RPC_URL")"
  is_controller="${is_controller%% *}"
  if [[ "$is_controller" == "false" ]]; then
    echo "Error: ${DEPLOYER_ADDRESS} is not a controller on ${ALLOWLIST}" >&2
    echo "setAllocations and resetConsumed would revert. Grant it control first," >&2
    echo "or run this from an account that already has it." >&2
    exit 1
  fi
}

confirm_write() {
  require_controller
  echo "Allowlist:   ${ALLOWLIST}"
  echo "Environment: ${ENVIRONMENT} (chain ${CHAIN_ID})"
  echo "Action:      $1"
  echo ""
  read -p "Send this transaction? (y/N) " -n 1 -r
  echo
  [[ $REPLY =~ ^[Yy]$ ]] || { echo "Cancelled."; exit 0; }
}

send() {
  cast send "$ALLOWLIST" "$@" --rpc-url "$RPC_URL" --chain "$CHAIN_ID" --account deployer
}

case "$COMMAND" in
  status)
    require_address "${1:-}"
    read_status "$1"
    ;;

  add)
    require_address "${1:-}"
    UNITS="$(brl_to_units "${2:-}")"
    confirm_write "allow ${1} up to R$ ${2}"
    send 'setAllocations(address[],uint256[],bool[])' "[${1}]" "[${UNITS}]" '[true]'
    echo ""
    read_status "$1"
    ;;

  disable)
    require_address "${1:-}"
    confirm_write "disable ${1}"
    send 'setAllocations(address[],uint256[],bool[])' "[${1}]" '[0]' '[false]'
    echo ""
    read_status "$1"
    ;;

  reset)
    require_address "${1:-}"
    confirm_write "reset consumed for ${1}"
    send 'resetConsumed(address[])' "[${1}]"
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
