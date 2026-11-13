//! The edges of every limit: the size of a list, the amounts, the
//! recipients, and the event budget the batch size rests on.
extern crate std;
use soroban_sdk::Address;

use super::{code, err, setup, Setup, FUNDS, USDC};
use crate::{OfferDistributorError, MAX_PAYOUTS};

/// The per-transaction limits, the same on mainnet and testnet (read live
/// with `stellar network settings` on 2026-11-13). The test host enforces
/// older, stricter figures, so the measuring test lifts the host's check
/// and asserts the network's limits itself.
const WRITE_ENTRIES: u32 = 200;
const FOOTPRINT_ENTRIES: u32 = 400;
const EVENT_BYTES: u32 = 16_384;
const INSTRUCTIONS: i64 = 400_000_000;
const WRITE_BYTES: u32 = 132_096;
const DISK_READ_BYTES: u32 = 200_000;

/// The asset contract's `BalanceError`.
const ASSET_BALANCE: u32 = 10;

fn network_limits(s: &Setup) {
    s.e.cost_estimate().budget().reset_unlimited();
    s.e.cost_estimate().disable_resource_limits();
}

/// `n` classic accounts with a USDC trustline: real recipients.
fn accounts(s: &Setup, n: u32) -> std::vec::Vec<Address> {
    (0..n)
        .map(|i| s.create_account_with_trustline(1_000 + i))
        .collect()
}

fn legs(to: &[Address], amount: i128) -> std::vec::Vec<(&Address, i128)> {
    to.iter().map(|a| (a, amount)).collect()
}

#[test]
fn a_distribution_takes_thirty_payouts_and_not_thirty_one() {
    let s = setup();
    assert_eq!(MAX_PAYOUTS, 30);
    network_limits(&s);
    let to = accounts(&s, 31);
    assert_eq!(
        err(s.try_distribute(1, &legs(&to, USDC))),
        OfferDistributorError::BatchTooLarge.into()
    );
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
    s.distribute(1, &legs(&to[..30], USDC));
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - 30 * USDC);
    assert_eq!(s.usdc.balance(&to[29]), USDC);
    assert_eq!(s.usdc.balance(&to[30]), 0);
}

#[test]
fn an_empty_distribution_is_refused() {
    let s = setup();
    assert_eq!(
        err(s.try_distribute(1, &[])),
        OfferDistributorError::EmptyBatch.into()
    );
    assert!(!s.client.is_paid(&s.dist(1)));
}

#[test]
fn every_amount_must_be_positive() {
    let s = setup();
    for amount in [0, -1, -USDC, i128::MIN] {
        assert_eq!(
            err(s.try_distribute(1, &[(&s.alice, USDC), (&s.bob, amount)])),
            OfferDistributorError::NonPositiveAmount.into()
        );
    }
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

#[test]
fn the_smallest_amount_is_one_stroop() {
    let s = setup();
    s.distribute(1, &[(&s.alice, 1)]);
    assert_eq!(s.usdc.balance(&s.alice), 1);
}

/// The distributor never holds money, so paying it would strand the
/// amount there.
#[test]
fn the_distributor_itself_cannot_be_a_recipient() {
    let s = setup();
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, USDC), (&s.id, USDC)])),
        OfferDistributorError::InvalidRecipient.into()
    );
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert_eq!(s.usdc.balance(&s.alice), 0);
}

/// An amount paid to the asset's own contract would sit there for good,
/// which no distribution means.
#[test]
fn the_asset_contract_cannot_be_a_recipient() {
    let s = setup();
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, USDC), (&s.usdc.address, USDC)])),
        OfferDistributorError::InvalidRecipient.into()
    );
    assert_eq!(s.usdc.balance(&s.usdc.address), 0);
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert!(!s.client.is_paid(&s.dist(1)));
}

#[test]
fn a_recipient_listed_twice_is_paid_twice() {
    let s = setup();
    s.distribute(1, &[(&s.alice, USDC), (&s.bob, USDC), (&s.alice, 2 * USDC)]);
    assert_eq!(s.usdc.balance(&s.alice), 3 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), USDC);
}

