//! The distributor needs no balance, but anyone can send it one by mistake.
//! `rescue` is the owner's way to send it out, and the only call that moves
//! money the contract holds. It never touches the owner's own account.
use soroban_sdk::{
    testutils::{
        storage::Instance as _, Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke,
    },
    xdr, Address, Event as _, IntoVal, TryFromVal,
};

use super::{code, err, setup, trapped, GreedyToken, Setup, FUNDS, USDC};
use crate::events::Rescued;
use crate::OfferDistributorError;

extern crate std;

/// The asset contract's own code for "not enough balance".
const BALANCE: u32 = 10;

/// What the stranger sends to the distributor by mistake.
const STRAY: i128 = 5 * USDC;

/// A distributor holding `STRAY` USDC that the stranger sent to it by hand.
fn stranded<'a>() -> Setup<'a> {
    let s = setup();
    s.usdc.transfer(&s.stranger, &s.id, &STRAY);
    s
}

fn try_rescue(s: &Setup, to: &Address, amount: i128, caller: &Address) -> soroban_sdk::Error {
    err(s.client.try_rescue(&s.usdc.address, to, &amount, caller))
}

#[test]
fn the_owner_sends_a_stray_balance_back() {
    let s = stranded();
    s.client
        .rescue(&s.usdc.address, &s.stranger, &STRAY, &s.owner);
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert_eq!(s.usdc.balance(&s.stranger), FUNDS);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

/// The amount is the owner's to choose: a part now, the rest later, to
/// anyone.
#[test]
fn a_rescue_may_take_a_part_of_the_balance() {
    let s = stranded();
    s.client
        .rescue(&s.usdc.address, &s.alice, &(2 * USDC), &s.owner);
    assert_eq!(s.usdc.balance(&s.id), 3 * USDC);
    s.client
        .rescue(&s.usdc.address, &s.bob, &(3 * USDC), &s.owner);
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
    s.client
        .rescue(&s.usdc.address, &s.stranger, &STRAY, &s.owner);
}

/// Mocks exactly one authorization: the owner signing this `rescue`, with
/// nothing under it.
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
/// distributor's balance, which the distributor itself authorizes, so a
/// rescue can never spend from the owner's account.
#[test]
fn the_owners_signature_covers_the_call_and_no_transfer() {
    let s = stranded();
    owner_signs_rescue(&s, &s.usdc.address, &s.stranger, STRAY);
    s.client
        .rescue(&s.usdc.address, &s.stranger, &STRAY, &s.owner);
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
#[test]
fn a_rescue_of_more_than_the_balance_fails_in_the_asset() {
    let s = stranded();
    assert_eq!(
        try_rescue(&s, &s.stranger, STRAY + 1, &s.owner),
        code(BALANCE)
    );
    assert_eq!(s.usdc.balance(&s.id), STRAY);
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

#[test]
fn a_distributor_that_holds_nothing_has_nothing_to_rescue() {
    let s = setup();
    assert_eq!(try_rescue(&s, &s.alice, 1, &s.owner), code(BALANCE));
    assert_eq!(s.usdc.balance(&s.owner), FUNDS);
}

/// The pause stops distributions. A rescue pays nothing from the owner's
/// account, and the owner could unpause anyway.
#[test]
fn a_rescue_works_while_paused() {
    let s = stranded();
    s.client.pause(&s.pauser);
    s.client
        .rescue(&s.usdc.address, &s.stranger, &STRAY, &s.owner);
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
    s.client
        .rescue(&s.usdc.address, &s.stranger, &STRAY, &s.owner);
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
}

/// The asset's own `transfer` first, then `rescued`.
#[test]
fn a_rescue_emits_rescued_after_the_assets_transfer() {
    let s = stranded();
    s.client
        .rescue(&s.usdc.address, &s.stranger, &STRAY, &s.owner);
    let all = s.e.events().all();
    let emitters: std::vec::Vec<Address> = all
        .events()
        .iter()
        .map(|ev| {
            let emitter = xdr::ScAddress::Contract(ev.contract_id.clone().unwrap());
            Address::try_from_val(&s.e, &emitter).unwrap()
        })
        .collect();
    assert_eq!(emitters, [s.usdc.address.clone(), s.id.clone()]);
    let rescued = Rescued {
        asset: s.usdc.address.clone(),
        to: s.stranger.clone(),
        amount: STRAY,
    };
    assert_eq!(
        all.filter_by_contract(&s.id).events().to_vec(),
        [rescued.to_xdr(&s.e, &s.id)]
    );
}

#[test]
fn a_refused_rescue_emits_nothing() {
    let s = stranded();
    try_rescue(&s, &s.stranger, 0, &s.owner);
    assert!(s
        .e
        .events()
        .all()
        .filter_by_contract(&s.id)
        .events()
        .is_empty());
}

#[test]
fn a_rescue_after_a_quiet_month_tops_the_instance_up() {
    let s = stranded();
    let ttl = || {
        s.e.as_contract(&s.id, || s.e.storage().instance().get_ttl())
    };
    let max = s.e.as_contract(&s.id, || s.e.storage().max_ttl());
    s.e.ledger().with_mut(|l| l.sequence_number += 40 * 17_280);
    assert!(ttl() < max);
    s.client
        .rescue(&s.usdc.address, &s.stranger, &STRAY, &s.owner);
    assert_eq!(ttl(), max);
}
