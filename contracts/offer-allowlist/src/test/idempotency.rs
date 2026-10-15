//! Repeating a write converges to the same state, and the read path agrees
//! with the write path at the same ledger time.
use soroban_sdk::{testutils::Events as _, vec};

use super::{controller, entry, period, setup};

#[test]
fn writing_the_same_allocations_twice_keeps_the_usage() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &40, &s.hot_key);
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
    s.client.consume(&s.alice, &40, &s.hot_key);
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
    s.client.grant_role(&s.hot_key, &controller(&s.e), &s.admin);
    assert!(s
        .e
        .events()
        .all()
        .filter_by_contract(&s.id)
        .events()
        .is_empty());
    assert_eq!(s.client.get_role_member_count(&controller(&s.e)), 1);
}

#[test]
fn remaining_and_can_invest_agree_with_consume() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &35, &s.hot_key);
    let room = s.client.remaining(&s.alice);
    assert_eq!(room, 65);
    assert!(s.client.can_invest(&s.alice, &room));
    assert!(!s.client.can_invest(&s.alice, &(room + 1)));
    assert!(s
        .client
        .try_consume(&s.alice, &(room + 1), &s.hot_key)
        .is_err());
    assert_eq!(
        s.client.consume(&s.alice, &room, &s.hot_key),
        period(2026, 10)
    );
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
