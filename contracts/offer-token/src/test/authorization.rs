//! One negative test per privileged entrypoint: a signed but wrong caller
//! fails with the power's own code; an unsigned call fails at
//! authorization. Positive cases use exact auths, proving which account
//! must sign.
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    Address, BytesN, IntoVal, String,
};

use super::{err, setup};
use crate::OfferTokenError;

#[test]
fn only_admin_manages_the_whitelist() {
    let s = setup();
    for caller in [&s.xfer_admin, &s.pauser, &s.upgrader, &s.alice] {
        let r = s.client.try_set_transfer_whitelist(&s.alice, &true, caller);
        assert_eq!(err(r), OfferTokenError::NotAdmin.into());
    }
}

#[test]
fn only_admin_replaces_the_uri() {
    let s = setup();
    let uri = String::from_str(&s.e, "ipfs://x");
    for caller in [&s.xfer_admin, &s.pauser, &s.upgrader, &s.alice] {
        let r = s.client.try_set_offer_uri(&uri, caller);
        assert_eq!(err(r), OfferTokenError::NotAdmin.into());
    }
}

#[test]
fn only_xfer_admin_forces_transfers() {
    let s = setup();
    for caller in [&s.admin, &s.pauser, &s.upgrader, &s.alice] {
        let r = s.client.try_admin_transfer(&s.alice, &s.bob, &1, caller);
        assert_eq!(err(r), OfferTokenError::NotXferAdmin.into());
    }
}

#[test]
fn only_admin_or_pauser_pauses() {
    let s = setup();
    for caller in [&s.xfer_admin, &s.upgrader, &s.alice] {
        let r = s.client.try_pause(caller);
        assert_eq!(err(r), OfferTokenError::NotPauser.into());
    }
}

#[test]
fn pauser_cannot_unpause() {
    let s = setup();
    s.client.pause(&s.pauser);
    for caller in [&s.pauser, &s.xfer_admin, &s.upgrader, &s.alice] {
        let r = s.client.try_unpause(caller);
        assert_eq!(err(r), OfferTokenError::NotAdmin.into());
    }
    s.client.unpause(&s.admin);
    assert!(!s.client.paused());
}

#[test]
fn only_upgrader_upgrades() {
    let s = setup();
    let hash = BytesN::from_array(&s.e, &[0; 32]);
    for caller in [&s.admin, &s.xfer_admin, &s.pauser, &s.alice] {
        let r = s.client.try_upgrade(&hash, caller);
        assert_eq!(err(r), OfferTokenError::NotUpgrader.into());
    }
}

/// A missing signature fails in the host's authorization check. A direct
/// call surfaces it as `Error(Auth, InvalidAction)`; a `try_` call rewraps
/// non-contract errors, so these tests use the direct call.
macro_rules! needs_signature {
    ($name:ident, |$s:ident| $call:expr) => {
        #[test]
        #[should_panic(expected = "Error(Auth, InvalidAction)")]
        fn $name() {
            let $s = setup();
            $s.e.set_auths(&[]);
            $call;
        }
    };
}

needs_signature!(whitelist_needs_admins_signature, |s| s
    .client
    .set_transfer_whitelist(&s.alice, &true, &s.admin));
needs_signature!(uri_needs_admins_signature, |s| s
    .client
    .set_offer_uri(&String::from_str(&s.e, "ipfs://x"), &s.admin));
needs_signature!(forced_transfer_needs_xfer_admins_signature, |s| s
    .client
    .admin_transfer(&s.alice, &s.bob, &1, &s.xfer_admin));
needs_signature!(pause_needs_pausers_signature, |s| s.client.pause(&s.pauser));
needs_signature!(unpause_needs_admins_signature, |s| s
    .client
    .unpause(&s.admin));
needs_signature!(upgrade_needs_upgraders_signature, |s| s
    .client
    .upgrade(&BytesN::from_array(&s.e, &[0; 32]), &s.upgrader));
needs_signature!(investor_transfer_needs_the_investors_signature, |s| s
    .client
    .transfer(&s.alice, &s.listed, &1));

#[test]
fn admin_signature_alone_manages_the_whitelist() {
    let s = setup();
    let args = (&s.alice, true, &s.admin).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.admin,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "set_transfer_whitelist",
            args,
            sub_invokes: &[],
        },
    }]);
    s.client.set_transfer_whitelist(&s.alice, &true, &s.admin);
    assert!(s.client.is_transfer_whitelisted(&s.alice));
}

#[test]
fn xfer_admin_signature_alone_forces_a_transfer() {
    let s = setup();
    let args = (&s.alice, &s.bob, 5_i128, &s.xfer_admin).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.xfer_admin,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "admin_transfer",
            args,
            sub_invokes: &[],
        },
    }]);
    s.client.admin_transfer(&s.alice, &s.bob, &5, &s.xfer_admin);
    assert_eq!(s.client.balance(&s.bob), 105);
}

#[test]
fn pauser_signature_alone_pauses() {
    let s = setup();
    let args = (&s.pauser,).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.pauser,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "pause",
            args,
            sub_invokes: &[],
        },
    }]);
    s.client.pause(&s.pauser);
    assert!(s.client.paused());
}

#[test]
fn a_stranger_cannot_reach_any_power() {
    let s = setup();
    let mallory = Address::generate(&s.e);
    assert_eq!(
        err(s
            .client
            .try_set_transfer_whitelist(&mallory, &true, &mallory)),
        OfferTokenError::NotAdmin.into()
    );
    assert_eq!(
        err(s.client.try_admin_transfer(&s.sale, &mallory, &1, &mallory)),
        OfferTokenError::NotXferAdmin.into()
    );
    assert_eq!(
        err(s.client.try_pause(&mallory)),
        OfferTokenError::NotPauser.into()
    );
}
