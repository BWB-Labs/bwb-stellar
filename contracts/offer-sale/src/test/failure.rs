//! Every documented rejection is reachable through the client and carries
//! its code: the constructor's limits, the codes the allowlist and the
//! payment asset surface through `buy`, and the payout checks. The other
//! 62xx codes are reached by the module that owns the rule.
use soroban_sdk::{testutils::Address as _, Address};

use super::{config, err, setup, setup_preparing, Setup, PRICE, SUPPLY};
use crate::{Contracts, OfferSale, OfferSaleError, PowerHolders, SaleConfig};

fn register_with(s: &Setup, config: SaleConfig) {
    let any = Address::generate(&s.e);
    s.e.register(
        OfferSale,
        (
            PowerHolders {
                owner: any.clone(),
                pauser: any.clone(),
                upgrader: any.clone(),
            },
            Contracts {
                token: any.clone(),
                payment_asset: any.clone(),
                allowlist: any,
            },
            config,
        ),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #6216)")]
fn a_zero_price_is_rejected_at_construction() {
    let s = setup_preparing();
    register_with(
        &s,
        SaleConfig {
            price: 0,
            ..config()
        },
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #6216)")]
fn a_cooling_off_under_a_day_is_rejected_at_construction() {
    let s = setup_preparing();
    register_with(
        &s,
        SaleConfig {
            cooling_off_secs: 86_399,
            ..config()
        },
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #6216)")]
fn a_minimum_under_ten_percent_is_rejected_at_construction() {
    let s = setup_preparing();
    register_with(
        &s,
        SaleConfig {
            min_success_percent: 9,
            ..config()
        },
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #6216)")]
fn a_minimum_over_a_hundred_percent_is_rejected_at_construction() {
    let s = setup_preparing();
    register_with(
        &s,
        SaleConfig {
            min_success_percent: 101,
            ..config()
        },
    );
}

#[test]
fn the_limits_themselves_are_accepted() {
    let s = setup_preparing();
    register_with(
        &s,
        SaleConfig {
            price: 1,
            cooling_off_secs: 86_400,
            min_success_percent: 10,
        },
    );
    register_with(
        &s,
        SaleConfig {
            min_success_percent: 100,
            ..config()
        },
    );
}

#[test]
fn an_investor_not_on_the_allowlist_fails_with_investor_not_enabled() {
    let s = setup();
    let carol = Address::generate(&s.e);
    s.fund(&carol, PRICE);
    assert_eq!(err(s.try_buy(&carol, 1)), super::code(6100));
    assert_eq!(s.usdc.balance(&carol), PRICE);
}

#[test]
fn a_purchase_over_the_allowlist_room_fails_with_allocation_exceeded() {
    let s = setup();
    s.allow(&s.alice, 100 * PRICE);
    assert_eq!(err(s.try_buy(&s.alice, 101)), super::code(6101));
    s.buy(&s.alice, 100);
    assert_eq!(err(s.try_buy(&s.alice, 1)), super::code(6101));
}

#[test]
fn an_unfunded_investor_fails_in_the_payment_asset() {
    let s = setup();
    let carol = Address::generate(&s.e);
    s.allow(&carol, PRICE);
    assert!(s.try_buy(&carol, 1).is_err());
    assert!(s.client.reservation(&carol).is_none());
    assert_eq!(s.client.totals().sold_tokens, 0);
}

#[test]
fn a_payout_leg_must_be_positive() {
    let s = setup();
    s.successful();
    for amount in [0, -1] {
        let legs = s.payouts(&[(&s.owner, amount)]);
        assert_eq!(
            err(s.client.try_withdraw_payment(&legs, &s.owner)),
            OfferSaleError::NonPositiveAmount.into()
        );
    }
    assert_eq!(s.client.withdrawable(), 750 * PRICE);
}

#[test]
fn a_token_withdrawal_must_be_positive_and_within_the_balance() {
    let s = setup();
    s.failed();
    assert_eq!(
        err(s.client.try_withdraw_tokens(&s.owner, &0, &s.owner)),
        OfferSaleError::NonPositiveAmount.into()
    );
    // Beyond the balance: the token's own code, 100.
    assert_eq!(
        err(s
            .client
            .try_withdraw_tokens(&s.owner, &(SUPPLY + 1), &s.owner)),
        super::code(100)
    );
}

/// Story 35: an investor whose payment-asset balance the issuer froze
/// fails the whole batch, so nobody in it is half served; the backend
/// leaves them out and retries the rest.
#[test]
fn a_frozen_balance_in_a_refund_batch_reverts_the_batch() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.buy(&s.bob, 10);
    s.client.cancel(&s.owner);
    s.usdc_admin.set_authorized(&s.alice, &false);
    assert!(s
        .client
        .try_refund(&s.investors(&[&s.bob, &s.alice]))
        .is_err());
    assert_eq!(s.usdc.balance(&s.bob), (SUPPLY - 10) * PRICE);
    assert!(s.client.reservation(&s.bob).is_some());
    s.usdc_admin.set_authorized(&s.alice, &true);
    s.client.refund(&s.investors(&[&s.bob, &s.alice]));
    assert_eq!(s.usdc.balance(&s.bob), SUPPLY * PRICE);
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
}

/// A refund batch moves money and receipts together for every investor
/// listed, or not at all: the batch is one transaction.
#[test]
fn a_refund_batch_pays_every_investor_whole() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.buy(&s.bob, 10);
    s.client.cancel(&s.owner);
    s.client.refund(&s.investors(&[&s.alice, &s.bob]));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    assert_eq!(s.usdc.balance(&s.bob), SUPPLY * PRICE);
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert!(s
        .allowlist
        .receipt_book(&s.id, &s.alice)
        .unwrap()
        .receipts
        .is_empty());
    assert!(s
        .allowlist
        .receipt_book(&s.id, &s.bob)
        .unwrap()
        .receipts
        .is_empty());
}
