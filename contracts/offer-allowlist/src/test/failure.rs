//! Every documented rejection is reachable through the client and carries
//! its code: each 61xx code at least once, plus OpenZeppelin's 2007.
extern crate std;
use soroban_sdk::{vec, Symbol};

use super::{code, entry, err, period, setup, ROLE_NOT_HELD};

use crate::storage::DataKey;
use crate::OfferAllowlistError;

#[test]
fn consume_for_a_disabled_investor_fails_with_investor_not_enabled() {
    let s = setup();
    s.client
        .set_allocations(&vec![&s.e, entry(&s.alice, 100, false)], &s.hot_key);
    let r = s.try_consume(&s.alice, &1);
    assert_eq!(err(r), OfferAllowlistError::InvestorNotEnabled.into());
}

#[test]
fn consume_for_an_unknown_investor_fails_with_investor_not_enabled() {
    let s = setup();
    let r = s.try_consume(&s.alice, &1);
    assert_eq!(err(r), OfferAllowlistError::InvestorNotEnabled.into());
    let view = s.client.allocation(&s.alice);
    assert!(!view.enabled);
    assert_eq!(view.max, 0);
    assert_eq!(view.consumed, 0);
}

#[test]
fn consume_beyond_the_room_fails_with_allocation_exceeded() {
    let s = setup();
    s.allow(&s.alice, 10);
    let r = s.try_consume(&s.alice, &11);
    assert_eq!(err(r), OfferAllowlistError::AllocationExceeded.into());
}

#[test]
fn a_negative_cap_fails_with_invalid_amount() {
    let s = setup();
    let r = s
        .client
        .try_set_allocations(&vec![&s.e, entry(&s.alice, -1, true)], &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
}

#[test]
fn a_non_positive_consume_fails_with_invalid_amount() {
    let s = setup();
    s.allow(&s.alice, 100);
    for amount in [0_i128, -1] {
        let r = s.try_consume(&s.alice, &amount);
        assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
    }
}

#[test]
fn an_empty_restore_list_fails_with_invalid_amount() {
    let s = setup();
    let r = s.try_restore(&[]);
    assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
}

#[test]
fn an_unknown_receipt_fails_with_receipt_not_found() {
    let s = setup();
    let r = s.try_restore(&[0]);
    assert_eq!(err(r), OfferAllowlistError::ReceiptNotFound.into());
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &10);
    let r = s.try_restore(&[1]);
    assert_eq!(err(r), OfferAllowlistError::ReceiptNotFound.into());
}

#[test]
fn a_double_restore_fails_with_receipt_not_found() {
    let s = setup();
    s.allow(&s.alice, 100);
    let first = s.consume(&s.alice, &10);
    // A later purchase in the same month leaves usage that the old check
    // would have let the second restore eat into.
    s.consume(&s.alice, &50);
    s.restore(&[first]);
    let r = s.try_restore(&[first]);
    assert_eq!(err(r), OfferAllowlistError::ReceiptNotFound.into());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 50);
}

#[test]
fn another_sales_receipt_fails_with_receipt_not_found() {
    let s = setup();
    s.allow(&s.alice, 100);
    let other = s.another_sale();
    // Both sales hold a receipt 0; each names only its own.
    let theirs = other.buy(&s.id, &s.alice, &30);
    let ours = s.consume(&s.alice, &20);
    assert_eq!((theirs, ours), (0, 0));
    s.restore(&[ours]);
    let r = s.try_restore(&[theirs]);
    assert_eq!(err(r), OfferAllowlistError::ReceiptNotFound.into());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 30);
    assert!(s.client.receipt(&other.address, &theirs).is_some());
}

#[test]
fn a_number_repeated_in_one_list_fails_and_changes_nothing() {
    let s = setup();
    s.allow(&s.alice, 100);
    let a = s.consume(&s.alice, &10);
    let b = s.consume(&s.alice, &20);
    let r = s.try_restore(&[a, b, a]);
    assert_eq!(err(r), OfferAllowlistError::ReceiptNotFound.into());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 30);
    assert!(s.client.receipt(&s.sale, &a).is_some());
    assert!(s.client.receipt(&s.sale, &b).is_some());
}

