//! Every documented rejection is reachable through the client and carries
//! its code.
use soroban_sdk::{testutils::Address as _, Address, String};

use super::{code, err, setup};
use crate::{OfferTokenError, MAX_URI_LEN};

const INSUFFICIENT_ALLOWANCE: u32 = 101;
const LESS_THAN_ZERO: u32 = 103;

#[test]
fn burn_is_disabled() {
    let s = setup();
    assert_eq!(
        err(s.client.try_burn(&s.alice, &1)),
        OfferTokenError::BurnDisabled.into()
    );
    assert_eq!(s.client.balance(&s.alice), 100);
}

#[test]
fn burn_from_is_disabled() {
    let s = setup();
    let spender = Address::generate(&s.e);
    s.client.approve(&s.alice, &spender, &10, &1_000);
    let r = s.client.try_burn_from(&spender, &s.alice, &1);
    assert_eq!(err(r), OfferTokenError::BurnDisabled.into());
}

#[test]
fn transfer_from_without_allowance_fails() {
    let s = setup();
    let spender = Address::generate(&s.e);
    let r = s
        .client
        .try_transfer_from(&spender, &s.alice, &s.listed, &1);
    assert_eq!(err(r), code(INSUFFICIENT_ALLOWANCE));
}

#[test]
fn negative_transfer_fails() {
    let s = setup();
    let r = s.client.try_transfer(&s.alice, &s.listed, &-1);
    assert_eq!(err(r), code(LESS_THAN_ZERO));
}

#[test]
fn uri_longer_than_the_cap_is_rejected() {
    let s = setup();
    let long = String::from_str(&s.e, &"x".repeat(MAX_URI_LEN as usize + 1));
    let r = s.client.try_set_offer_uri(&long, &s.admin);
    assert_eq!(err(r), OfferTokenError::UriTooLong.into());
    let max = String::from_str(&s.e, &"x".repeat(MAX_URI_LEN as usize));
    s.client.set_offer_uri(&max, &s.admin);
    assert_eq!(s.client.offer_uri(), max);
}
