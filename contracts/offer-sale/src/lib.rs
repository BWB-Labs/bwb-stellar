//! Sale: escrow and lifecycle of one offering. One instance per offer.
//! Soroban port of Base's `OfferTokenSale`.
//!
//! PLACEHOLDER until L4.
#![no_std]

use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct OfferSale;

#[contractimpl]
impl OfferSale {}

mod test;
