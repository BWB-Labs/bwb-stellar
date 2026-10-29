//! One negative test per privileged entrypoint: a signed but wrong caller
//! fails with the power's own code; an unsigned call fails at
//! authorization. The open calls run with no auths at all. Positive cases
//! use exact auths, proving which account must sign.
use soroban_sdk::{
    testutils::{MockAuth, MockAuthInvoke},
    BytesN, IntoVal,
};

use super::{err, setup, setup_preparing, PRICE, SUCCESS_TOKENS};
use crate::{OfferSaleError, SaleState};

#[test]
fn only_the_owner_closes_reopens_and_cancels() {
    let s = setup();
    for caller in [&s.platform, &s.hot_key, &s.alice] {
        assert_eq!(
            err(s.client.try_close(caller)),
            OfferSaleError::NotOwner.into()
        );
        assert_eq!(
            err(s.client.try_cancel(caller)),
            OfferSaleError::NotOwner.into()
        );
    }
    s.buy(&s.alice, SUCCESS_TOKENS);
    s.client.close(&s.owner);
    for caller in [&s.platform, &s.hot_key, &s.alice] {
        assert_eq!(
            err(s.client.try_reopen(caller)),
            OfferSaleError::NotOwner.into()
        );
    }
}

#[test]
fn only_the_owner_withdraws() {
    let s = setup();
    s.successful();
    let legs = s.payouts(&[(&s.owner, 1)]);
    for caller in [&s.platform, &s.hot_key, &s.alice] {
        assert_eq!(
            err(s.client.try_withdraw_payment(&legs, caller)),
            OfferSaleError::NotOwner.into()
        );
    }
    let s = setup();
    s.failed();
    for caller in [&s.platform, &s.hot_key, &s.alice] {
        assert_eq!(
            err(s.client.try_withdraw_tokens(caller, &1, caller)),
            OfferSaleError::NotOwner.into()
        );
    }
}

#[test]
fn only_owner_or_pauser_pauses_and_only_the_owner_unpauses() {
    let s = setup();
    for caller in [&s.hot_key, &s.alice] {
        assert_eq!(
            err(s.client.try_pause(caller)),
            OfferSaleError::NotPauser.into()
        );
    }
    s.client.pause(&s.platform);
    for caller in [&s.platform, &s.hot_key, &s.alice] {
        assert_eq!(
            err(s.client.try_unpause(caller)),
            OfferSaleError::NotOwner.into()
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
    for caller in [&s.owner, &s.hot_key, &s.alice] {
        assert_eq!(
            err(s.client.try_upgrade(&hash, caller)),
            OfferSaleError::NotUpgrader.into()
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

needs_signature!(buy_needs_the_investors_signature, |s| s
    .client
    .buy(&s.alice, &PRICE));
needs_signature!(cooling_off_refund_needs_the_investors_signature, |s| { s.buy(&s.alice, 1); }; s
    .client
    .cooling_off_refund(&s.alice));
needs_signature!(close_needs_the_owners_signature, |s| s
    .client
    .close(&s.owner));
needs_signature!(reopen_needs_the_owners_signature, |s| {
    s.buy(&s.alice, SUCCESS_TOKENS);
    s.client.close(&s.owner);
}; s
    .client
    .reopen(&s.owner));
needs_signature!(cancel_needs_the_owners_signature, |s| s
    .client
    .cancel(&s.owner));
needs_signature!(withdraw_payment_needs_the_owners_signature, |s| { s.successful(); }; s
    .client
    .withdraw_payment(&s.payouts(&[(&s.owner, 1)]), &s.owner));
needs_signature!(withdraw_tokens_needs_the_owners_signature, |s| { s.failed(); }; s
    .client
    .withdraw_tokens(&s.owner, &1, &s.owner));
needs_signature!(pause_needs_the_pausers_signature, |s| s
    .client
    .pause(&s.platform));
needs_signature!(unpause_needs_the_owners_signature, |s| { s.client.pause(&s.platform); }; s
    .client
    .unpause(&s.owner));
needs_signature!(upgrade_needs_the_upgraders_signature, |s| s
    .client
    .upgrade(&BytesN::from_array(&s.e, &[0; 32]), &s.platform));

#[test]
fn activate_finalize_release_and_refund_need_no_authorization() {
    let s = setup_preparing();
    s.e.set_auths(&[]);
    s.client.activate();
    s.e.mock_all_auths();
    s.allow(&s.alice, 1_000 * PRICE);
    s.fund(&s.alice, 1_000 * PRICE);
    s.buy(&s.alice, 750);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.e.set_auths(&[]);
    // The burn inside finalize and the transfer inside release are the
    // sale's own, as the direct caller: no entry needed.
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Successful);
    s.client.release(&s.investors(&[&s.alice]));
    assert_eq!(s.token.balance(&s.alice), 750);

    let s = setup();
    s.failed();
    s.e.set_auths(&[]);
    s.client.refund(&s.investors(&[&s.alice]));
    assert_eq!(s.usdc.balance(&s.alice), super::SUPPLY * PRICE);
}

/// The investor signs one authorization for `buy`, which covers the
/// payment transfer the sale makes in their name (interface 4.2).
#[test]
fn the_investors_signature_alone_buys() {
    let s = setup();
    let amount = 3 * PRICE;
    let args = (&s.alice, amount).into_val(&s.e);
    let sub_args = (&s.alice, &s.id, amount).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.alice,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "buy",
            args,
            sub_invokes: &[MockAuthInvoke {
                contract: &s.usdc.address,
                fn_name: "transfer",
                args: sub_args,
                sub_invokes: &[],
            }],
        },
    }]);
    s.client.buy(&s.alice, &amount);
    assert_eq!(s.client.reservation(&s.alice).unwrap().tokens, 3);
}

#[test]
fn the_owners_signature_alone_closes() {
    let s = setup();
    s.buy(&s.alice, SUCCESS_TOKENS);
    let args = (&s.owner,).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "close",
            args,
            sub_invokes: &[],
        },
    }]);
    s.client.close(&s.owner);
    assert_eq!(s.client.state(), SaleState::Closing);
}

#[test]
fn nobody_can_refund_in_an_investors_name_through_cooling_off() {
    let s = setup();
    s.buy(&s.alice, 1);
    s.e.set_auths(&[]);
    assert!(s.client.try_cooling_off_refund(&s.alice).is_err());
    assert_eq!(s.client.reservation(&s.alice).unwrap().tokens, 1);
}
