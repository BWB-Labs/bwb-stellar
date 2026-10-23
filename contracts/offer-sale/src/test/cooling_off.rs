//! The investor's right to undo a purchase: per purchase, whole purchases,
//! while the offering is Active or Closing, never after it is decided.
use soroban_sdk::testutils::Ledger as _;

use super::{err, setup, COOLING_OFF, PRICE, SUPPLY};
use crate::{OfferSaleError, SaleState};

#[test]
fn a_refund_inside_the_window_returns_the_purchase() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.advance(COOLING_OFF);
    s.client.cooling_off_refund(&s.alice);
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    assert!(s.client.reservation(&s.alice).is_none());
    assert_eq!(s.allowlist.remaining(&s.alice), SUPPLY * PRICE);
    let totals = s.client.totals();
    assert_eq!((totals.sold_tokens, totals.raised), (0, 0));
}

/// Buy, buy again later, ask for cooling-off: only the purchase still
/// inside its own window comes back; the older one stays reserved.
#[test]
fn only_purchases_still_in_their_window_come_back() {
    let s = setup();
    let first = s.buy(&s.alice, 10);
    s.advance(6 * 86_400);
    let second = s.buy(&s.alice, 5);
    s.advance(2 * 86_400);
    s.client.cooling_off_refund(&s.alice);
    let reservation = s.client.reservation(&s.alice).unwrap();
    assert_eq!((reservation.tokens, reservation.paid), (10, 10 * PRICE));
    assert!(reservation.purchases.contains_key(first));
    assert!(!reservation.purchases.contains_key(second));
    assert!(s.allowlist.receipt(&s.id, &s.alice, &first).is_some());
    assert!(s.allowlist.receipt(&s.id, &s.alice, &second).is_none());
    assert_eq!(s.client.totals().sold_tokens, 10);
    assert_eq!(s.usdc.balance(&s.alice), (SUPPLY - 10) * PRICE);
}

/// Story 7: a purchase charged to September, undone in October, gives
/// September's room back and leaves October's alone.
#[test]
fn a_refund_across_a_month_boundary_restores_the_month_charged() {
    let s = setup();
    // T0 is 2026-10-15; go back to 2026-09-28 for the purchase.
    s.e.ledger().set_timestamp(super::T0 - 17 * 86_400);
    s.buy(&s.alice, 10);
    let september = s.allowlist.current_period();
    s.advance(4 * 86_400);
    let october = s.allowlist.current_period();
    assert_eq!(october, september + 1);
    s.buy(&s.alice, 5);
    s.client.cooling_off_refund(&s.alice);
    assert_eq!(s.allowlist.usage(&s.alice, &september), 0);
    assert_eq!(s.allowlist.usage(&s.alice, &october), 0);
    assert!(s.client.reservation(&s.alice).is_none());
}

#[test]
fn after_the_window_fails_with_cooling_off_closed() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.advance(COOLING_OFF + 1);
    assert_eq!(
        err(s.client.try_cooling_off_refund(&s.alice)),
        OfferSaleError::CoolingOffClosed.into()
    );
    assert_eq!(s.client.reservation(&s.alice).unwrap().tokens, 10);
}

/// Above the minimum the offering stays in Closing; below it, the refund
/// reopens buying (the `state` module covers that case).
#[test]
fn works_in_closing() {
    let s = setup();
    s.buy(&s.alice, 760);
    s.buy(&s.bob, 10);
    s.client.close(&s.owner);
    s.client.cooling_off_refund(&s.bob);
    assert!(s.client.reservation(&s.bob).is_none());
    assert_eq!(s.client.state(), SaleState::Closing);
}

#[test]
fn with_nothing_reserved_fails_with_nothing_reserved() {
    let s = setup();
    assert_eq!(
        err(s.client.try_cooling_off_refund(&s.bob)),
        OfferSaleError::NothingReserved.into()
    );
}

#[test]
fn after_the_offering_is_decided_fails_with_not_open() {
    let s = setup();
    s.successful();
    assert_eq!(
        err(s.client.try_cooling_off_refund(&s.alice)),
        OfferSaleError::NotOpen.into()
    );
    let s = setup();
    s.failed();
    assert_eq!(
        err(s.client.try_cooling_off_refund(&s.alice)),
        OfferSaleError::NotOpen.into()
    );
}

/// Parity with Base: nothing can be undone once the tokens are delivered.
/// Here release happens only in Successful, where cooling-off is already
/// over, so the two never meet.
#[test]
fn after_release_fails_with_not_open() {
    let s = setup();
    s.successful();
    s.client.release(&s.investors(&[&s.alice]));
    assert_eq!(
        err(s.client.try_cooling_off_refund(&s.alice)),
        OfferSaleError::NotOpen.into()
    );
}

#[test]
fn twenty_purchases_come_back_in_one_call() {
    let s = setup();
    for _ in 0..20 {
        s.buy(&s.alice, 1);
    }
    s.client.cooling_off_refund(&s.alice);
    assert!(s.client.reservation(&s.alice).is_none());
    let book = s.allowlist.receipt_book(&s.id, &s.alice).unwrap();
    assert_eq!((book.next, book.receipts.len()), (20, 0));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
}

#[test]
fn a_second_request_with_nothing_left_in_a_window_fails() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.advance(COOLING_OFF + 1);
    s.buy(&s.alice, 5);
    s.client.cooling_off_refund(&s.alice);
    assert_eq!(
        err(s.client.try_cooling_off_refund(&s.alice)),
        OfferSaleError::CoolingOffClosed.into()
    );
    assert_eq!(s.client.reservation(&s.alice).unwrap().tokens, 10);
}
