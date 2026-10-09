# Shared by deploy.sh and offering-addresses.sh. Sourced, not run.
#
# An offering's token and sale contract live at addresses derived from the
# deployer and a salt, known before either is deployed. The salt comes from
# the offering identifier, so a retry or a second operator recomputes the
# same addresses:
#
#   salt = sha256("bwb:<offering-id>:<role>"), role = token | sale
#
# The network is deliberately not in the salt: the contract address already
# includes the network ID, and the CLI's name for a network is local config
# (mainnet, public, or any alias), which would give one offering different
# addresses depending on who runs the script. The L8 factory is expected to
# use the same rule.

# offering_salt <offering-id> <role> -> 64 hex chars
offering_salt() {
  python3 -c 'import hashlib, sys; print(hashlib.sha256(sys.argv[1].encode()).hexdigest())' \
    "bwb:$1:$2"
}

# resolve_source <network>: sets SOURCE, defaulting to the testnet deployer
# on testnet only.
resolve_source() {
  case "$1" in
    testnet) SOURCE="${SOURCE:-bwb-testnet-deployer}" ;;
    *) : "${SOURCE:?SOURCE must be set explicitly for $1 (no default identity outside testnet)}" ;;
  esac
}

# crate_role <crate> -> token | sale; fails for crates that are not per-offering
crate_role() {
  case "$1" in
    offer-token) echo token ;;
    offer-sale) echo sale ;;
    *) return 1 ;;
  esac
}

# offering_address <network> <source> <salt> -> C... address
offering_address() {
  stellar contract id wasm --network "$1" --source-account "$2" --salt "$3"
}
