#!/usr/bin/env bash
# Print an offering's token and sale contract addresses before deploying.
#
#   scripts/offering-addresses.sh <offering-id>
#
# Environment: NETWORK (default testnet), SOURCE (the deployer identity;
# defaults to bwb-testnet-deployer on testnet only). The addresses depend
# only on the deployer and the salts, not on code or constructor arguments.
#
# Example, deploying the token with the sale's address as initial holder:
#   SALE=$(scripts/offering-addresses.sh aurora-01 | jq -r .sale.contract_id)
#   OFFERING=aurora-01 scripts/deploy.sh offer-token -- --initial_holder "$SALE" ...
set -euo pipefail

OFFERING="${1:?usage: scripts/offering-addresses.sh <offering-id>}"
NETWORK="${NETWORK:-testnet}"

# shellcheck source=lib/offering.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/offering.sh"
resolve_source "$NETWORK"

TOKEN_SALT="$(offering_salt "$OFFERING" token)"
SALE_SALT="$(offering_salt "$OFFERING" sale)"
TOKEN_ID="$(offering_address "$NETWORK" "$SOURCE" "$TOKEN_SALT")"
SALE_ID="$(offering_address "$NETWORK" "$SOURCE" "$SALE_SALT")"
DEPLOYER="$(stellar keys address "$SOURCE")"

python3 - "$OFFERING" "$NETWORK" "$DEPLOYER" "$TOKEN_SALT" "$TOKEN_ID" "$SALE_SALT" "$SALE_ID" <<'PY'
import json, sys
offering, network, deployer, ts, tid, ss, sid = sys.argv[1:]
print(json.dumps({
    "offering": offering,
    "network": network,
    "deployer": deployer,
    "token": {"salt": ts, "contract_id": tid},
    "sale": {"salt": ss, "contract_id": sid},
}, indent=2))
PY
