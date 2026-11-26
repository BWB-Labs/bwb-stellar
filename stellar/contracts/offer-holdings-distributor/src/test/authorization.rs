//! One negative test per privileged entrypoint: a signed but wrong caller
//! fails with the power's own code; an unsigned call fails at
//! authorization. `claim` and the reads run with no auths at all. The
//! positive case uses an exact auth, proving which account must sign and
//! what the signature covers.
use soroban_sdk::{
    testutils::{MockAuth, MockAuthInvoke},
    BytesN, IntoVal,
};

use super::{err, setup, FUNDS, SUPPLY, USDC};
use crate::OfferHoldingsDistributorError;

extern crate std;

/// A funded account with no power cannot deposit, even its own money.
#[test]
fn only_the_owner_deposits() {
    let s = setup();
    for caller in [&s.pauser, &s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_deposit(&USDC, caller)),
            OfferHoldingsDistributorError::NotOwner.into()
        );
    }
    assert_eq!(s.pot(), 0);
    assert_eq!(s.usdc.balance(&s.stranger), FUNDS);
}

#[test]
fn only_owner_or_pauser_pauses_and_only_the_owner_unpauses() {
    let s = setup();
    for caller in [&s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_pause(caller)),
            OfferHoldingsDistributorError::NotPauser.into()
        );
    }
    s.client.pause(&s.pauser);
    for caller in [&s.pauser, &s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            err(s.client.try_unpause(caller)),
            OfferHoldingsDistributorError::NotOwner.into()
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
            OfferHoldingsDistributorError::NotUpgrader.into()
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

needs_signature!(deposit_needs_the_owners_signature, |s| s.deposit(USDC));
needs_signature!(withdraw_needs_the_owners_signature, |s| { s.deposit(USDC); }; s
    .client
    .withdraw(&s.usdc.address, &USDC, &s.owner));
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
    assert_eq!(s.client.claimable(&s.alice), 0);
    assert_eq!(s.client.credited(&s.alice), 0);
    assert_eq!(s.client.cumulative_per_quota(), 0);
    assert_eq!(s.client.powers().owner, s.owner);
    assert_eq!(s.client.contracts().payment_asset, s.usdc.address);
    assert!(!s.client.paused());
    assert_eq!(s.client.schema_version(), 1);
}

/// The owner signs one authorization for `deposit`, which covers the one
/// transfer the asset makes in its name: this amount, to the distributor.
#[test]
fn the_owners_signature_covers_the_deposit_and_its_one_transfer() {
    let s = setup();
    let amount = SUPPLY * USDC;
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "deposit",
            args: (amount, &s.owner).into_val(&s.e),
            sub_invokes: &[MockAuthInvoke {
                contract: &s.usdc.address,
                fn_name: "transfer",
                args: (&s.owner, &s.id, amount).into_val(&s.e),
                sub_invokes: &[],
            }],
        },
    }]);
    s.deposit(amount);
    assert_eq!(s.pot(), amount);
}

/// A signature for one amount does not pay another.
#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn a_signature_for_another_amount_is_refused() {
    let s = setup();
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "deposit",
            args: (USDC, &s.owner).into_val(&s.e),
            sub_invokes: &[MockAuthInvoke {
                contract: &s.usdc.address,
                fn_name: "transfer",
                args: (&s.owner, &s.id, USDC).into_val(&s.e),
                sub_invokes: &[],
            }],
        },
    }]);
    s.deposit(2 * USDC);
}

/// A withdrawal is signed as the call alone: the distributor pays from its
/// own balance, so the owner authorizes no transfer.
#[test]
fn the_owners_signature_for_a_withdrawal_covers_the_call_alone() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "withdraw",
            args: (&s.usdc.address, USDC, &s.owner).into_val(&s.e),
            sub_invokes: &[],
        },
    }]);
    s.client.withdraw(&s.usdc.address, &USDC, &s.owner);
    assert_eq!(s.pot(), (SUPPLY - 1) * USDC);
}
