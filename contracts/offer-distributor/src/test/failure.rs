//! Every rejection the distributor document lists, reached through the
//! client with its code, and the rule under them all: a distribution pays
//! every recipient or nobody.
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

use super::{code, err, setup, FUNDS, USDC};

/// The asset contract's own codes, which surface unchanged.
const BALANCE: u32 = 10;
const BALANCE_DEAUTHORIZED: u32 = 11;
const TRUSTLINE_MISSING: u32 = 13;

/// A recipient whose balance the issuer froze fails the whole call: the
/// recipients listed before it are not paid either.
#[test]
fn a_frozen_recipient_fails_the_whole_distribution() {
    let s = setup();
    s.usdc_admin.set_authorized(&s.bob, &false);
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, USDC), (&s.bob, USDC)])),
        code(BALANCE_DEAUTHORIZED)
    );
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
    s.usdc_admin.set_authorized(&s.bob, &true);
    s.distribute(1, &[(&s.alice, USDC), (&s.bob, USDC)]);
    assert_eq!(s.usdc.balance(&s.bob), USDC);
}

/// A classic account that never opened a trustline for the asset cannot
/// receive it, and nobody else in the list is paid.
#[test]
fn a_recipient_without_a_trustline_fails_the_whole_distribution() {
    let s = setup();
    let ready = s.create_account_with_trustline(1);
    let no_trustline = s.create_account(2);
    assert_eq!(
        err(s.try_distribute(1, &[(&ready, USDC), (&no_trustline, USDC)])),
        code(TRUSTLINE_MISSING)
    );
    assert_eq!(s.usdc.balance(&ready), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
    s.distribute(1, &[(&ready, USDC)]);
    assert_eq!(s.usdc.balance(&ready), USDC);
}

#[test]
fn a_recipient_account_that_does_not_exist_fails_the_whole_distribution() {
    let s = setup();
    let ready = s.create_account_with_trustline(1);
    let nobody = s.account(3);
    assert_eq!(
        err(s.try_distribute(1, &[(&ready, USDC), (&nobody, USDC)])),
        code(TRUSTLINE_MISSING)
    );
    assert_eq!(s.usdc.balance(&ready), 0);
}

/// The owner pays from its own account, so a list worth more than it
/// holds pays nobody.
#[test]
fn an_owner_without_enough_balance_pays_nobody() {
    let s = setup();
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, FUNDS), (&s.bob, 1)])),
        code(BALANCE)
    );
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
    assert!(!s.client.is_paid(&s.dist(1)));
}

#[test]
fn a_frozen_owner_pays_nobody() {
    let s = setup();
    s.usdc_admin.set_authorized(&s.owner, &false);
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, USDC)])),
        code(BALANCE_DEAUTHORIZED)
    );
    assert_eq!(s.usdc.balance(&s.alice), 0);
}

/// The asset is whatever address the owner passes. One that is no token
/// contract fails the call and marks nothing.
#[test]
fn an_asset_that_is_not_a_token_fails_the_distribution() {
    let s = setup();
    let not_a_token = Address::generate(&s.e);
    assert!(s
        .client
        .try_distribute(
            &s.dist(1),
            &not_a_token,
            &s.payouts(&[(&s.alice, USDC)]),
            &s.owner
        )
        .is_err());
    assert!(!s.client.is_paid(&s.dist(1)));
}

/// The same distributor pays any asset: the asset is chosen per
/// distribution, and each asset's balances move separately.
#[test]
fn one_distributor_pays_in_more_than_one_asset() {
    let s = setup();
    let issuer = Address::generate(&s.e);
    let other = s.e.register_stellar_asset_contract_v2(issuer);
    let other_admin = soroban_sdk::token::StellarAssetClient::new(&s.e, &other.address());
    let other_token = soroban_sdk::token::TokenClient::new(&s.e, &other.address());
    other_admin.mint(&s.owner, &(10 * USDC));
    s.distribute(1, &[(&s.alice, USDC)]);
    s.client.distribute(
        &s.dist(2),
        &other.address(),
        &s.payouts(&[(&s.alice, 4 * USDC)]),
        &s.owner,
    );
    assert_eq!(s.usdc.balance(&s.alice), USDC);
    assert_eq!(other_token.balance(&s.alice), 4 * USDC);
    assert_eq!(other_token.balance(&s.owner), 6 * USDC);
}
