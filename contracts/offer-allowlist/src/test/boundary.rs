//! Edges of the cap, the batches, the month, the receipt numbers and the
//! restore list.
extern crate std;
use soroban_sdk::{testutils::Address as _, Address, Vec};

use super::{at, at_time, entry, err, period, setup};
use crate::{AllocationEntry, OfferAllowlistError, Receipt, MAX_BATCH, MAX_RESTORE};

#[test]
fn the_exact_cap_fits_and_one_more_does_not() {
    let s = setup();
    s.allow(&s.alice, 100);
    let r = s.try_consume(&s.alice, &101);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
    s.consume(&s.alice, &100);
    assert_eq!(s.client.remaining(&s.alice), 0);
    let r = s.try_consume(&s.alice, &1);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
}

#[test]
fn room_is_consumed_in_steps_up_to_the_cap() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &60);
    s.consume(&s.alice, &40);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
    let r = s.try_consume(&s.alice, &1);
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
    let r = s.try_consume(&s.alice, &1);
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

#[test]
fn restore_takes_twenty_receipts_and_not_twenty_one() {
    let s = setup();
    s.allow(&s.alice, 100);
    let receipts: std::vec::Vec<u32> = (0..=MAX_RESTORE).map(|_| s.consume(&s.alice, &1)).collect();
    let r = s.try_restore(&receipts);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
    s.restore(&receipts[..MAX_RESTORE as usize]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 1);
}

/// The worst case `MAX_RESTORE` is sized for: 20 receipts in 20 different
/// months, each emptying its month, so every receipt deletes two entries.
/// With the sale contract as the direct caller (no authorization entry)
/// the allowlist writes 40 of the transaction's 50 entries and emits 20 ×
/// 292 B of events; the rest is the sale contract's. The interface
/// document quotes these numbers; `-- --nocapture` prints the footprint.
#[test]
fn a_full_restore_leaves_ten_write_entries_to_the_sale() {
    let s = setup();
    s.allow(&s.alice, 1_000);
    let receipts: std::vec::Vec<u32> = (0..u64::from(MAX_RESTORE))
        .map(|i| {
            at(&s.e, 2024 + i / 12, 1 + i % 12, 15);
            s.consume(&s.alice, &10)
        })
        .collect();
    s.e.set_auths(&[]);
    s.e.cost_estimate().budget().reset_unlimited();
    s.restore(&receipts);
    let used = s.e.cost_estimate().resources();
    std::println!(
        "restore x{MAX_RESTORE}: {} write entries, {} footprint entries, {} B of events",
        used.write_entries,
        used.memory_read_entries,
        used.contract_events_size_bytes
    );
    assert_eq!(used.write_entries, 2 * MAX_RESTORE);
    assert!(used.memory_read_entries <= 50);
    assert_eq!(used.contract_events_size_bytes, 292 * MAX_RESTORE);
}

#[test]
fn receipt_numbers_start_at_zero_and_count_per_sale() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.allow(&s.bob, 100);
    let other = s.another_sale();
    assert_eq!(s.consume(&s.alice, &1), 0);
    assert_eq!(s.consume(&s.bob, &1), 1);
    // Another offering numbers its own purchases from 0.
    assert_eq!(other.buy(&s.id, &s.alice, &1), 0);
    assert_eq!(s.consume(&s.alice, &1), 2);
    assert_eq!(other.buy(&s.id, &s.bob, &1), 1);
    // A restore never frees a number for reuse.
    s.restore(&[2]);
    assert_eq!(s.consume(&s.alice, &1), 3);
}

