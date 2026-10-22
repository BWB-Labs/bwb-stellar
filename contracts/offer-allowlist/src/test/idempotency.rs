//! Repeating a write converges to the same state, and the read path agrees
//! with the write path at the same ledger time.
use soroban_sdk::{testutils::Events as _, vec};

use super::{allocator, entry, period, setup};
use crate::Receipt;

#[test]
fn writing_the_same_allocations_twice_keeps_the_usage() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &40);
    s.allow(&s.alice, 100);
    s.allow(&s.alice, 100);
    let view = s.client.allocation(&s.alice);
    assert_eq!(view.max, 100);
    assert!(view.enabled);
    assert_eq!(view.consumed, 40);
    assert_eq!(s.client.remaining(&s.alice), 60);
}

#[test]
fn an_empty_batch_is_a_no_op() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.set_allocations(&vec![&s.e], &s.hot_key);
    assert_eq!(s.client.allocation(&s.alice).max, 100);
    assert!(s
        .e
        .events()
        .all()
        .filter_by_contract(&s.id)
        .events()
        .is_empty());
}

#[test]
fn the_last_duplicate_in_a_batch_wins() {
    let s = setup();
    s.client.set_allocations(
        &vec![&s.e, entry(&s.alice, 100, true), entry(&s.alice, 50, false)],
        &s.hot_key,
    );
    let view = s.client.allocation(&s.alice);
    assert_eq!(view.max, 50);
    assert!(!view.enabled);
}

#[test]
fn disabling_keeps_the_cap_and_the_usage() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &40);
    s.client
        .set_allocations(&vec![&s.e, entry(&s.alice, 100, false)], &s.hot_key);
    let view = s.client.allocation(&s.alice);
    assert_eq!(view.max, 100);
    assert_eq!(view.consumed, 40);
    assert_eq!(s.client.remaining(&s.alice), 0);
    s.allow(&s.alice, 100);
    assert_eq!(s.client.remaining(&s.alice), 60);
}

#[test]
fn granting_a_held_role_is_silent() {
    let s = setup();
    s.client.grant_role(&s.hot_key, &allocator(&s.e), &s.admin);
    assert!(s
        .e
        .events()
        .all()
        .filter_by_contract(&s.id)
        .events()
        .is_empty());
    assert_eq!(s.client.get_role_member_count(&allocator(&s.e)), 1);
}

#[test]
fn remaining_and_can_invest_agree_with_consume() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &35);
    let room = s.client.remaining(&s.alice);
    assert_eq!(room, 65);
    assert!(s.client.can_invest(&s.alice, &room));
    assert!(!s.client.can_invest(&s.alice, &(room + 1)));
    assert!(s.try_consume(&s.alice, &(room + 1)).is_err());
    s.consume(&s.alice, &room);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
    assert_eq!(s.client.remaining(&s.alice), 0);
    assert!(!s.client.can_invest(&s.alice, &1));
}

#[test]
fn can_invest_is_false_for_a_non_positive_amount() {
    let s = setup();
    s.allow(&s.alice, 100);
    assert!(!s.client.can_invest(&s.alice, &0));
    assert!(!s.client.can_invest(&s.alice, &-1));
}

#[test]
fn the_receipt_getter_follows_consume_and_restore() {
    let s = setup();
    s.allow(&s.alice, 100);
    assert_eq!(s.receipt(&s.alice, 0), None);
    assert!(s.client.receipt_book(&s.sale, &s.alice).is_none());
    let n = s.consume(&s.alice, &35);
    let expected = Receipt {
        amount: 35,
        period: period(2026, 10),
    };
    assert_eq!(s.receipt(&s.alice, n), Some(expected));
    // Under its own sale and investor only.
    assert_eq!(s.client.receipt(&s.hot_key, &s.alice, &n), None);
    assert_eq!(s.receipt(&s.bob, n), None);
    let all = s.client.receipt_book(&s.sale, &s.alice).unwrap();
    assert_eq!((all.next, all.receipts.len()), (1, 1));
    s.restore(&s.alice, &[n]);
    assert_eq!(s.receipt(&s.alice, n), None);
}

/// A failed offering refunds the whole investor: every receipt back to its
/// own month in one call, the book kept with its next number.
#[test]
fn restore_all_gives_back_every_live_receipt_across_months() {
    let s = setup();
    s.allow(&s.alice, 1_000);
    super::at(&s.e, 2026, 9, 20);
    s.consume(&s.alice, &70);
    super::at(&s.e, 2026, 10, 15);
    s.consume(&s.alice, &10);
    let already = s.consume(&s.alice, &5);
    s.restore(&s.alice, &[already]);
    assert_eq!(s.restore_all(&s.alice), 2);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 9)), 0);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
    let book = s.client.receipt_book(&s.sale, &s.alice).unwrap();
    assert_eq!((book.next, book.receipts.len()), (3, 0));
    assert_eq!(s.client.remaining(&s.alice), 1_000);
}

#[test]
fn a_disabled_investor_can_still_be_refunded() {
    let s = setup();
    s.allow(&s.alice, 100);
    let n = s.consume(&s.alice, &40);
    s.client
        .set_allocations(&vec![&s.e, entry(&s.alice, 100, false)], &s.hot_key);
    s.restore(&s.alice, &[n]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
    assert_eq!(s.receipt(&s.alice, n), None);
}
