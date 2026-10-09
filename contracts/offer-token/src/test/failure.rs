//! Every documented rejection is reachable through the client and carries
//! its code.
use soroban_sdk::{testutils::Address as _, Address};

use super::{code, err, setup};
use crate::OfferTokenError;

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
fn error_codes_are_the_documented_ones() {
    let codes = [
        (OfferTokenError::TransferNotWhitelisted, 6000),
        (OfferTokenError::InvalidAdminTransfer, 6001),
        (OfferTokenError::BurnDisabled, 6002),
        (OfferTokenError::NotAdmin, 6003),
        (OfferTokenError::NotXferAdmin, 6004),
        (OfferTokenError::NotPauser, 6005),
        (OfferTokenError::NotUpgrader, 6006),
        (OfferTokenError::InvalidSupply, 6007),
    ];
    for (e, n) in codes {
        assert_eq!(e as u32, n);
    }
}
