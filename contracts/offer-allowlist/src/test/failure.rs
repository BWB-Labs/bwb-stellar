//! Every documented rejection is reachable through the client and carries
//! its code: each 61xx code at least once, plus OpenZeppelin's 2007.
use soroban_sdk::{vec, Symbol};

use super::{code, entry, err, give_back, period, setup, ROLE_NOT_HELD};
use crate::OfferAllowlistError;

#[test]
fn consume_for_a_disabled_investor_fails_with_investor_not_enabled() {
    let s = setup();
    s.client
        .set_allocations(&vec![&s.e, entry(&s.alice, 100, false)], &s.hot_key);
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::InvestorNotEnabled.into());
}

#[test]
fn consume_for_an_unknown_investor_fails_with_investor_not_enabled() {
    let s = setup();
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
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
    let r = s.client.try_consume(&s.alice, &11, &s.hot_key);
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
        let r = s.client.try_consume(&s.alice, &amount, &s.hot_key);
        assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
    }
}

#[test]
fn a_non_positive_restore_amount_fails_with_invalid_amount() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &10, &s.hot_key);
    for amount in [0_i128, -1] {
        let r = s.client.try_restore(
            &s.alice,
            &vec![&s.e, give_back(period(2026, 10), amount)],
            &s.hot_key,
        );
        assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
    }
}

#[test]
fn an_empty_restore_list_fails_with_invalid_amount() {
    let s = setup();
    let r = s.client.try_restore(&s.alice, &vec![&s.e], &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::InvalidAmount.into());
}

#[test]
fn a_restore_for_an_investor_with_no_allocation_fails_with_restore_exceeds_usage() {
    let s = setup();
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(period(2026, 10), 1)],
        &s.hot_key,
    );
    assert_eq!(err(r), OfferAllowlistError::RestoreExceedsUsage.into());
}

#[test]
fn a_restore_of_a_month_with_no_usage_fails_with_restore_exceeds_usage() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &10, &s.hot_key);
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(period(2026, 9), 1)],
        &s.hot_key,
    );
    assert_eq!(err(r), OfferAllowlistError::RestoreExceedsUsage.into());
}

#[test]
fn a_double_restore_fails_with_restore_exceeds_usage() {
    let s = setup();
    s.allow(&s.alice, 100);
    let p = s.client.consume(&s.alice, &10, &s.hot_key);
    s.client
        .restore(&s.alice, &vec![&s.e, give_back(p, 10)], &s.hot_key);
    let r = s
        .client
        .try_restore(&s.alice, &vec![&s.e, give_back(p, 10)], &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::RestoreExceedsUsage.into());
}

#[test]
fn a_restore_for_a_future_period_fails_with_invalid_period() {
    let s = setup();
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(period(2026, 11), 1)],
        &s.hot_key,
    );
    assert_eq!(err(r), OfferAllowlistError::InvalidPeriod.into());
}

#[test]
fn a_batch_over_thirty_fails_with_batch_too_large() {
    let s = setup();
    let mut entries = soroban_sdk::Vec::new(&s.e);
    for _ in 0..=crate::MAX_BATCH {
        entries.push_back(entry(&s.alice, 1, true));
    }
    let r = s.client.try_set_allocations(&entries, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
    let mut refunds = soroban_sdk::Vec::new(&s.e);
    for _ in 0..=crate::MAX_BATCH {
        refunds.push_back(give_back(period(2026, 10), 1));
    }
    let r = s.client.try_restore(&s.alice, &refunds, &s.hot_key);
    assert_eq!(err(r), OfferAllowlistError::BatchTooLarge.into());
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
        .try_revoke_role(&s.alice, &super::controller(&s.e), &s.admin);
    assert_eq!(err(r), code(ROLE_NOT_HELD));
    let r = s
        .client
        .try_revoke_role(&s.alice, &Symbol::new(&s.e, "auditor"), &s.admin);
    assert_eq!(err(r), code(ROLE_NOT_HELD));
}

#[test]
fn a_failed_restore_reverts_every_period_in_the_call() {
    let s = setup();
    s.allow(&s.alice, 100);
    super::at(&s.e, 2026, 9, 1);
    let september = s.client.consume(&s.alice, &50, &s.hot_key);
    super::at(&s.e, 2026, 10, 1);
    let october = s.client.consume(&s.alice, &50, &s.hot_key);
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(september, 50), give_back(october, 51)],
        &s.hot_key,
    );
    assert_eq!(err(r), OfferAllowlistError::RestoreExceedsUsage.into());
    assert_eq!(s.client.usage(&s.alice, &september), 50);
    assert_eq!(s.client.usage(&s.alice, &october), 50);
}
