//! The distributor needs no balance, but anyone can send it one by mistake.
//! `rescue` is the owner's way to send it out, and the only call that moves
//! money the contract holds. It never touches the owner's own account.
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    Address, Event as _, IntoVal,
};

use super::ttl::{advance, instance_ttl, max_ttl};
use super::{
    code, emitted, emitters, err, setup, trapped, DrainingToken, GreedyToken, Setup, FUNDS, USDC,
};
use crate::events::Rescued;
use crate::OfferDistributorError;

/// The asset contract's own codes, which surface unchanged.
const BALANCE: u32 = 10;
const BALANCE_DEAUTHORIZED: u32 = 11;
const TRUSTLINE_MISSING: u32 = 13;

/// What the stranger sends to the distributor by mistake.
const STRAY: i128 = 5 * USDC;

/// A distributor holding `STRAY` USDC that the stranger sent to it by hand.
fn stranded<'a>() -> Setup<'a> {
    let s = setup();
    s.usdc.transfer(&s.stranger, &s.id, &STRAY);
    s
}

/// The owner rescues `amount` of USDC to `to`.
fn rescue(s: &Setup, to: &Address, amount: i128) {
    s.client.rescue(&s.usdc.address, to, &amount, &s.owner);
}

fn try_rescue(s: &Setup, to: &Address, amount: i128, caller: &Address) -> soroban_sdk::Error {
    err(s.client.try_rescue(&s.usdc.address, to, &amount, caller))
}

