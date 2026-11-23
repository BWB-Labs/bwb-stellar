#!/usr/bin/env bash
set -euo pipefail

if ! command -v cast >/dev/null 2>&1; then
  echo "Foundry 'cast' CLI is required to run this script" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Load project environment variables when available
if [[ -f "${PROJECT_ROOT}/.env" ]]; then
  # shellcheck disable=SC1091
  source "${PROJECT_ROOT}/.env"
fi

KEYSTORE_DIR="${FOUNDRY_ETH_KEYSTORE:-$HOME/.foundry/keystores}"
ACCOUNT_MAPPINGS=(
  "deployer:DEPLOYER_ADDRESS"
  "advisor:ADVISOR_ADDRESS"
  "investor1:INVESTOR1_ADDRESS"
  "investor2:INVESTOR2_ADDRESS"
  "investor3:INVESTOR3_ADDRESS"
)

declare -A ACCOUNT_SEEN=()

PASSWORD_VALUE="${FOUNDRY_WALLET_PASSWORD:-${FOUNDRY_PASSWORD:-${CAST_WALLET_PASSWORD:-}}}"
PASSWORD_FILE="${FOUNDRY_WALLET_PASSWORD_FILE:-${FOUNDRY_PASSWORD_FILE:-${CAST_WALLET_PASSWORD_FILE:-}}}"

build_password_args() {
  local alias="$1"
  local -n result_ref="$2"
  result_ref=()

  local alias_password_file="${KEYSTORE_DIR}/${alias}.pass"

  if [[ -f "$alias_password_file" ]]; then
    result_ref=(--password-file "$alias_password_file")
  elif [[ -n "${PASSWORD_FILE:-}" ]]; then
    result_ref=(--password-file "$PASSWORD_FILE")
  elif [[ -n "${PASSWORD_VALUE:-}" ]]; then
    result_ref=(--password "$PASSWORD_VALUE")
  fi
}

find_keystore_file() {
  local alias="$1"
  local candidate
  for candidate in "${KEYSTORE_DIR}/${alias}.json" "${KEYSTORE_DIR}/${alias}"; do
    if [[ -f "$candidate" ]]; then
      printf '%s' "$candidate"
      return 0
    fi
  done
  return 1
}

address_from_keystore() {
  local alias="$1"
  local -a password_args=()
  build_password_args "$alias" password_args

  local keystore_path address
  if keystore_path=$(find_keystore_file "$alias" 2>/dev/null); then
    if address=$(cast wallet address --keystore "$keystore_path" "${password_args[@]}" 2>/dev/null); then
      printf '%s' "$address"
      return 0
    fi
  fi

  if address=$(cast wallet address "$alias" "${password_args[@]}" 2>/dev/null); then
    printf '%s' "$address"
    return 0
  fi

  return 1
}

print_env_summary() {
  echo "==> Environment configuration"
  printf '  %-12s %s\n' "Alias" "Value"
  for mapping in "${ACCOUNT_MAPPINGS[@]}"; do
    local alias env_key env_value
    IFS=':' read -r alias env_key <<<"$mapping"
    env_value="${!env_key:-}"
    if [[ -z "$env_value" ]]; then
      env_value="(unset)"
    fi
    printf '  %-12s %s\n' "$alias" "$env_value"
  done
}

print_primary_keystore_addresses() {
  echo
  echo "==> Account addresses derived from keystores"
  printf '  %-12s %s\n' "Alias" "Keystore address"
  for mapping in "${ACCOUNT_MAPPINGS[@]}"; do
    local alias derived address
    IFS=':' read -r alias _ <<<"$mapping"
    address="(unavailable)"
    if derived=$(address_from_keystore "$alias" 2>/dev/null); then
      address="$derived"
    fi
    ACCOUNT_SEEN["$alias"]=1
    printf '  %-12s %s\n' "$alias" "$address"
  done
}

list_keystore_accounts() {
  echo
  echo "==> Foundry keystore entries (${KEYSTORE_DIR})"

  if [[ ! -d "$KEYSTORE_DIR" ]]; then
    echo "  (keystore directory not found)"
    return
  fi

  shopt -s nullglob
  local entries=("${KEYSTORE_DIR}"/* "${KEYSTORE_DIR}"/*.json)
  shopt -u nullglob

  if [[ ${#entries[@]} -eq 0 ]]; then
    echo "  (no keystore files found)"
    return
  fi

  for entry in "${entries[@]}"; do
    if [[ ! -f "$entry" ]]; then
      continue
    fi

    local filename="${entry##*/}"
    local alias="${filename%.json}"

    if [[ -n "${ACCOUNT_SEEN[$alias]:-}" ]]; then
      continue
    fi

    ACCOUNT_SEEN["$alias"]=1

    local derived address
    address="(unavailable)"
    if derived=$(address_from_keystore "$alias" 2>/dev/null); then
      address="$derived"
    fi

    printf '  %-16s %s\n' "$alias" "$address"
  done
}


print_env_summary
print_primary_keystore_addresses
list_keystore_accounts
