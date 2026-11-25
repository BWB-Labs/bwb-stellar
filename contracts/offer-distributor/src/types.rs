//! The distributor's stored and passed values. Each is a `#[contracttype]`,
//! so the CLI takes it as one named JSON value and fields can't be swapped
//! by position.
use soroban_sdk::{contracttype, Address};

/// Who holds each of the distributor's three powers. Fixed at construction,
/// no setter: changing a holder takes an upgrade, as on the token and the
/// sale.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PowerHolders {
    /// The treasury that pays: it distributes, pauses and unpauses. Every
    /// distribution is paid from this account.
    pub owner: Address,
    /// The platform multisig: pauses, never unpauses.
    pub pauser: Address,
    /// The platform multisig: replaces the code.
    pub upgrader: Address,
}

/// One leg of a distribution.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Payout {
    pub recipient: Address,
    pub amount: i128,
}
