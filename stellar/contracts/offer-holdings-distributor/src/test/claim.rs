//! A claim pays each listed holder what its quotas earned and it has not
//! received. Anyone sends it; the money goes to the holder.
use soroban_sdk::{testutils::Address as _, Address, Vec};

use super::{err, setup, SUPPLY, USDC};

extern crate std;
use crate::{OfferHoldingsDistributorError, MAX_HOLDERS};

#[test]
fn holders_are_paid_by_what_they_hold() {
    let s = setup();
    s.give(&s.alice, 100);
    s.give(&s.bob, 300);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice, &s.bob]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), 300 * USDC);
    assert_eq!(s.client.credited(&s.alice), 100 * USDC);
    assert_eq!(s.client.credited(&s.bob), 300 * USDC);
    assert_eq!(s.pot(), 600 * USDC);
}

/// Nobody signs a claim: the sender gains nothing from it.
#[test]
fn a_claim_needs_no_signature() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.e.set_auths(&[]);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
}

#[test]
fn a_second_claim_pays_nothing() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice]);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    assert_eq!(s.client.credited(&s.alice), 100 * USDC);
    assert_eq!(s.client.claimable(&s.alice), 0);
}

#[test]
fn a_claim_after_a_new_deposit_pays_the_difference() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice]);
    s.deposit(2 * SUPPLY * USDC);
    assert_eq!(s.client.claimable(&s.alice), 200 * USDC);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 300 * USDC);
}

/// A holder who skipped a deposit loses nothing: the next claim pays every
/// deposit since its last one.
#[test]
fn a_late_claim_pays_every_deposit_at_once() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.deposit(SUPPLY * USDC);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 300 * USDC);
}

#[test]
fn a_holder_listed_twice_is_paid_once() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice, &s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
}

#[test]
fn an_account_with_no_quotas_is_passed_over() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.stranger, &s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    assert_eq!(s.client.credited(&s.stranger), 0);
}

#[test]
fn a_claim_before_any_deposit_pays_nothing() {
    let s = setup();
    s.give(&s.alice, 100);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 0);
}

/// A holder who returned quotas after claiming has received more than its
/// balance now earns. It is owed nothing; the claim does not fail.
#[test]
fn a_holder_who_gave_quotas_back_after_claiming_is_passed_over() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice]);
    s.token.transfer(&s.alice, &s.desk, &60);
    assert_eq!(s.client.claimable(&s.alice), 0);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    // Its record catches up with later deposits before it is paid again:
    // 40 quotas × 3 USDC = 120, of which 100 was paid.
    s.deposit(2 * SUPPLY * USDC);
    assert_eq!(s.client.claimable(&s.alice), 20 * USDC);
}

/// The accepted risk, pinned: the record is per account and the balance is
/// read at claim time, so quotas forced to another account after their
/// holder claimed are paid a second time, out of the other holders' money.
#[test]
fn quotas_moved_after_a_claim_are_paid_again() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice]);
    s.token
        .admin_transfer(&s.alice, &s.bob, &100, &s.token_admin);
    assert_eq!(s.client.claimable(&s.bob), 100 * USDC);
    s.claim(&[&s.bob]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), 100 * USDC);
}

/// The same risk without a forced transfer: an ordinary transfer passes
/// when either side is on the whitelist, so quotas a holder returned to an
/// operational account can be handed on to another investor.
#[test]
fn quotas_resold_through_a_whitelisted_account_are_paid_again() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice]);
    s.token.transfer(&s.alice, &s.desk, &100);
    s.token.transfer(&s.desk, &s.bob, &100);
    assert_eq!(s.client.claimable(&s.bob), 100 * USDC);
    s.claim(&[&s.bob]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), 100 * USDC);
}

