//! A deposit pulls the money in and raises the per-quota counter. It
//! touches no holder.
use super::{err, setup, setup_with_supply, FUNDS, SUPPLY, USDC};
use crate::{OfferHoldingsDistributorError, MAGNITUDE};

#[test]
fn a_deposit_pulls_the_money_and_raises_the_counter() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    assert_eq!(s.pot(), SUPPLY * USDC);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - SUPPLY * USDC);
    assert_eq!(s.client.cumulative_per_quota(), USDC * MAGNITUDE);
}

#[test]
fn deposits_add_up() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    s.deposit(2 * SUPPLY * USDC);
    assert_eq!(s.client.cumulative_per_quota(), 3 * USDC * MAGNITUDE);
    assert_eq!(s.pot(), 3 * SUPPLY * USDC);
}

#[test]
fn a_deposit_pays_nobody() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.client.credited(&s.alice), 0);
    assert_eq!(s.client.claimable(&s.alice), 100 * USDC);
}

#[test]
fn a_deposit_that_is_not_positive_is_refused() {
    let s = setup();
    for amount in [0, -1, i128::MIN] {
        assert_eq!(
            err(s.client.try_deposit(&amount, &s.owner)),
            OfferHoldingsDistributorError::NonPositiveAmount.into()
        );
    }
    assert_eq!(s.pot(), 0);
    assert_eq!(s.client.cumulative_per_quota(), 0);
}

/// With every quota burned there is nobody to divide a deposit among.
#[test]
fn a_deposit_with_no_supply_is_refused() {
    let s = setup();
    s.token.burn(&s.desk, &SUPPLY);
    assert_eq!(
        err(s.client.try_deposit(&USDC, &s.owner)),
        OfferHoldingsDistributorError::NoSupply.into()
    );
    assert_eq!(s.pot(), 0);
}

#[test]
fn a_deposit_too_large_for_the_counter_is_refused() {
    let s = setup();
    assert_eq!(
        err(s.client.try_deposit(&i128::MAX, &s.owner)),
        OfferHoldingsDistributorError::AmountOverflow.into()
    );
    // The largest amount the scaling takes is refused by the asset instead:
    // the owner does not have it.
    assert!(s
        .client
        .try_deposit(&(i128::MAX / MAGNITUDE), &s.owner)
        .is_err());
    assert_eq!(s.client.cumulative_per_quota(), 0);
}

/// Divisions round down. 100.0000001 USDC over 3 quotas is 33.3333333 each;
/// the last stroops stay in the distributor.
#[test]
fn rounding_goes_down_and_the_dust_stays() {
    let s = setup_with_supply(3);
    s.give(&s.alice, 1);
    s.give(&s.bob, 2);
    s.deposit(100 * USDC + 1);
    assert_eq!(s.client.claimable(&s.alice), 333_333_333);
    assert_eq!(s.client.claimable(&s.bob), 666_666_667);
    s.claim(&[&s.alice, &s.bob]);
    assert_eq!(s.pot(), 1);
}

/// A deposit smaller than the supply still raises the counter: that is
/// what the magnitude is for.
#[test]
fn a_deposit_smaller_than_the_supply_still_counts() {
    let s = setup();
    s.give(&s.alice, 500);
    s.deposit(10);
    assert_eq!(s.client.cumulative_per_quota(), MAGNITUDE / 100);
    assert_eq!(s.client.claimable(&s.alice), 5);
}

/// The quotas that exist at each deposit share it: quotas burned before a
/// deposit take no part of it.
#[test]
fn a_deposit_divides_by_the_supply_of_its_moment() {
    let s = setup();
    s.give(&s.alice, 500);
    s.token.burn(&s.desk, &500);
    s.deposit(500 * USDC);
    assert_eq!(s.client.claimable(&s.alice), 500 * USDC);
}
