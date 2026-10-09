# Shared by deploy.sh and offering-addresses.sh. Sourced, not run.
#
# An offering's token and sale contract live at addresses derived from the
# deployer and a salt, known before either is deployed. The salt comes from
# the offering identifier, so a retry or a second operator recomputes the
# same addresses:
#
#   salt = sha256("bwb:<network>:<offering-id>:<role>"), role = token | sale
#
# The L8 factory is expected to use the same rule.

# offering_salt <network> <offering-id> <role> -> 64 hex chars
offering_salt() {
  python3 -c 'import hashlib, sys; print(hashlib.sha256(sys.argv[1].encode()).hexdigest())' \
    "bwb:$1:$2:$3"
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
