//! The owner takes any balance out of the distributor, and only into its
//! own account.
use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Event as _};

use super::{code, emitted, emitters, err, setup, FUNDS, SUPPLY, USDC};
use crate::events::Withdrawn;
use crate::OfferHoldingsDistributorError;

/// The asset's code for a transfer above the sender's balance.
const BALANCE: u32 = 10;

#[test]
fn the_owner_withdraws_into_its_own_account() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    s.client.withdraw(&s.usdc.address, &(300 * USDC), &s.owner);
    let withdrawn = Withdrawn {
        asset: s.usdc.address.clone(),
        amount: 300 * USDC,
    };
    assert_eq!(emitted(&s.e, &s.id), [withdrawn.to_xdr(&s.e, &s.id)]);
    // The asset's `transfer` comes first.
    assert_eq!(emitters(&s.e), [s.usdc.address.clone(), s.id.clone()]);
    assert_eq!(s.pot(), 700 * USDC);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - 700 * USDC);
}

#[test]
fn only_the_owner_withdraws() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    for caller in [&s.pauser, &s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_withdraw(&s.usdc.address, &USDC, caller)),
            OfferHoldingsDistributorError::NotOwner.into()
        );
    }
    assert_eq!(s.pot(), SUPPLY * USDC);
}

#[test]
fn a_withdrawal_that_is_not_positive_is_refused() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    for amount in [0, -1, i128::MIN] {
        assert_eq!(
            err(s.client.try_withdraw(&s.usdc.address, &amount, &s.owner)),
            OfferHoldingsDistributorError::NonPositiveAmount.into()
        );
    }
}

#[test]
fn a_withdrawal_above_the_balance_fails_with_the_assets_code() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    assert_eq!(
        err(s
            .client
            .try_withdraw(&s.usdc.address, &(SUPPLY * USDC + 1), &s.owner)),
        code(BALANCE)
    );
    assert_eq!(s.pot(), SUPPLY * USDC);
}

/// A withdrawal moves no record: what a holder earned is still owed.
#[test]
fn a_withdrawal_changes_no_holders_record() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.client
        .withdraw(&s.usdc.address, &(SUPPLY * USDC), &s.owner);
    assert_eq!(s.client.claimable(&s.alice), 100 * USDC);
    assert_eq!(s.client.cumulative_per_quota(), USDC * crate::MAGNITUDE);
}

/// Another asset sent here by mistake leaves the same way.
#[test]
fn a_stray_asset_can_be_withdrawn() {
    let s = setup();
    let other =
        s.e.register_stellar_asset_contract_v2(Address::generate(&s.e));
    let other_id = other.address();
    StellarAssetClient::new(&s.e, &other_id).mint(&s.id, &(5 * USDC));
    s.client.withdraw(&other_id, &(5 * USDC), &s.owner);
    let balance = soroban_sdk::token::TokenClient::new(&s.e, &other_id).balance(&s.owner);
    assert_eq!(balance, 5 * USDC);
}

#[test]
fn a_withdrawal_works_while_paused() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    s.client.pause(&s.pauser);
    s.client
        .withdraw(&s.usdc.address, &(SUPPLY * USDC), &s.owner);
    assert_eq!(s.pot(), 0);
}
