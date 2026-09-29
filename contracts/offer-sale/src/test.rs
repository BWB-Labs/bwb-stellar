#![cfg(test)]

use super::*;
use soroban_sdk::Env;

#[test]
fn registers() {
    let e = Env::default();
    let _id = e.register(OfferSale, ());
}
