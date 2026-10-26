//! Repeating a batch converges to the same state, and the reads agree
//! with the writes.
use soroban_sdk::testutils::Events as _;

use super::{setup, COOLING_OFF, PRICE, SUCCESS_TOKENS, SUPPLY, T0};
use crate::SaleState;

#[test]
fn release_twice_and_with_duplicates_delivers_once() {
    let s = setup();
    s.successful();
    s.client.release(&s.investors(&[&s.alice, &s.alice]));
    assert_eq!(s.token.balance(&s.alice), SUCCESS_TOKENS);
    assert_eq!(s.client.totals().released_tokens, SUCCESS_TOKENS);
    s.client.release(&s.investors(&[&s.alice]));
    assert_eq!(s.token.balance(&s.alice), SUCCESS_TOKENS);
    assert_eq!(s.client.totals().released_tokens, SUCCESS_TOKENS);
    assert!(s
        .e
        .events()
        .all()
        .filter_by_contract(&s.id)
        .events()
        .is_empty());
}

#[test]
fn refund_twice_and_with_duplicates_returns_once() {
    let s = setup();
    s.failed();
    s.client.refund(&s.investors(&[&s.alice, &s.alice]));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    s.client.refund(&s.investors(&[&s.alice]));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    assert_eq!(s.client.totals().raised, 0);
}

#[test]
fn a_never_buyer_is_skipped_by_release_and_refund() {
    let s = setup();
    s.successful();
    s.client.release(&s.investors(&[&s.bob, &s.alice]));
    assert_eq!(s.token.balance(&s.bob), 0);
    assert_eq!(s.token.balance(&s.alice), SUCCESS_TOKENS);
    let s = setup();
    s.failed();
    s.client.refund(&s.investors(&[&s.bob]));
    assert_eq!(s.usdc.balance(&s.bob), SUPPLY * PRICE);
}

#[test]
fn close_reopen_close_ends_where_it_started() {
    let s = setup();
    s.buy(&s.alice, SUCCESS_TOKENS);
    s.client.close(&s.owner);
    s.client.reopen(&s.owner);
    s.client.close(&s.owner);
    assert_eq!(s.client.state(), SaleState::Closing);
}

#[test]
fn the_reads_agree_with_the_writes() {
    let s = setup();
    let first = s.buy(&s.alice, 10);
    s.advance(100);
    let second = s.buy(&s.alice, 5);
    let reservation = s.client.reservation(&s.alice).unwrap();
    assert_eq!((reservation.tokens, reservation.paid), (15, 15 * PRICE));
    assert_eq!(reservation.schema, 1);
    let p1 = reservation.purchases.get(first).unwrap();
    let p2 = reservation.purchases.get(second).unwrap();
    assert_eq!((p1.tokens, p1.paid, p1.at), (10, 10 * PRICE, T0));
    assert_eq!((p2.tokens, p2.paid, p2.at), (5, 5 * PRICE, T0 + 100));
    let totals = s.client.totals();
    assert_eq!(totals.supply, SUPPLY);
    assert_eq!((totals.sold_tokens, totals.raised), (15, 15 * PRICE));
    assert_eq!((totals.released_tokens, totals.withdrawn), (0, 0));
    assert_eq!(s.client.finalizable_at(), T0 + 100 + COOLING_OFF + 1);
    assert_eq!(s.client.withdrawable(), 0);
    assert_eq!(s.client.config().price, PRICE);
    assert_eq!(s.client.powers().owner, s.owner);
    assert_eq!(s.client.contracts().allowlist, s.allowlist.address);
    assert_eq!(s.client.schema_version(), 1);
}
