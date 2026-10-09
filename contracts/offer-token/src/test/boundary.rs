//! Edges of supply, amounts and construction inputs.
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use super::{code, ctor_args, err, parties, setup};
use crate::{OfferToken, OfferTokenClient, OfferTokenError};

const INSUFFICIENT_BALANCE: u32 = 100;

#[test]
#[should_panic(expected = "Error(Contract, #6007)")]
fn zero_supply_is_rejected_at_construction() {
    let e = Env::default();
    let p = parties(&e);
    e.register(OfferToken, ctor_args(&e, &p, 0, vec![&e]));
}

#[test]
#[should_panic(expected = "Error(Contract, #6007)")]
fn negative_supply_is_rejected_at_construction() {
    let e = Env::default();
    let p = parties(&e);
    e.register(OfferToken, ctor_args(&e, &p, -1, vec![&e]));
}

#[test]
fn supply_of_one_is_valid() {
    let e = Env::default();
    let p = parties(&e);
    let id = e.register(OfferToken, ctor_args(&e, &p, 1, vec![&e]));
    let client = OfferTokenClient::new(&e, &id);
    assert_eq!(client.balance(&p.sale), 1);
}

#[test]
fn duplicate_whitelist_inputs_are_ignored() {
    let e = Env::default();
    let p = parties(&e);
    let listed = Address::generate(&e);
    let list = vec![&e, listed.clone(), p.sale.clone(), listed.clone()];
    let id = e.register(OfferToken, ctor_args(&e, &p, 10, list));
    let client = OfferTokenClient::new(&e, &id);
    assert!(client.is_transfer_whitelisted(&listed));
    assert!(client.is_transfer_whitelisted(&p.sale));
}

#[test]
fn whole_balance_moves_and_one_more_does_not() {
    let s = setup();
    let r = s.client.try_transfer(&s.alice, &s.listed, &101);
    assert_eq!(err(r), code(INSUFFICIENT_BALANCE));
    s.client.transfer(&s.alice, &s.listed, &100);
    assert_eq!(s.client.balance(&s.alice), 0);
}

#[test]
fn zero_transfer_is_allowed_between_permitted_parties() {
    let s = setup();
    s.client.transfer(&s.alice, &s.listed, &0);
    assert_eq!(s.client.balance(&s.alice), 100);
}

#[test]
fn forced_transfer_rejects_same_endpoints_and_non_positive_amounts() {
    let s = setup();
    for amount in [0_i128, -1] {
        let r = s
            .client
            .try_admin_transfer(&s.alice, &s.bob, &amount, &s.xfer_admin);
        assert_eq!(err(r), OfferTokenError::InvalidAdminTransfer.into());
    }
    let r = s
        .client
        .try_admin_transfer(&s.alice, &s.alice, &1, &s.xfer_admin);
    assert_eq!(err(r), OfferTokenError::InvalidAdminTransfer.into());
}

#[test]
fn forced_transfer_cannot_exceed_the_balance() {
    let s = setup();
    let r = s
        .client
        .try_admin_transfer(&s.alice, &s.bob, &101, &s.xfer_admin);
    assert_eq!(err(r), code(INSUFFICIENT_BALANCE));
    s.client
        .admin_transfer(&s.alice, &s.bob, &100, &s.xfer_admin);
    assert_eq!(s.client.balance(&s.bob), 200);
}
