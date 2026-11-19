//! One negative test per privileged entrypoint: a signed but wrong caller
//! fails with the power's own code; an unsigned call fails at
//! authorization. The reads run with no auths at all. Positive cases use
//! exact auths, proving which account must sign and what the signature
//! covers.
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    Address, BytesN, IntoVal,
};

use super::{err, setup, trapped, GreedyToken, Setup, FUNDS, USDC};
use crate::OfferDistributorError;

extern crate std;

/// A funded account with no power cannot distribute, even its own money:
/// every distribution of this contract is the owner's.
#[test]
fn only_the_owner_distributes() {
    let s = setup();
    let legs = s.payouts(&[(&s.alice, USDC)]);
    for caller in [&s.pauser, &s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            err(s
                .client
                .try_distribute(&s.dist(1), &s.usdc.address, &legs, caller)),
            OfferDistributorError::NotOwner.into()
        );
    }
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.usdc.balance(&s.stranger), FUNDS);
    assert!(!s.client.is_paid(&s.dist(1)));
}

#[test]
fn only_owner_or_pauser_pauses_and_only_the_owner_unpauses() {
    let s = setup();
    for caller in [&s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_pause(caller)),
            OfferDistributorError::NotPauser.into()
        );
    }
    s.client.pause(&s.pauser);
    for caller in [&s.pauser, &s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_unpause(caller)),
            OfferDistributorError::NotOwner.into()
        );
    }
    s.client.unpause(&s.owner);
    s.client.pause(&s.owner);
    assert!(s.client.paused());
}

#[test]
fn only_the_upgrader_upgrades() {
    let s = setup();
    let hash = BytesN::from_array(&s.e, &[0; 32]);
    for caller in [&s.owner, &s.pauser, &s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_upgrade(&hash, caller)),
            OfferDistributorError::NotUpgrader.into()
        );
    }
}

/// A missing signature fails in the host's authorization check. A direct
/// call surfaces it as `Error(Auth, InvalidAction)`; a `try_` call rewraps
/// non-contract errors, so these tests use the direct call.
macro_rules! needs_signature {
    ($name:ident, |$s:ident| $call:expr) => {
        needs_signature!($name, |$s| {}; $call);
    };
    ($name:ident, |$s:ident| $prep:block; $call:expr) => {
        #[test]
        #[should_panic(expected = "Error(Auth, InvalidAction)")]
        fn $name() {
            let $s = setup();
            $prep
            $s.e.set_auths(&[]);
            $call;
        }
    };
}

needs_signature!(distribute_needs_the_owners_signature, |s| s
    .distribute(1, &[(&s.alice, USDC)]));
needs_signature!(pause_needs_a_signature, |s| s.client.pause(&s.pauser));
needs_signature!(unpause_needs_the_owners_signature, |s| { s.client.pause(&s.pauser); }; s
    .client
    .unpause(&s.owner));
needs_signature!(upgrade_needs_the_upgraders_signature, |s| s
    .client
    .upgrade(&BytesN::from_array(&s.e, &[0; 32]), &s.upgrader));

#[test]
fn the_reads_need_no_signature() {
    let s = setup();
    s.e.set_auths(&[]);
    assert!(!s.client.is_paid(&s.dist(1)));
    assert_eq!(s.client.powers().owner, s.owner);
    assert!(!s.client.paused());
    assert_eq!(s.client.schema_version(), 1);
}

/// Mocks exactly one authorization: the owner signing distribution `n` of
/// `signed` in `asset`, with one `transfer` on that asset per entry of
/// `transfers` and nothing else under it.
fn owner_signs(
    s: &Setup,
    n: u8,
    asset: &Address,
    signed: &[(&Address, i128)],
    transfers: &[(&Address, i128)],
) {
    let args = (s.dist(n), asset, s.payouts(signed), &s.owner).into_val(&s.e);
    let sub_invokes: std::vec::Vec<MockAuthInvoke> = transfers
        .iter()
        .map(|(to, amount)| MockAuthInvoke {
            contract: asset,
            fn_name: "transfer",
            args: (&s.owner, *to, *amount).into_val(&s.e),
            sub_invokes: &[],
        })
        .collect();
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "distribute",
            args,
            sub_invokes: &sub_invokes,
        },
    }]);
}

/// The owner signs one authorization for `distribute`, which covers each
/// transfer the asset makes in its name: the exact recipients and amounts.
#[test]
fn the_owners_signature_alone_distributes() {
    let s = setup();
    let legs = [(&s.alice, 3 * USDC), (&s.bob, 2 * USDC)];
    owner_signs(&s, 1, &s.usdc.address, &legs, &legs);
    s.distribute(1, &legs);
    assert_eq!(s.usdc.balance(&s.alice), 3 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), 2 * USDC);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - 5 * USDC);
}

/// A signature for one list pays that list and no other: the same id sent
/// with a different recipient is not covered. It fails at the call's own
/// authorization check, before the contract runs.
#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn a_signature_for_one_list_does_not_pay_another() {
    let s = setup();
    let signed = [(&s.alice, USDC)];
    owner_signs(&s, 1, &s.usdc.address, &signed, &signed);
    s.distribute(1, &[(&s.bob, USDC)]);
}

/// A signature for `distribute` alone, without the transfers under it,
/// moves nothing: the asset asks for the owner's authorization per payout
/// and refuses, which fails the whole call.
#[test]
fn a_signature_without_the_transfers_moves_nothing() {
    let s = setup();
    let legs = [(&s.alice, USDC)];
    owner_signs(&s, 1, &s.usdc.address, &legs, &[]);
    assert_eq!(err(s.try_distribute(1, &legs)), trapped());
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert!(!s.client.is_paid(&s.dist(1)));
}

/// A signature that covers one payout of two pays neither.
#[test]
fn a_signature_missing_one_transfer_pays_nobody() {
    let s = setup();
    let legs = [(&s.alice, USDC), (&s.bob, USDC)];
    owner_signs(&s, 1, &s.usdc.address, &legs, &legs[..1]);
    assert_eq!(err(s.try_distribute(1, &legs)), trapped());
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

/// An asset that tries to move the owner's real USDC to a thief gets
/// nothing when the owner signed only what a distribution is: one
/// `transfer` on the named asset per payout. This is why the signing side
/// must build or check that tree itself and never sign whatever a
/// simulation recorded: under a recorded tree the theft below would pass.
#[test]
fn an_asset_that_asks_for_more_than_was_signed_gets_nothing() {
    let s = setup();
    let thief = Address::generate(&s.e);
    let greedy =
        s.e.register(GreedyToken, (s.usdc.address.clone(), thief.clone()));
    let legs = [(&s.alice, USDC)];
    owner_signs(&s, 1, &greedy, &legs, &legs);
    assert_eq!(
        err(s
            .client
            .try_distribute(&s.dist(1), &greedy, &s.payouts(&legs), &s.owner)),
        trapped()
    );
    assert_eq!(s.usdc.balance(&thief), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
    assert!(!s.client.is_paid(&s.dist(1)));

    // The same call with every authorization granted, as a recording
    // simulation would grant it, pays the thief.
    s.e.mock_all_auths();
    s.client
        .distribute(&s.dist(1), &greedy, &s.payouts(&legs), &s.owner);
    assert_eq!(s.usdc.balance(&thief), USDC);
}
