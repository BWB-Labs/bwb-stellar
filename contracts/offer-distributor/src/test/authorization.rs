//! One negative test per privileged entrypoint: a signed but wrong caller
//! fails with the power's own code; an unsigned call fails at
//! authorization. The reads run with no auths at all. Positive cases use
//! exact auths, proving which account must sign and what the signature
//! covers.
use soroban_sdk::{
    testutils::{MockAuth, MockAuthInvoke},
    BytesN, IntoVal,
};

use super::{err, setup, FUNDS, USDC};
use crate::OfferDistributorError;

/// A funded account with no power cannot distribute, even its own money:
/// every distribution of this contract is the owner's.
#[test]
fn only_the_owner_distributes() {
    let s = setup();
    let legs = s.payouts(&[(&s.alice, USDC)]);
    for caller in [&s.platform, &s.stranger, &s.alice] {
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
    for caller in [&s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_pause(caller)),
            OfferDistributorError::NotPauser.into()
        );
    }
    s.client.pause(&s.platform);
    for caller in [&s.platform, &s.stranger, &s.alice] {
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
    for caller in [&s.owner, &s.stranger, &s.alice] {
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
needs_signature!(pause_needs_a_signature, |s| s.client.pause(&s.platform));
needs_signature!(unpause_needs_the_owners_signature, |s| { s.client.pause(&s.platform); }; s
    .client
    .unpause(&s.owner));
needs_signature!(upgrade_needs_the_upgraders_signature, |s| s
    .client
    .upgrade(&BytesN::from_array(&s.e, &[0; 32]), &s.platform));

#[test]
fn the_reads_need_no_signature() {
    let s = setup();
    s.e.set_auths(&[]);
    assert!(!s.client.is_paid(&s.dist(1)));
    assert_eq!(s.client.powers().owner, s.owner);
    assert!(!s.client.paused());
    assert_eq!(s.client.schema_version(), 1);
}

/// The owner signs one authorization for `distribute`, which covers each
/// transfer the asset makes in its name: the exact recipients and amounts.
#[test]
fn the_owners_signature_alone_distributes() {
    let s = setup();
    let legs = s.payouts(&[(&s.alice, 3 * USDC), (&s.bob, 2 * USDC)]);
    let args = (s.dist(1), &s.usdc.address, legs.clone(), &s.owner).into_val(&s.e);
    let to_alice = (&s.owner, &s.alice, 3 * USDC).into_val(&s.e);
    let to_bob = (&s.owner, &s.bob, 2 * USDC).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "distribute",
            args,
            sub_invokes: &[
                MockAuthInvoke {
                    contract: &s.usdc.address,
                    fn_name: "transfer",
                    args: to_alice,
                    sub_invokes: &[],
                },
                MockAuthInvoke {
                    contract: &s.usdc.address,
                    fn_name: "transfer",
                    args: to_bob,
                    sub_invokes: &[],
                },
            ],
        },
    }]);
    s.client
        .distribute(&s.dist(1), &s.usdc.address, &legs, &s.owner);
    assert_eq!(s.usdc.balance(&s.alice), 3 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), 2 * USDC);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - 5 * USDC);
}

/// A signature for one list pays that list and no other: the same id sent
/// with a different recipient is not covered.
#[test]
fn a_signature_for_one_list_does_not_pay_another() {
    let s = setup();
    let signed = s.payouts(&[(&s.alice, USDC)]);
    let sent = s.payouts(&[(&s.bob, USDC)]);
    let args = (s.dist(1), &s.usdc.address, signed, &s.owner).into_val(&s.e);
    let to_alice = (&s.owner, &s.alice, USDC).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "distribute",
            args,
            sub_invokes: &[MockAuthInvoke {
                contract: &s.usdc.address,
                fn_name: "transfer",
                args: to_alice,
                sub_invokes: &[],
            }],
        },
    }]);
    assert!(s
        .client
        .try_distribute(&s.dist(1), &s.usdc.address, &sent, &s.owner)
        .is_err());
    assert_eq!(s.usdc.balance(&s.bob), 0);
    assert!(!s.client.is_paid(&s.dist(1)));
}

/// A signature for `distribute` alone, without the transfers under it,
/// moves nothing: the asset asks for the owner's authorization per leg.
#[test]
fn a_signature_without_the_transfers_moves_nothing() {
    let s = setup();
    let legs = s.payouts(&[(&s.alice, USDC)]);
    let args = (s.dist(1), &s.usdc.address, legs.clone(), &s.owner).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "distribute",
            args,
            sub_invokes: &[],
        },
    }]);
    assert!(s
        .client
        .try_distribute(&s.dist(1), &s.usdc.address, &legs, &s.owner)
        .is_err());
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert!(!s.client.is_paid(&s.dist(1)));
}
