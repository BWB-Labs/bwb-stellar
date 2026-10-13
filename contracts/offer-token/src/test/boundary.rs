//! Edges of supply, amounts and construction inputs.
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use super::{code, ctor_args, err, parties, setup};
use crate::{
    OfferToken, OfferTokenClient, OfferTokenError, MAX_INITIAL_OPERATIONAL_ACCOUNTS, MAX_URI_LEN,
};

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
#[should_panic(expected = "Error(Contract, #6008)")]
fn constructor_rejects_operational_accounts_over_the_cap() {
    let e = Env::default();
    let p = parties(&e);
    let mut list = soroban_sdk::Vec::new(&e);
    for _ in 0..=MAX_INITIAL_OPERATIONAL_ACCOUNTS {
        list.push_back(Address::generate(&e));
    }
    e.register(OfferToken, ctor_args(&e, &p, 10, list));
}

#[test]
fn constructor_accepts_operational_accounts_at_the_cap() {
    let e = Env::default();
    let p = parties(&e);
    let mut list = soroban_sdk::Vec::new(&e);
    for _ in 0..MAX_INITIAL_OPERATIONAL_ACCOUNTS {
        list.push_back(Address::generate(&e));
    }
    let last = list.last().unwrap();
    let id = e.register(OfferToken, ctor_args(&e, &p, 10, list));
    assert!(OfferTokenClient::new(&e, &id).is_operational_account(&last));
}

#[test]
#[should_panic(expected = "Error(Contract, #6009)")]
fn constructor_rejects_a_uri_over_the_cap() {
    let e = Env::default();
    let p = parties(&e);
    let mut args = ctor_args(&e, &p, 10, vec![&e]);
    args.9 = soroban_sdk::String::from_str(&e, &"u".repeat(MAX_URI_LEN as usize + 1));
    e.register(OfferToken, args);
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
fn duplicate_operational_account_inputs_are_ignored() {
    let e = Env::default();
    let p = parties(&e);
    let listed = Address::generate(&e);
    let list = vec![&e, listed.clone(), p.sale.clone(), listed.clone()];
    let id = e.register(OfferToken, ctor_args(&e, &p, 10, list));
    let client = OfferTokenClient::new(&e, &id);
    assert!(client.is_operational_account(&listed));
    assert!(client.is_operational_account(&p.sale));
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
