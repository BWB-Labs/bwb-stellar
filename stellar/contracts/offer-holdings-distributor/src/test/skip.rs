//! A holder that cannot be paid is passed over with `claim_skipped`: its
//! share stays in the distributor, its record does not change, and the
//! rest of the list is paid.
use soroban_sdk::Event as _;

use super::{emitted, emitters, setup, SUPPLY, USDC};
use crate::events::{ClaimSkipped, Claimed};

#[test]
fn a_whitelisted_account_cannot_claim() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    assert_eq!(s.client.claimable(&s.desk), 0);
    s.claim(&[&s.desk, &s.alice]);
    let skipped = ClaimSkipped {
        holder: s.desk.clone(),
    };
    let claimed = Claimed {
        count: 1,
        total: 100 * USDC,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [skipped.to_xdr(&s.e, &s.id), claimed.to_xdr(&s.e, &s.id)]
    );
    assert_eq!(s.usdc.balance(&s.desk), 0);
    assert_eq!(s.client.credited(&s.desk), 0);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
}

/// A deposit made while the desk still holds an investor's quotas is not
/// lost: the share waits in the distributor and the investor claims it
/// after receiving the quotas.
#[test]
fn a_deposit_before_the_quotas_are_handed_out_waits_for_the_holder() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.desk]);
    assert_eq!(s.pot(), SUPPLY * USDC);
    s.give(&s.alice, 100);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
}

/// An account taken off the whitelist claims like any holder.
#[test]
fn an_account_taken_off_the_whitelist_can_claim() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    s.token
        .set_transfer_whitelist(&s.desk, &false, &s.token_admin);
    s.claim(&[&s.desk]);
    assert_eq!(s.usdc.balance(&s.desk), SUPPLY * USDC);
}

#[test]
fn a_holder_without_a_trustline_is_skipped_and_paid_once_it_has_one() {
    let s = setup();
    let carol = s.create_account(1);
    s.give(&carol, 200);
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&carol, &s.alice]);
    let skipped = ClaimSkipped {
        holder: carol.clone(),
    };
    assert_eq!(emitted(&s.e, &s.id)[0], skipped.to_xdr(&s.e, &s.id));
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    assert_eq!(s.client.credited(&carol), 0);
    assert_eq!(s.client.claimable(&carol), 200 * USDC);
    assert_eq!(s.pot(), 900 * USDC);

    s.add_trustline(1);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&carol]);
    assert_eq!(s.usdc.balance(&carol), 400 * USDC);
}

#[test]
fn a_holder_whose_balance_is_frozen_is_skipped() {
    let s = setup();
    let carol = s.create_account_with_trustline(1);
    s.give(&carol, 200);
    s.deposit(SUPPLY * USDC);
    s.usdc_admin.set_authorized(&carol, &false);
    s.claim(&[&carol]);
    let skipped = ClaimSkipped {
        holder: carol.clone(),
    };
    let claimed = Claimed { count: 0, total: 0 };
    assert_eq!(
        emitted(&s.e, &s.id),
        [skipped.to_xdr(&s.e, &s.id), claimed.to_xdr(&s.e, &s.id)]
    );
    // The refused transfer left no event of the asset's.
    assert_eq!(emitters(&s.e), [s.id.clone(), s.id.clone()]);
    assert_eq!(s.client.credited(&carol), 0);
    assert_eq!(s.pot(), SUPPLY * USDC);
    s.usdc_admin.set_authorized(&carol, &true);
    s.claim(&[&carol]);
    assert_eq!(s.usdc.balance(&carol), 200 * USDC);
}

/// When the owner has taken the money out, a claim finds the distributor
/// short. The holder is skipped, not charged: once the money is back the
/// same share is paid.
#[test]
fn a_holder_the_distributor_cannot_afford_is_skipped() {
    let s = setup();
    s.give(&s.alice, 100);
    s.give(&s.bob, 100);
    s.deposit(SUPPLY * USDC);
    s.client.withdraw(&s.usdc.address, &(850 * USDC), &s.owner);
    s.claim(&[&s.alice, &s.bob]);
    let skipped = ClaimSkipped {
        holder: s.bob.clone(),
    };
    let claimed = Claimed {
        count: 1,
        total: 100 * USDC,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [skipped.to_xdr(&s.e, &s.id), claimed.to_xdr(&s.e, &s.id)]
    );
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), 0);
    assert_eq!(s.client.credited(&s.bob), 0);
    s.usdc.transfer(&s.owner, &s.id, &(50 * USDC));
    s.claim(&[&s.bob]);
    assert_eq!(s.usdc.balance(&s.bob), 100 * USDC);
    assert_eq!(s.pot(), 0);
}

/// A share that does not fit an `i128` cannot be worked out. It takes a
/// counter no real deposit reaches, so the test writes one. The holder is
/// skipped like any other the claim cannot pay, and the read says why.
#[test]
fn a_holder_whose_share_overflows_is_skipped() {
    use crate::{storage, OfferHoldingsDistributorError};
    let s = setup();
    s.give(&s.alice, 2);
    s.e.as_contract(&s.id, || storage::set_per_quota(&s.e, i128::MAX));
    s.claim(&[&s.alice]);
    let skipped = ClaimSkipped {
        holder: s.alice.clone(),
    };
    let claimed = Claimed { count: 0, total: 0 };
    assert_eq!(
        emitted(&s.e, &s.id),
        [skipped.to_xdr(&s.e, &s.id), claimed.to_xdr(&s.e, &s.id)]
    );
    assert_eq!(s.client.credited(&s.alice), 0);
    assert_eq!(
        super::err(s.client.try_claimable(&s.alice)),
        OfferHoldingsDistributorError::AmountOverflow.into()
    );
}