#[test]
fn the_owner_sends_a_stray_balance_back() {
    let s = stranded();
    rescue(&s, &s.stranger, STRAY);
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert_eq!(s.usdc.balance(&s.stranger), FUNDS);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

/// The amount is the owner's to choose: a part now, the rest later, to
/// anyone.
#[test]
fn a_rescue_may_take_a_part_of_the_balance() {
    let s = stranded();
    rescue(&s, &s.alice, 2 * USDC);
    assert_eq!(s.usdc.balance(&s.id), 3 * USDC);
    rescue(&s, &s.bob, 3 * USDC);
    assert_eq!(s.usdc.balance(&s.alice), 2 * USDC);
    assert_eq!(s.usdc.balance(&s.bob), 3 * USDC);
    assert_eq!(s.usdc.balance(&s.id), 0);
}

/// Not even the account that sent the money can take it back itself.
#[test]
fn only_the_owner_rescues() {
    let s = stranded();
    for caller in [&s.pauser, &s.upgrader, &s.stranger, &s.alice] {
        assert_eq!(
            try_rescue(&s, caller, STRAY, caller),
            OfferDistributorError::NotOwner.into()
        );
    }
    assert_eq!(s.usdc.balance(&s.id), STRAY);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn rescue_needs_the_owners_signature() {
    let s = stranded();
    s.e.set_auths(&[]);
    rescue(&s, &s.stranger, STRAY);
}

/// Mocks exactly one authorization: the owner signing this `rescue`, with
/// nothing under it. This is the whole tree a treasury signs for a rescue.
fn owner_signs_rescue(s: &Setup, asset: &Address, to: &Address, amount: i128) {
    s.e.mock_auths(&[MockAuth {
        address: &s.owner,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "rescue",
            args: (asset, to, amount, &s.owner).into_val(&s.e),
            sub_invokes: &[],
        },
    }]);
}

/// The owner signs the call and no transfer: the money leaves the
/// distributor's balance, which the distributor itself authorizes.
#[test]
fn the_owners_signature_covers_the_call_and_no_transfer() {
    let s = stranded();
    owner_signs_rescue(&s, &s.usdc.address, &s.stranger, STRAY);
    rescue(&s, &s.stranger, STRAY);
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

#[test]
fn a_rescue_of_zero_or_less_is_refused() {
    let s = stranded();
    for amount in [0, -1, i128::MIN] {
        assert_eq!(
            try_rescue(&s, &s.stranger, amount, &s.owner),
            OfferDistributorError::NonPositiveAmount.into()
        );
    }
    assert_eq!(s.usdc.balance(&s.id), STRAY);
}

/// A rescue to the distributor itself moves nothing, and one to the
/// asset's own contract would strand the money for good.
#[test]
fn a_rescue_to_the_distributor_or_to_the_asset_is_refused() {
    let s = stranded();
    for to in [&s.id, &s.usdc.address] {
        assert_eq!(
            try_rescue(&s, to, STRAY, &s.owner),
            OfferDistributorError::InvalidRecipient.into()
        );
    }
    assert_eq!(s.usdc.balance(&s.id), STRAY);
}

/// More than the distributor holds fails in the asset, and moves nothing:
/// the owner's own balance is never drawn on to make up the difference.
/// A distributor that holds nothing is the same case.
#[test]
fn a_rescue_of_more_than_the_balance_fails_in_the_asset() {
    for (s, amount) in [(stranded(), STRAY + 1), (setup(), 1)] {
        let held = s.usdc.balance(&s.id);
        assert_eq!(try_rescue(&s, &s.alice, amount, &s.owner), code(BALANCE));
        assert_eq!(s.usdc.balance(&s.id), held);
        assert_eq!(s.usdc.balance(&s.owner), FUNDS);
    }
}

/// The asset refuses a `to` it would refuse in a distribution: an account
/// with no trustline, a balance its issuer froze.
#[test]
fn a_rescue_to_a_recipient_the_asset_refuses_fails_with_the_assets_code() {
    let s = stranded();
    let no_trustline = s.create_account(1);
    assert_eq!(
        try_rescue(&s, &no_trustline, STRAY, &s.owner),
        code(TRUSTLINE_MISSING)
    );
    s.usdc_admin.set_authorized(&s.bob, &false);
    assert_eq!(
        try_rescue(&s, &s.bob, STRAY, &s.owner),
        code(BALANCE_DEAUTHORIZED)
    );
    assert_eq!(s.usdc.balance(&s.id), STRAY);
}

/// The issuer can freeze the distributor's own balance. The owner then
/// cannot rescue it: only the issuer gives it back.
#[test]
fn a_balance_the_issuer_froze_cannot_be_rescued() {
    let s = stranded();
    s.usdc_admin.set_authorized(&s.id, &false);
    assert_eq!(
        try_rescue(&s, &s.stranger, STRAY, &s.owner),
        code(BALANCE_DEAUTHORIZED)
    );
    s.usdc_admin.set_authorized(&s.id, &true);
    rescue(&s, &s.stranger, STRAY);
    assert_eq!(s.usdc.balance(&s.id), 0);
}

/// The pause stops distributions. A rescue pays nothing from the owner's
/// account, and the owner could unpause anyway.
#[test]
fn a_rescue_works_while_paused() {
    let s = stranded();
    s.client.pause(&s.pauser);
    rescue(&s, &s.stranger, STRAY);
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert!(s.client.paused());
}

/// A rescue is not a distribution: it uses no id and leaves the
/// distributor working as before.
#[test]
fn a_rescue_leaves_distributions_as_they_were() {
    let s = stranded();
    s.distribute(1, &[(&s.alice, USDC)]);
    // The distribution paid from the owner, not from the stray balance.
    assert_eq!(s.usdc.balance(&s.id), STRAY);
    rescue(&s, &s.stranger, STRAY);
    assert!(s.client.is_paid(&s.dist(1)));
    assert!(!s.client.is_paid(&s.dist(2)));
    s.distribute(2, &[(&s.bob, USDC)]);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - 2 * USDC);
}

/// The asset is whatever address the owner names. A contract that, asked
/// to `transfer`, tries to move the distributor's real USDC to a thief
/// gets nothing: the distributor authorizes only the call it makes itself.
#[test]
fn a_hostile_asset_cannot_take_another_assets_balance() {
    let s = stranded();
    let thief = Address::generate(&s.e);
    let greedy =
        s.e.register(GreedyToken, (s.usdc.address.clone(), thief.clone()));
    owner_signs_rescue(&s, &greedy, &s.alice, STRAY);
    assert_eq!(
        err(s.client.try_rescue(&greedy, &s.alice, &STRAY, &s.owner)),
        trapped()
    );
    assert_eq!(s.usdc.balance(&thief), 0);
    assert_eq!(s.usdc.balance(&s.id), STRAY);

    // The control: the same contract does steal once the test environment
    // grants the distributor's authorization where the distributor is not
    // the caller, which no transaction can do for a contract's address.
    // The refusal above is the authorization, not a broken thief.
    s.e.mock_all_auths_allowing_non_root_auth();
    s.client.rescue(&greedy, &s.alice, &STRAY, &s.owner);
    assert_eq!(s.usdc.balance(&thief), STRAY);
}

/// A wrong asset can also reach for the owner's own USDC. Signed with the
/// tree a rescue has, the call alone, it gets nothing. This is why the
/// signing side builds or checks that tree and never signs whatever a
/// simulation recorded: under a recorded tree the theft below passes.
#[test]
fn a_hostile_asset_gets_the_owners_money_only_under_a_recorded_tree() {
    let s = stranded();
    let thief = Address::generate(&s.e);
    let draining = s.e.register(
        DrainingToken,
        (s.usdc.address.clone(), s.owner.clone(), thief.clone()),
    );
    owner_signs_rescue(&s, &draining, &s.alice, STRAY);
    assert_eq!(
        err(s.client.try_rescue(&draining, &s.alice, &STRAY, &s.owner)),
        trapped()
    );
    assert_eq!(s.usdc.balance(&thief), 0);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);

    // Every authorization granted, as a recording simulation grants it.
    s.e.mock_all_auths();
    s.client.rescue(&draining, &s.alice, &STRAY, &s.owner);
    assert_eq!(s.usdc.balance(&thief), STRAY);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS - STRAY);
}

/// The asset's own `transfer` first, then `rescued`.
#[test]
fn a_rescue_emits_rescued_after_the_assets_transfer() {
    let s = stranded();
    rescue(&s, &s.stranger, STRAY);
    assert_eq!(emitters(&s.e), [s.usdc.address.clone(), s.id.clone()]);
    let rescued = Rescued {
        asset: s.usdc.address.clone(),
        to: s.stranger.clone(),
        amount: STRAY,
    };
    assert_eq!(emitted(&s.e, &s.id), [rescued.to_xdr(&s.e, &s.id)]);
}

#[test]
fn a_refused_rescue_emits_nothing() {
    let s = stranded();
    assert_eq!(
        try_rescue(&s, &s.stranger, 0, &s.owner),
        OfferDistributorError::NonPositiveAmount.into()
    );
    assert!(emitted(&s.e, &s.id).is_empty());
}

#[test]
fn a_rescue_after_a_quiet_month_tops_the_instance_up() {
    let s = stranded();
    advance(&s.e, 40);
    assert!(instance_ttl(&s.e, &s.id) < max_ttl(&s.e, &s.id));
    rescue(&s, &s.stranger, STRAY);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}
