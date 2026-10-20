//! Edges of the cap, the batches, the month, the receipt numbers and the
//! restore list.
extern crate std;
use soroban_sdk::{
    contract, contracterror, contractimpl, testutils::Address as _, Address, Env, Vec,
};

use super::{at, at_time, entry, err, period, setup, SaleStubClient};
use crate::{AllocationEntry, OfferAllowlistError, Receipt, MAX_BATCH, MAX_RECEIPTS, MAX_RESTORE};

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
    assert_eq!(MAX_BATCH, 30);
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
    assert_eq!(MAX_RESTORE, 20);
    let s = setup();
    s.allow(&s.alice, 100);
    let receipts: std::vec::Vec<u32> = (0..=MAX_RESTORE).map(|_| s.consume(&s.alice, &1)).collect();
    let r = s.try_restore(&s.alice, &receipts);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
    s.restore(&s.alice, &receipts[..MAX_RESTORE as usize]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 1);
}

/// The worst case `MAX_RESTORE` is sized for: 20 receipts in 20 different
/// months, each emptying its month. The allowlist writes the investor's
/// receipts entry once and one usage entry per month, 21 entries (of the
/// 200 a transaction may write), and emits 20 × 292 B of events. The
/// interface document quotes these numbers; `-- --nocapture` prints them.
#[test]
fn a_full_restore_writes_one_entry_per_month_plus_the_receipts() {
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
    s.restore(&s.alice, &receipts);
    let used = s.e.cost_estimate().resources();
    std::println!(
        "restore x{MAX_RESTORE}: {} write entries, {} footprint entries, {} B written, {} B of events",
        used.write_entries,
        used.memory_read_entries,
        used.write_bytes,
        used.contract_events_size_bytes
    );
    assert_eq!(used.write_entries, MAX_RESTORE + 1);
    // The 21 written entries, plus 4 a restore through the stub reads
    // whatever it gives back. The test environment loads no contract code;
    // on the network the code entries add to the footprint.
    assert_eq!(used.memory_read_entries, (MAX_RESTORE + 1) + 4);
    assert_eq!(used.contract_events_size_bytes, 292 * MAX_RESTORE);
}

/// `MAX_RECEIPTS` bounds the one entry every purchase and refund of an
/// investor rewrites whole. At 100 live receipts its value encodes to the
/// size pinned here, far under the 65,536-byte ledger entry limit.
#[test]
fn a_full_receipts_entry_stays_far_under_the_ledger_entry_limit() {
    use soroban_sdk::{
        xdr::{Limits, ScVal, WriteXdr},
        IntoVal, TryFromVal, Val,
    };
    let s = setup();
    s.allow(&s.alice, i128::MAX);
    for _ in 1..MAX_RECEIPTS {
        s.consume(&s.alice, &1_000_000_000_000);
    }
    s.e.cost_estimate().budget().reset_unlimited();
    s.consume(&s.alice, &1_000_000_000_000);
    let written = s.e.cost_estimate().resources().write_bytes;
    let entry = s.client.receipts(&s.sale, &s.alice).unwrap();
    assert_eq!(entry.items.len(), MAX_RECEIPTS);
    let value: Val = entry.into_val(&s.e);
    let size = ScVal::try_from_val(&s.e, &value)
        .unwrap()
        .to_xdr(Limits::none())
        .unwrap()
        .len();
    std::println!(
        "receipts entry at {MAX_RECEIPTS}: value {size} B; the 100th purchase writes {written} B"
    );
    assert_eq!(size, 8_060);
}

#[test]
fn receipt_numbers_start_at_zero_per_sale_and_investor() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.allow(&s.bob, 100);
    let other = s.another_sale();
    assert_eq!(s.consume(&s.alice, &1), 0);
    assert_eq!(s.consume(&s.bob, &1), 0);
    assert_eq!(other.buy(&s.id, &s.alice, &1), 0);
    at(&s.e, 2026, 10, 20);
    assert_eq!(s.consume(&s.alice, &1), 1);
    at(&s.e, 2026, 11, 2);
    assert_eq!(s.consume(&s.alice, &1), 2);
    assert_eq!(s.consume(&s.bob, &1), 1);
    // A restore never frees a number for reuse.
    s.restore(&s.alice, &[2]);
    assert_eq!(s.consume(&s.alice, &1), 3);
}

#[test]
fn a_fully_refunded_investor_keeps_the_entry_and_its_next_number() {
    let s = setup();
    s.allow(&s.alice, 100);
    let a = s.consume(&s.alice, &10);
    let b = s.consume(&s.alice, &20);
    s.restore(&s.alice, &[a, b]);
    let left = s.client.receipts(&s.sale, &s.alice).unwrap();
    assert_eq!((left.next, left.items.len()), (2, 0));
    assert_eq!(s.consume(&s.alice, &5), 2);
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
    s.restore(&s.alice, &[second]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 100);
    assert_eq!(s.client.remaining(&s.alice), 900);
    assert_eq!(
        s.receipt(&s.alice, first),
        Some(Receipt {
            amount: 100,
            period: period(2026, 10),
        })
    );
    assert_eq!(s.receipt(&s.alice, second), None);
}

