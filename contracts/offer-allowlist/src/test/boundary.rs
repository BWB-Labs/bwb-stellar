//! Edges of the cap, the batches, the month and the restore list.
use soroban_sdk::{testutils::Address as _, vec, Address, Vec};

use super::{at, at_time, entry, err, give_back, period, setup};
use crate::{AllocationEntry, OfferAllowlistError, RestoreEntry, MAX_BATCH};

#[test]
fn the_exact_cap_fits_and_one_more_does_not() {
    let s = setup();
    s.allow(&s.alice, 100);
    let r = s.client.try_consume(&s.alice, &101, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
    s.client.consume(&s.alice, &100, &s.hot_key);
    assert_eq!(s.client.remaining(&s.alice), 0);
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
}

#[test]
fn room_is_consumed_in_steps_up_to_the_cap() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &60, &s.hot_key);
    s.client.consume(&s.alice, &40, &s.hot_key);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
}

#[test]
fn a_cap_of_zero_is_valid_and_means_no_room() {
    let s = setup();
    s.allow(&s.alice, 0);
    let view = s.client.allocation(&s.alice);
    assert!(view.enabled);
    assert_eq!(view.max, 0);
    assert_eq!(s.client.remaining(&s.alice), 0);
    assert!(!s.client.can_invest(&s.alice, &1));
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
}

fn investors(s: &super::Setup, n: u32) -> Vec<AllocationEntry> {
    let mut v = Vec::new(&s.e);
    for _ in 0..n {
        v.push_back(entry(&Address::generate(&s.e), 1, true));
    }
    v
}

#[test]
fn set_allocations_takes_thirty_entries_and_not_thirty_one() {
    let s = setup();
    s.client
        .set_allocations(&investors(&s, MAX_BATCH), &s.hot_key);
    let r = s
        .client
        .try_set_allocations(&investors(&s, MAX_BATCH + 1), &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
}

fn one_each(s: &super::Setup, n: u32) -> Vec<RestoreEntry> {
    let mut v = Vec::new(&s.e);
    for _ in 0..n {
        v.push_back(give_back(period(2026, 10), 1));
    }
    v
}

#[test]
fn restore_takes_thirty_entries_and_not_thirty_one() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &100, &s.hot_key);
    let r = s
        .client
        .try_restore(&s.alice, &one_each(&s, MAX_BATCH + 1), &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
    s.client
        .restore(&s.alice, &one_each(&s, MAX_BATCH), &s.hot_key);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100 - 30);
}

#[test]
fn the_last_second_of_a_month_and_the_first_of_the_next_are_different_periods() {
    let s = setup();
    s.allow(&s.alice, 100);
    at_time(&s.e, 2026, 10, 31, 23, 59, 59);
    assert_eq!(
        s.client.consume(&s.alice, &100, &s.hot_key),
        period(2026, 10)
    );
    assert_eq!(s.client.remaining(&s.alice), 0);
    at_time(&s.e, 2026, 11, 1, 0, 0, 0);
    assert_eq!(s.client.remaining(&s.alice), 100);
    assert_eq!(
        s.client.consume(&s.alice, &100, &s.hot_key),
        period(2026, 11)
    );
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 11)), 100);
}

#[test]
fn december_rolls_into_january_of_the_next_year() {
    let s = setup();
    s.allow(&s.alice, 100);
    at_time(&s.e, 2026, 12, 31, 23, 59, 59);
    let december = s.client.consume(&s.alice, &100, &s.hot_key);
    at(&s.e, 2027, 1, 1);
    let january = s.client.consume(&s.alice, &100, &s.hot_key);
    assert_eq!(december, period(2026, 12));
    assert_eq!(january, period(2027, 1));
    assert_eq!(january, december + 1);
}

