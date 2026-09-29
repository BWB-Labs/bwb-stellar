//! Allowlist: who may invest and up to how much, administered by BWB.
//! One global instance. Soroban port of Base's `OfferSaleAllowlist`.
//!
//! PLACEHOLDER until L3.
#![no_std]

use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct OfferAllowlist;

#[contractimpl]
impl OfferAllowlist {}

mod test;
