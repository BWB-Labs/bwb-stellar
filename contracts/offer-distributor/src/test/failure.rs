//! Every rejection the distributor document lists, reached through the
//! client with its code, and the rule under them all: a distribution pays
//! every recipient or nobody.
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

use super::{code, err, setup, trapped, InertToken, ReentrantToken, FUNDS, USDC};
use crate::OfferDistributorError;

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

/// The asset is whatever address the owner passes. An address with no
/// contract behind it fails the call and marks nothing; so does the
/// distributor's own address, which would be a call into itself.
#[test]
fn an_asset_address_with_no_token_behind_it_fails_the_distribution() {
    let s = setup();
    for not_a_token in [Address::generate(&s.e), s.id.clone()] {
        assert_eq!(
            err(s.client.try_distribute(
                &s.dist(1),
                &not_a_token,
                &s.payouts(&[(&s.alice, USDC)]),
                &s.owner
            )),
            trapped()
        );
    }
    assert!(!s.client.is_paid(&s.dist(1)));
}

/// The distributor cannot tell a real asset from a contract that answers
/// `transfer` and moves nothing: the id is recorded and the events are
/// emitted, with no money moved. Naming the asset is the owner's act, and
/// the owner's signature is what names it.
#[test]
fn an_asset_that_moves_nothing_still_records_the_distribution() {
    let s = setup();
    let inert = s.e.register(InertToken, ());
    s.client.distribute(
        &s.dist(1),
        &inert,
        &s.payouts(&[(&s.alice, USDC)]),
        &s.owner,
    );
    assert!(s.client.is_paid(&s.dist(1)));
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

/// An asset that calls back into the distributor during `transfer` is
/// stopped by the network, which refuses re-entry, and the whole
/// distribution fails.
#[test]
fn an_asset_that_calls_back_into_the_distributor_fails_the_distribution() {
    let s = setup();
    let reentrant = s.e.register(ReentrantToken, (s.id.clone(),));
    assert_eq!(
        err(s.client.try_distribute(
            &s.dist(1),
            &reentrant,
            &s.payouts(&[(&s.alice, USDC)]),
            &s.owner
        )),
        trapped()
    );
    assert!(!s.client.is_paid(&s.dist(1)));
}

/// The checks run in the documented order, so the first rule broken is the
/// one reported: pause before the caller, the caller before the list, the
/// list's size before the id, the id before the amounts.
#[test]
fn the_first_rule_broken_is_the_one_reported() {
    let s = setup();
    s.distribute(1, &[(&s.alice, USDC)]);
    // A paid id with an empty list: the size comes first.
    assert_eq!(
        err(s.try_distribute(1, &[])),
        OfferDistributorError::EmptyBatch.into()
    );
    // A paid id with a zero amount: the id comes first.
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, 0)])),
        OfferDistributorError::AlreadyPaid.into()
    );
    // A stranger sending an empty list: the caller comes first.
    assert_eq!(
        err(s
            .client
            .try_distribute(&s.dist(2), &s.usdc.address, &s.payouts(&[]), &s.stranger)),
        OfferDistributorError::NotOwner.into()
    );
    // Paused, and a stranger: the pause comes first.
    s.client.pause(&s.pauser);
    assert_eq!(
        err(s.client.try_distribute(
            &s.dist(2),
            &s.usdc.address,
            &s.payouts(&[(&s.alice, USDC)]),
            &s.stranger
        )),
        code(1000)
    );
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
