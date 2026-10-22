//! What the sale needs from `offer-token` beyond SEP-41 (`transfer`,
//! `balance` and `burn` go through the SDK's token client): the supply, an
//! OpenZeppelin read. Hand-written for the same reason as the allowlist
//! client.
use soroban_sdk::{contractclient, Env};

// The trait exists for the client the attribute derives from it.
#[allow(dead_code)]
#[contractclient(name = "OfferTokenSupplyClient")]
pub trait OfferTokenSupply {
    fn total_supply(e: Env) -> i128;
}