#[test]
fn a_refund_across_a_month_boundary_restores_each_month_in_one_call() {
    // The grill scenario: buy in September and in October, refund both in
    // October. Each month gets its own amount back; October's room is not
    // inflated by September's refund.
    let s = setup();
    s.allow(&s.alice, 100);
    at(&s.e, 2026, 9, 20);
    let september = s.client.consume(&s.alice, &70, &s.hot_key);
    at(&s.e, 2026, 10, 15);
    let october = s.client.consume(&s.alice, &30, &s.hot_key);
    assert_eq!(s.client.remaining(&s.alice), 70);
    s.client.restore(
        &s.alice,
        &vec![&s.e, give_back(september, 70), give_back(october, 30)],
        &s.hot_key,
    );
    assert_eq!(s.client.usage(&s.alice, &september), 0);
    assert_eq!(s.client.usage(&s.alice, &october), 0);
    assert_eq!(s.client.remaining(&s.alice), 100);
}

#[test]
fn restoring_last_months_money_never_frees_this_months_room() {
    let s = setup();
    s.allow(&s.alice, 100);
    at(&s.e, 2026, 9, 20);
    let september = s.client.consume(&s.alice, &70, &s.hot_key);
    at(&s.e, 2026, 10, 15);
    s.client.consume(&s.alice, &100, &s.hot_key);
    s.client
        .restore(&s.alice, &vec![&s.e, give_back(september, 70)], &s.hot_key);
    assert_eq!(s.client.remaining(&s.alice), 0);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
}

#[test]
fn restore_of_the_exact_usage_passes_and_one_above_fails() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &60, &s.hot_key);
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(period(2026, 10), 61)],
        &s.hot_key,
    );
    assert_eq!(err(r), OfferAllowlistError::RestoreExceedsUsage.into());
    s.client.restore(
        &s.alice,
        &vec![&s.e, give_back(period(2026, 10), 60)],
        &s.hot_key,
    );
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
}

#[test]
fn a_restore_list_is_summed_per_period_before_the_check() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &60, &s.hot_key);
    let p = period(2026, 10);
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(p, 30), give_back(p, 31)],
        &s.hot_key,
    );
    assert_eq!(err(r), OfferAllowlistError::RestoreExceedsUsage.into());
    assert_eq!(s.client.usage(&s.alice, &p), 60);
    s.client.restore(
        &s.alice,
        &vec![&s.e, give_back(p, 30), give_back(p, 30)],
        &s.hot_key,
    );
    assert_eq!(s.client.usage(&s.alice, &p), 0);
}

#[test]
fn a_restore_for_the_current_period_passes_and_one_after_it_fails() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &10, &s.hot_key);
    let now = s.client.current_period();
    let r = s
        .client
        .try_restore(&s.alice, &vec![&s.e, give_back(now + 1, 1)], &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::InvalidPeriod.into());
    s.client
        .restore(&s.alice, &vec![&s.e, give_back(now, 10)], &s.hot_key);
}

#[test]
fn a_cap_lowered_below_the_usage_leaves_no_room_and_keeps_the_usage() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &80, &s.hot_key);
    s.allow(&s.alice, 50);
    assert_eq!(s.client.remaining(&s.alice), 0);
    assert!(!s.client.can_invest(&s.alice, &1));
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
    assert_eq!(s.client.allocation(&s.alice).consumed, 80);
    // Next month the new cap applies in full.
    at(&s.e, 2026, 11, 1);
    assert_eq!(s.client.remaining(&s.alice), 50);
}

#[test]
fn a_restore_sum_that_overflows_fails_with_invalid_amount() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &10, &s.hot_key);
    let p = period(2026, 10);
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(p, i128::MAX), give_back(p, 1)],
        &s.hot_key,
    );
    assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
}

#[test]
fn a_cap_at_the_top_of_i128_leaves_the_room_arithmetic_sound() {
    let s = setup();
    s.allow(&s.alice, i128::MAX);
    assert!(s.client.can_invest(&s.alice, &i128::MAX));
    s.client.consume(&s.alice, &i128::MAX, &s.hot_key);
    assert_eq!(s.client.remaining(&s.alice), 0);
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
}