/// The share follows the quotas until it is claimed: a holder who returns
/// quotas before anyone claimed for it gives up what they had earned.
#[test]
fn a_holder_who_returns_quotas_before_claiming_gives_up_their_share() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.token.transfer(&s.alice, &s.desk, &100);
    assert_eq!(s.client.claimable(&s.alice), 0);
    s.claim(&[&s.alice, &s.desk]);
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.pot(), SUPPLY * USDC);
}

#[test]
fn an_empty_list_is_refused() {
    let s = setup();
    assert_eq!(
        err(s.client.try_claim(&Vec::new(&s.e))),
        OfferHoldingsDistributorError::EmptyBatch.into()
    );
}

#[test]
fn a_list_over_the_cap_is_refused() {
    let s = setup();
    let mut holders = Vec::new(&s.e);
    for _ in 0..=MAX_HOLDERS {
        holders.push_back(Address::generate(&s.e));
    }
    assert_eq!(
        err(s.client.try_claim(&holders)),
        OfferHoldingsDistributorError::BatchTooLarge.into()
    );
}

/// The limits both networks give one transaction (read 2026-11-26 with
/// `stellar network settings`). soroban-sdk 26's test host still enforces
/// an older, smaller table, so the measuring test lifts the host's check
/// and asserts these.
const EVENT_BYTES: u32 = 16_384;
const WRITE_ENTRIES: u32 = 200;
const FOOTPRINT_ENTRIES: u32 = 400;
const DISK_READ_ENTRIES: u32 = 200;
const INSTRUCTIONS: i64 = 400_000_000;

/// A full list of classic accounts, each paid: what the platform's relay
/// sends. `-- --nocapture` prints the figures the distributor document
/// quotes.
#[test]
fn a_full_list_pays_every_holder_under_the_network_limits() {
    let s = setup();
    s.e.cost_estimate().budget().reset_unlimited();
    s.e.cost_estimate().disable_resource_limits();
    let mut holders = Vec::new(&s.e);
    for i in 0..MAX_HOLDERS {
        let holder = s.create_account_with_trustline(i);
        s.give(&holder, 10);
        holders.push_back(holder);
    }
    s.deposit(SUPPLY * USDC);
    s.client.claim(&holders);
    let used = s.e.cost_estimate().resources();
    std::println!(
        "claim x{MAX_HOLDERS}: {} B of events, {} write entries, {} read entries in memory, {} on disk, {} instructions, {} B written",
        used.contract_events_size_bytes,
        used.write_entries,
        used.memory_read_entries,
        used.disk_read_entries,
        used.instructions,
        used.write_bytes
    );
    // 50 × the asset's `transfer` (240 B) + `claimed` (140 B).
    assert_eq!(used.contract_events_size_bytes, 50 * 240 + 140);
    // One trustline and one record per holder, and the distributor's own
    // balance.
    assert_eq!(used.write_entries, 101);
    // The test asset's code is one character shorter than USDC's, so each
    // of its `transfer` events is 4 B smaller: 12,340 B with USDC.
    let with_usdc = used.contract_events_size_bytes + MAX_HOLDERS * 4;
    assert!(with_usdc <= EVENT_BYTES);
    assert!(used.write_entries <= WRITE_ENTRIES);
    assert_eq!(used.memory_read_entries + used.disk_read_entries, 205);
    assert!(used.memory_read_entries + used.disk_read_entries <= FOOTPRINT_ENTRIES);
    // One trustline per holder is read from disk; a holder whose balance
    // or record was archived adds a restore, a disk read and a write each.
    assert_eq!(used.disk_read_entries, 50);
    assert!(used.disk_read_entries <= DISK_READ_ENTRIES);
    // Counted on native code, without the cost of running the token's and
    // the distributor's WASM: a floor, not the figure a transaction pays.
    assert!(used.instructions <= INSTRUCTIONS / 4);
    for holder in holders.iter() {
        assert_eq!(s.usdc.balance(&holder), 10 * USDC);
    }
    assert_eq!(s.pot(), 500 * USDC);
}
