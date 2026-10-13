//! The constructor's grouped arguments. Each is a `#[contracttype]` struct,
//! so callers pass it as one named value (JSON on the CLI) and fields can't
//! be swapped by position.
use soroban_sdk::{contracttype, Address, String};

/// Who holds each of the token's four powers. Passed to the constructor as
/// one argument, so each holder is named at the call site and two of them
/// can't be swapped by position. Fixed at construction: there is no setter,
/// so changing a holder takes an upgrade.
#[contracttype]
pub struct PowerHolders {
    /// Manages the whitelist and the offering URI, pauses and unpauses.
    /// Routine operation.
    pub admin: Address,
    /// Moves tokens between any two accounts, ignoring pause and the
    /// whitelist (court orders, lost keys, estates). The most dangerous
    /// power, kept apart from `admin` so it can sit behind a stricter quorum.
    pub xfer_admin: Address,
    /// Pauses, never unpauses.
    pub pauser: Address,
    /// Replaces the contract's code.
    pub upgrader: Address,
}

/// The offering's descriptive data, set at construction.
#[contracttype]
pub struct OfferingMetadata {
    /// Token name (SEP-41 `name`).
    pub name: String,
    /// Token symbol (SEP-41 `symbol`).
    pub symbol: String,
    /// URI of the offering's documents, at most `MAX_URI_LEN` bytes.
    /// Replaceable later by `admin` with `set_offer_uri`.
    pub offer_uri: String,
}