/// The case that decided per-purchase receipts: buy 100, buy 100 again ten
/// days later, ask for cooling-off two days after that. Only the second
/// purchase comes back; the first keeps its usage and its receipt.
#[test]
fn cooling_off_returns_only_the_purchase_still_in_its_window() {
    let s = setup();
    s.allow(&s.alice, 1_000);
    at(&s.e, 2026, 10, 1);
    let first = s.consume(&s.alice, &100);
    at(&s.e, 2026, 10, 11);
    let second = s.consume(&s.alice, &100);
    at(&s.e, 2026, 10, 13);
    s.restore(&[second]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
    assert_eq!(s.client.remaining(&s.alice), 900);
    assert_eq!(
        s.client.receipt(&s.sale, &first),
        Some(Receipt {
            amount: 100,
            investor: s.alice.clone(),
            period: period(2026, 10),
        })
    );
    assert_eq!(s.client.receipt(&s.sale, &second), None);
}

#[test]
fn one_restore_takes_receipts_of_different_investors() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.allow(&s.bob, 100);
    let alice = s.consume(&s.alice, &30);
    let bob = s.consume(&s.bob, &40);
    s.consume(&s.bob, &5);
    s.restore(&[bob, alice]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
    assert_eq!(s.client.usage(&s.bob, &period(2026, 10)), 5);
}

#[test]
fn a_usage_restored_to_zero_is_removed() {
    let s = setup();
    s.allow(&s.alice, 100);
    let a = s.consume(&s.alice, &30);
    let b = s.consume(&s.alice, &20);
    let has_usage = || {
        s.e.as_contract(&s.id, || {
            s.e.storage()
                .persistent()
                .has(&crate::storage::DataKey::Usage(
                    s.alice.clone(),
                    period(2026, 10),
                ))
        })
    };
    s.restore(&[a]);
    assert!(has_usage());
    s.restore(&[b]);
    assert!(!has_usage());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
}

/// The period the stub sale's receipt was charged to.
fn charged_to(s: &super::Setup, receipt: u32) -> u32 {
    s.client.receipt(&s.sale, &receipt).unwrap().period
}

#[test]
fn the_last_second_of_a_month_and_the_first_of_the_next_are_different_periods() {
    let s = setup();
    s.allow(&s.alice, 100);
    at_time(&s.e, 2026, 10, 31, 23, 59, 59);
    let october = s.consume(&s.alice, &100);
    assert_eq!(s.client.remaining(&s.alice), 0);
    at_time(&s.e, 2026, 11, 1, 0, 0, 0);
    assert_eq!(s.client.remaining(&s.alice), 100);
    let november = s.consume(&s.alice, &100);
    assert_eq!(charged_to(&s, october), period(2026, 10));
    assert_eq!(charged_to(&s, november), period(2026, 11));
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 11)), 100);
}

#[test]
fn december_rolls_into_january_of_the_next_year() {
    let s = setup();
    s.allow(&s.alice, 100);
    at_time(&s.e, 2026, 12, 31, 23, 59, 59);
    let december = charged_to(&s, s.consume(&s.alice, &100));
    at(&s.e, 2027, 1, 1);
    let january = charged_to(&s, s.consume(&s.alice, &100));
    assert_eq!(december, period(2026, 12));
    assert_eq!(january, period(2027, 1));
    assert_eq!(january, december + 1);
}

#[test]
fn a_refund_across_a_month_boundary_restores_each_month_in_one_call() {
    // The grill scenario: buy in September and in October, refund both in
    // October. Each receipt goes back to its own month; October's room is
    // not inflated by September's refund.
    let s = setup();
    s.allow(&s.alice, 100);
    at(&s.e, 2026, 9, 20);
    let september = s.consume(&s.alice, &70);
    at(&s.e, 2026, 10, 15);
    let october = s.consume(&s.alice, &30);
    assert_eq!(s.client.remaining(&s.alice), 70);
    s.restore(&[september, october]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 9)), 0);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
    assert_eq!(s.client.remaining(&s.alice), 100);
}

#[test]
fn restoring_last_months_money_never_frees_this_months_room() {
    let s = setup();
    s.allow(&s.alice, 100);
    at(&s.e, 2026, 9, 20);
    let september = s.consume(&s.alice, &70);
    at(&s.e, 2026, 10, 15);
    s.consume(&s.alice, &100);
    s.restore(&[september]);
    assert_eq!(s.client.remaining(&s.alice), 0);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
}

#[test]
fn a_restore_gives_back_exactly_each_receipts_amount() {
    let s = setup();
    s.allow(&s.alice, 100);
    let a = s.consume(&s.alice, &60);
    let b = s.consume(&s.alice, &15);
    s.consume(&s.alice, &25);
    s.restore(&[a, b]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 25);
    assert_eq!(s.client.remaining(&s.alice), 75);
}

#[test]
fn a_cap_lowered_below_the_usage_leaves_no_room_and_keeps_the_usage() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &80);
    s.allow(&s.alice, 50);
    assert_eq!(s.client.remaining(&s.alice), 0);
    assert!(!s.client.can_invest(&s.alice, &1));
    let r = s.try_consume(&s.alice, &1);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
    assert_eq!(s.client.allocation(&s.alice).consumed, 80);
    // Next month the new cap applies in full.
    at(&s.e, 2026, 11, 1);
    assert_eq!(s.client.remaining(&s.alice), 50);
}

#[test]
fn a_cap_at_the_top_of_i128_leaves_the_room_arithmetic_sound() {
    let s = setup();
    s.allow(&s.alice, i128::MAX);
    assert!(s.client.can_invest(&s.alice, &i128::MAX));
    s.consume(&s.alice, &i128::MAX);
    assert_eq!(s.client.remaining(&s.alice), 0);
    let r = s.try_consume(&s.alice, &1);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
}
