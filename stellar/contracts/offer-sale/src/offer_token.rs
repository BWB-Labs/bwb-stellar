//! What the sale needs from `offer-token` beyond SEP-41 (`transfer`,
//! `balance` and `burn` go through the SDK's token client): the supply, an
//! OpenZeppelin read, and the initial holder. Hand-written for the same
//! reason as the allowlist client.
use soroban_sdk::{contractclient, Address, Env};

// The trait exists for the client the attribute derives from it.
#[allow(dead_code)]
#[contractclient(name = "OfferTokenSupplyClient")]
pub trait OfferTokenSupply {
    fn total_supply(e: Env) -> i128;

    /// The account the supply was minted to; void on a token older than
    /// 0.2.0, which has no such entry (and older still, no such call).
    fn initial_holder(e: Env) -> Option<Address>;
}