#[test]
fn amounts_that_overflow_together_are_refused_before_any_transfer() {
    let s = setup();
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, i128::MAX), (&s.bob, 1)])),
        OfferDistributorError::AmountOverflow.into()
    );
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

/// The largest amounts the type holds pass the distributor's own checks
/// and stop at the asset, which has no such balance to move.
#[test]
fn the_largest_amounts_reach_the_asset_and_fail_there() {
    let s = setup();
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, i128::MAX)])),
        code(ASSET_BALANCE)
    );
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, i128::MAX - 1), (&s.bob, 1)])),
        code(ASSET_BALANCE)
    );
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
    assert!(!s.client.is_paid(&s.dist(1)));
}

/// Listing the owner pays the owner from its own account: allowed, and the
/// balance does not move.
#[test]
fn the_owner_may_list_itself() {
    let s = setup();
    s.distribute(1, &[(&s.owner, 5 * USDC), (&s.alice, USDC)]);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - USDC);
    assert_eq!(s.usdc.balance(&s.alice), USDC);
}

/// The contract reads nothing into an id: 32 zero bytes are an id like any
/// other, paid once.
#[test]
fn an_id_of_all_zeros_is_an_id_like_any_other() {
    let s = setup();
    s.distribute(0, &[(&s.alice, USDC)]);
    assert!(s.client.is_paid(&s.dist(0)));
    assert_eq!(
        err(s.try_distribute(0, &[(&s.alice, USDC)])),
        OfferDistributorError::AlreadyPaid.into()
    );
}

/// The contract keeps nothing: after a distribution its own balance is
/// zero and the owner's fell by exactly the total.
#[test]
fn the_distributor_holds_no_balance() {
    let s = setup();
    s.distribute(1, &[(&s.alice, 7 * USDC), (&s.bob, 5 * USDC)]);
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - 12 * USDC);
}

/// A full distribution to classic accounts, the largest case: each payout
/// emits the asset's `transfer` and this contract's `distributed`, and the
/// call ends with `distribution_completed`. `-- --nocapture` prints the
/// figures the distributor document quotes.
#[test]
fn a_full_distribution_stays_under_the_network_limits() {
    let s = setup();
    network_limits(&s);
    let to = accounts(&s, MAX_PAYOUTS);
    s.distribute(1, &legs(&to, 1_000 * USDC));
    let used = s.e.cost_estimate().resources();
    std::println!(
        "distribute x{MAX_PAYOUTS}: {} B of events, {} write entries, {} read entries in memory, {} on disk, {} instructions, {} B written, {} B read from disk",
        used.contract_events_size_bytes,
        used.write_entries,
        used.memory_read_entries,
        used.disk_read_entries,
        used.instructions,
        used.write_bytes,
        used.disk_read_bytes
    );
    // 30 × (`transfer` 240 B + `distributed` 260 B) + the summary's 252 B.
    assert_eq!(used.contract_events_size_bytes, 30 * (240 + 260) + 252);
    // The test asset's code is one character shorter than USDC's, which
    // pads each of its `transfer` events by 4 B more: 15,372 B with USDC.
    let with_usdc = used.contract_events_size_bytes + MAX_PAYOUTS * 4;
    assert!(with_usdc <= EVENT_BYTES);
    // One trustline per recipient, the owner's, the paid id and the
    // instance, whose lifetime the call may extend.
    assert_eq!(used.write_entries, 33);
    assert!(used.write_entries <= WRITE_ENTRIES);
    assert!(used.memory_read_entries + used.disk_read_entries <= FOOTPRINT_ENTRIES);
    assert!(used.instructions <= INSTRUCTIONS / 10);
    assert!(used.write_bytes <= WRITE_BYTES / 10);
    assert!(used.disk_read_bytes <= DISK_READ_BYTES / 10);
}
