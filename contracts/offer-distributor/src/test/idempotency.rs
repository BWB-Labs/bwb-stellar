//! A distribution id is paid at most once. Repeating a call pays nothing
//! more, a correction is a new id, and a call that failed leaves its id
//! free to be sent again.
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

use super::{err, setup, FUNDS, USDC};
use crate::{OfferDistributor, OfferDistributorClient, OfferDistributorError};

#[test]
fn the_same_distribution_sent_twice_pays_once() {
    let s = setup();
    assert!(!s.client.is_paid(&s.dist(42)));
    s.distribute(42, &[(&s.alice, 900 * USDC)]);
    assert!(s.client.is_paid(&s.dist(42)));
    assert_eq!(
        err(s.try_distribute(42, &[(&s.alice, 900 * USDC)])),
        OfferDistributorError::AlreadyPaid.into()
    );
    assert_eq!(s.usdc.balance(&s.alice), 900 * USDC);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - 900 * USDC);
}

/// The id is what is remembered, not the list: a paid id is refused
/// whatever comes with it.
#[test]
fn a_paid_id_is_refused_with_a_different_list_or_asset() {
    let s = setup();
    s.distribute(42, &[(&s.alice, 900 * USDC)]);
    assert_eq!(
        err(s.try_distribute(42, &[(&s.bob, USDC)])),
        OfferDistributorError::AlreadyPaid.into()
    );
    let other_asset = Address::generate(&s.e);
    assert_eq!(
        err(s.client.try_distribute(
            &s.dist(42),
            &other_asset,
            &s.payouts(&[(&s.bob, USDC)]),
            &s.owner
        )),
        OfferDistributorError::AlreadyPaid.into()
    );
    assert_eq!(s.usdc.balance(&s.bob), 0);
}

/// Ana was owed 1,000 and distribution 42 paid her 900. The owner pays the
/// missing 100 as distribution 43; 42 stays as it happened.
#[test]
fn a_correction_is_a_new_distribution() {
    let s = setup();
    s.distribute(42, &[(&s.alice, 900 * USDC)]);
    s.distribute(43, &[(&s.alice, 100 * USDC)]);
    assert_eq!(s.usdc.balance(&s.alice), 1_000 * USDC);
    assert!(s.client.is_paid(&s.dist(42)));
    assert!(s.client.is_paid(&s.dist(43)));
}

/// A call that fails undoes everything, the id's mark included, so the
/// same id can be sent again once the list is fixed.
#[test]
fn a_failed_distribution_leaves_its_id_unpaid() {
    let s = setup();
    s.usdc_admin.set_authorized(&s.bob, &false);
    assert!(s
        .try_distribute(42, &[(&s.alice, USDC), (&s.bob, USDC)])
        .is_err());
    assert!(!s.client.is_paid(&s.dist(42)));
    assert_eq!(s.usdc.balance(&s.alice), 0);
    s.distribute(42, &[(&s.alice, USDC)]);
    assert!(s.client.is_paid(&s.dist(42)));
    assert_eq!(s.usdc.balance(&s.alice), USDC);
}

/// A refused call marks nothing either.
#[test]
fn a_refused_distribution_leaves_its_id_unpaid() {
    let s = setup();
    assert_eq!(
        err(s.try_distribute(42, &[(&s.alice, 0)])),
        OfferDistributorError::NonPositiveAmount.into()
    );
    assert!(!s.client.is_paid(&s.dist(42)));
    s.distribute(42, &[(&s.alice, USDC)]);
}

/// Ids belong to one distributor: another offering's distributor may use
/// the same id.
#[test]
fn ids_are_separate_per_distributor() {
    let s = setup();
    let other = s.e.register(OfferDistributor, (s.client.powers(),));
    let other = OfferDistributorClient::new(&s.e, &other);
    s.distribute(42, &[(&s.alice, USDC)]);
    assert!(!other.is_paid(&s.dist(42)));
    other.distribute(
        &s.dist(42),
        &s.usdc.address,
        &s.payouts(&[(&s.alice, USDC)]),
        &s.owner,
    );
    assert_eq!(s.usdc.balance(&s.alice), 2 * USDC);
}
