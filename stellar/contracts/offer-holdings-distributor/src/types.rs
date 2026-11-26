//! The distributor's stored and passed values. Each is a `#[contracttype]`,
//! so the CLI takes it as one named JSON value and fields can't be swapped
//! by position.
use soroban_sdk::{contracttype, Address};

/// Who holds each of the distributor's three powers. Fixed at construction,
/// no setter: changing a holder takes an upgrade, as on the other
/// contracts.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PowerHolders {
    /// The issuer's treasury: it deposits, withdraws, pauses and unpauses.
    /// Every deposit comes from this account and every withdrawal goes to
    /// it.
    pub owner: Address,
    /// The platform multisig: pauses, never unpauses.
    pub pauser: Address,
    /// The platform multisig: replaces the code.
    pub upgrader: Address,
}

/// The two contracts the distributor works with. Fixed at construction.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Contracts {
    /// The offering's quota token: balances and supply are read from it.
    pub token: Address,
    /// The asset deposits come in and claims are paid in.
    pub payment_asset: Address,
}
