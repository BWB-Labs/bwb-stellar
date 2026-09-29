//! Offer token: the investor's position in one offering. One instance per offer.
//! Soroban port of Base's `OfferERC20`.
//!
//! PLACEHOLDER. This is the shape of the CLI template plus an admin set through
//! OpenZeppelin's access control, so the workspace, the deploy script and the
//! constructor path can be exercised before the real contract lands (L2).
#![no_std]

use soroban_sdk::{contract, contractimpl, vec, Address, Env, String, Symbol, Vec};
use stellar_access::access_control::{set_admin, AccessControl};

#[contract]
pub struct OfferToken;

#[contractimpl]
impl OfferToken {
    pub fn __constructor(e: &Env, admin: Address) {
        set_admin(e, &admin);
    }

    pub fn hello(e: &Env, to: String) -> Vec<String> {
        vec![e, String::from_str(e, "Hello"), to]
    }
}

#[contractimpl(contracttrait)]
impl AccessControl for OfferToken {}

mod test;