#[test]
fn a_restore_reaches_only_the_named_investors_receipts() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.allow(&s.bob, 100);
    let alice = s.consume(&s.alice, &30);
    let bob = s.consume(&s.bob, &40);
    assert_eq!(alice, bob);
    s.restore(&s.bob, &[bob]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 30);
    assert_eq!(s.client.usage(&s.bob, &period(2026, 10)), 0);
    assert!(s.receipt(&s.alice, alice).is_some());
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
    s.restore(&s.alice, &[a]);
    assert!(has_usage());
    s.restore(&s.alice, &[b]);
    assert!(!has_usage());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
}

/// Stands in for a simulation: buys through the sale stub, then fails, so
/// the state rolls back while the footprint the buy recorded stays.
#[contract]
pub struct Simulate;

#[contracterror]
#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum SimError {
    RolledBack = 1,
}

#[contractimpl]
impl Simulate {
    pub fn run(
        e: &Env,
        sale: Address,
        allowlist: Address,
        investor: Address,
        amount: i128,
    ) -> Result<(), SimError> {
        SaleStubClient::new(e, &sale).buy(&allowlist, &investor, &amount);
        Err(SimError::RolledBack)
    }
}

/// Two purchases racing for the same ledger. `racer`'s purchase is
/// simulated first, then, if `other_lands_first`, `other` buys; then the
/// footprint is frozen and `racer`'s purchase runs against it. The frozen
/// footprint is everything the run touched so far, so the harness proves
/// that `racer`'s purchase needs no key its own simulation didn't record
/// beyond what `other`'s purchase recorded: with different investors,
/// none at all.
#[contract]
pub struct Race;

#[contractimpl]
impl Race {
    pub fn run(
        e: &Env,
        sale: Address,
        allowlist: Address,
        simulate: Address,
        other: Address,
        racer: Address,
        other_lands_first: bool,
    ) -> u32 {
        let stub = SaleStubClient::new(e, &sale);
        // Warm-up: the shared entries exist, as on a live offering.
        let n = stub.buy(&allowlist, &other, &1);
        stub.refund(&allowlist, &other, &soroban_sdk::vec![e, n]);
        let _ = SimulateClient::new(e, &simulate).try_run(&sale, &allowlist, &racer, &1);
        if other_lands_first {
            stub.buy(&allowlist, &other, &1);
        }
        e.host().switch_to_enforcing_storage().unwrap();
        stub.buy(&allowlist, &racer, &1)
    }
}

fn race(same_investor: bool, other_lands_first: bool) -> u32 {
    let s = setup();
    s.allow(&s.alice, 100);
    s.allow(&s.bob, 100);
    let simulate = s.e.register(Simulate, ());
    let race = s.e.register(Race, ());
    let racer = if same_investor { &s.alice } else { &s.bob };
    RaceClient::new(&s.e, &race).run(
        &s.sale,
        &s.id,
        &simulate,
        &s.alice,
        racer,
        &other_lands_first,
    )
}

/// The regression for a per-sale counter: Bob's purchase, simulated before
/// Alice's landed in the same offering, used to need a receipt key outside
/// its footprint and fail with `Error(Storage, ExceededLimit)`. Receipt
/// keys name only the purchase's own sale and investor, so it passes.
#[test]
fn purchases_by_different_investors_never_conflict() {
    assert_eq!(race(false, false), 0);
    assert_eq!(race(false, true), 0);
}

/// The same investor's two purchases use the same keys, so the footprint
/// holds either way; the second gets the next number. What breaks is the
/// write size, below.
#[test]
fn the_same_investors_second_purchase_keeps_its_footprint() {
    assert_eq!(race(true, false), 1);
    assert_eq!(race(true, true), 2);
}

/// But the shared entry grows by one receipt per purchase, which is what
/// breaks the same investor's racing purchases on the network. No
/// simulation runs here: purchases A and B by the same investor are both
/// simulated against the same state, so B's simulation equals A's own run
/// and declares A's write bytes; B then runs after A, against an entry
/// one receipt bigger. This test measures that difference between two
/// consecutive purchases. On the network it exceeds B's declared write
/// bytes unless the submitter left a margin; the backend re-simulates and
/// resubmits, as the documents say.
#[test]
fn each_purchase_by_the_same_investor_writes_one_receipt_more() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &1);
    s.e.cost_estimate().budget().reset_unlimited();
    s.consume(&s.alice, &1);
    let simulated = s.e.cost_estimate().resources().write_bytes;
    s.e.cost_estimate().budget().reset_unlimited();
    s.consume(&s.alice, &1);
    let landed = s.e.cost_estimate().resources().write_bytes;
    std::println!("same investor: {simulated} B simulated, {landed} B landed");
    // One more `{amount, period}` item under its number: 80 B.
    assert_eq!(landed - simulated, 80);
}

/// The period Alice's receipt in the stub sale was charged to.
fn charged_to(s: &super::Setup, receipt: u32) -> u32 {
    s.receipt(&s.alice, receipt).unwrap().period
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
    s.restore(&s.alice, &[september, october]);
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
    s.restore(&s.alice, &[september]);
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
    s.restore(&s.alice, &[a, b]);
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
