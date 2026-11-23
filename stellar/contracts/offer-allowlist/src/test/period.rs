//! The contract's period getter agrees with the documented formula at
//! pinned ledger times. The arithmetic itself is unit-tested inside the
//! period module.
use super::{at, at_time, err, period, setup};
use crate::OfferAllowlistError;
use soroban_sdk::testutils::Ledger as _;

#[test]
fn current_period_follows_the_ledger_time() {
    let s = setup();
    assert_eq!(s.client.current_period(), period(2026, 10));
    at(&s.e, 2027, 1, 1);
    assert_eq!(s.client.current_period(), period(2027, 1));
    at_time(&s.e, 2026, 12, 31, 23, 59, 59);
    assert_eq!(s.client.current_period(), period(2026, 12));
}

#[test]
fn a_ledger_time_whose_period_does_not_fit_fails_with_invalid_period() {
    let s = setup();
    s.e.ledger().set_timestamp(u64::MAX);
    assert_eq!(
        err(s.client.try_current_period()),
        OfferAllowlistError::InvalidPeriod.into()
    );
}