/// The audit's finding A, kept as a regression: sale A refunding twice
/// while sale B holds usage in the same month. With month totals the second
/// refund passed, B's honest refund then failed, and the investor could
/// buy past the cap. With receipts A's second refund fails and B is
/// untouched.
#[test]
fn one_sale_cannot_over_restore_into_another_sales_usage() {
    let s = setup();
    s.allow(&s.alice, 1_000);
    let b = s.another_sale();
    let from_a = s.consume(&s.alice, &600);
    let from_b = b.buy(&s.id, &s.alice, &400);
    assert_eq!(s.client.remaining(&s.alice), 0);

    s.restore(&[from_a]);
    let r = s.try_restore(&[from_a]);
    assert_eq!(err(r), OfferAllowlistError::ReceiptNotFound.into());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 400);

    b.refund(&s.id, &vec![&s.e, from_b]);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
    assert_eq!(s.client.remaining(&s.alice), 1_000);
}

/// Usage is the sum of its live receipts, so a receipt is always covered.
/// Should that ever break (a bad upgrade, a hand-edited entry), restore
/// fails with 6102 instead of writing a negative usage.
#[test]
fn a_receipt_its_usage_no_longer_covers_fails_with_invalid_amount() {
    let s = setup();
    s.allow(&s.alice, 100);
    let n = s.consume(&s.alice, &10);
    s.e.as_contract(&s.id, || {
        crate::storage::set_usage(&s.e, &s.alice, period(2026, 10), 9)
    });
    let r = s.try_restore(&[n]);
    assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
    assert!(s.client.receipt(&s.sale, &n).is_some());
}

/// A sale's receipt counter is a `u32`. Past the last number, `consume`
/// fails with 6102 rather than wrapping onto receipt 0.
#[test]
fn a_sale_out_of_receipt_numbers_fails_with_invalid_amount() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.e.as_contract(&s.id, || {
        s.e.storage()
            .persistent()
            .set(&DataKey::NextReceipt(s.sale.clone()), &u32::MAX)
    });
    let r = s.try_consume(&s.alice, &1);
    assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 0);
}

#[test]
fn a_list_over_its_limit_fails_with_batch_too_large() {
    let s = setup();
    let mut entries = soroban_sdk::Vec::new(&s.e);
    for _ in 0..=crate::MAX_BATCH {
        entries.push_back(entry(&s.alice, 1, true));
    }
    let r = s.client.try_set_allocations(&entries, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
    let receipts: std::vec::Vec<u32> = (0..=crate::MAX_RESTORE).collect();
    let r = s.try_restore(&receipts);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
}

#[test]
fn consume_at_a_ledger_time_whose_period_does_not_fit_fails_with_invalid_period() {
    use soroban_sdk::testutils::Ledger as _;
    let s = setup();
    s.allow(&s.alice, 100);
    s.e.ledger().set_timestamp(u64::MAX);
    let r = s.try_consume(&s.alice, &1);
    assert_eq!(err(r), OfferAllowlistError::InvalidPeriod.into());
}

#[test]
fn renouncing_the_admin_fails_with_renounce_blocked() {
    let s = setup();
    assert_eq!(
        err(s.client.try_renounce_admin()),
        OfferAllowlistError::RenounceBlocked.into()
    );
}

#[test]
fn revoking_a_role_not_held_fails_with_role_not_held() {
    let s = setup();
    let r = s
        .client
        .try_revoke_role(&s.alice, &super::allocator(&s.e), &s.admin);
    assert_eq!(err(r), code(ROLE_NOT_HELD));
    let r = s
        .client
        .try_revoke_role(&s.alice, &Symbol::new(&s.e, "auditor"), &s.admin);
    assert_eq!(err(r), code(ROLE_NOT_HELD));
}

#[test]
fn a_failed_restore_reverts_every_receipt_in_the_call() {
    let s = setup();
    s.allow(&s.alice, 100);
    super::at(&s.e, 2026, 9, 1);
    let september = s.consume(&s.alice, &50);
    super::at(&s.e, 2026, 10, 1);
    let october = s.consume(&s.alice, &50);
    let r = s.try_restore(&[september, october, october + 1]);
    assert_eq!(err(r), OfferAllowlistError::ReceiptNotFound.into());
    assert_eq!(s.client.usage(&s.alice, &period(2026, 9)), 50);
    assert_eq!(s.client.usage(&s.alice, &period(2026, 10)), 50);
    assert!(s.client.receipt(&s.sale, &september).is_some());
    assert!(s.client.receipt(&s.sale, &october).is_some());
}
