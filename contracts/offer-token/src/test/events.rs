//! Each entrypoint emits exactly the events the token document lists.
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    vec, xdr, Address, Env, String, Symbol, TryFromVal,
};

use super::{ctor_args, parties, setup, SUPPLY, URI};
use crate::events::{AdminTransfer, Initialized, OfferUriUpdated, PauseChanged, WhitelistUpdated};
use crate::OfferToken;
use soroban_sdk::Event as _;

extern crate std;
use std::vec::Vec;

/// The events the token emitted in the last invocation.
fn emitted(e: &Env, id: &Address) -> Vec<xdr::ContractEvent> {
    e.events().all().filter_by_contract(id).events().to_vec()
}

/// The first topic of each event the token emitted, i.e. the event names.
fn names(e: &Env, id: &Address) -> Vec<Symbol> {
    emitted(e, id)
        .iter()
        .map(|ev| {
            let xdr::ContractEventBody::V0(body) = &ev.body;
            let topic = soroban_sdk::Val::try_from_val(e, &body.topics[0]).unwrap();
            Symbol::try_from_val(e, &topic).unwrap()
        })
        .collect()
}

fn syms(e: &Env, names: &[&str]) -> Vec<Symbol> {
    names.iter().map(|n| Symbol::new(e, n)).collect()
}

#[test]
fn construction_emits_initialized_mint_and_one_whitelist_update_per_account() {
    let e = Env::default();
    let p = parties(&e);
    let listed = Address::generate(&e);
    let list = vec![&e, listed.clone(), p.sale.clone(), listed.clone()];
    let id = e.register(OfferToken, ctor_args(&e, &p, SUPPLY, list));

    let evs = emitted(&e, &id);
    assert_eq!(
        names(&e, &id),
        syms(
            &e,
            &[
                "initialized",
                "mint",
                "whitelist_updated",
                "whitelist_updated"
            ]
        )
    );
    let init = Initialized {
        admin: p.admin.clone(),
        xfer_admin: p.xfer_admin.clone(),
        pauser: p.pauser.clone(),
        upgrader: p.upgrader.clone(),
        initial_holder: p.sale.clone(),
        supply: SUPPLY,
        offer_uri: String::from_str(&e, URI),
    };
    assert_eq!(evs[0], init.to_xdr(&e, &id));
    let sale = WhitelistUpdated {
        account: p.sale.clone(),
        allowed: true,
    };
    let other = WhitelistUpdated {
        account: listed,
        allowed: true,
    };
    assert_eq!(evs[2], sale.to_xdr(&e, &id));
    assert_eq!(evs[3], other.to_xdr(&e, &id));
}

#[test]
fn whitelist_change_emits_whitelist_updated_every_time() {
    let s = setup();
    s.client.set_transfer_whitelist(&s.alice, &false, &s.admin);
    let ev = WhitelistUpdated {
        account: s.alice.clone(),
        allowed: false,
    };
    assert_eq!(emitted(&s.e, &s.id), [ev.to_xdr(&s.e, &s.id)]);
}

#[test]
fn uri_change_emits_offer_uri_updated() {
    let s = setup();
    let uri = String::from_str(&s.e, "ipfs://v2");
    s.client.set_offer_uri(&uri, &s.admin);
    let ev = OfferUriUpdated { uri };
    assert_eq!(emitted(&s.e, &s.id), [ev.to_xdr(&s.e, &s.id)]);
}

#[test]
fn pause_and_unpause_record_the_caller() {
    let s = setup();
    s.client.pause(&s.pauser);
    assert_eq!(names(&s.e, &s.id), syms(&s.e, &["paused", "pause_changed"]));
    let ev = PauseChanged {
        caller: s.pauser.clone(),
        paused: true,
    };
    assert_eq!(emitted(&s.e, &s.id)[1], ev.to_xdr(&s.e, &s.id));

    s.client.unpause(&s.admin);
    assert_eq!(
        names(&s.e, &s.id),
        syms(&s.e, &["unpaused", "pause_changed"])
    );
    let ev = PauseChanged {
        caller: s.admin.clone(),
        paused: false,
    };
    assert_eq!(emitted(&s.e, &s.id)[1], ev.to_xdr(&s.e, &s.id));
}

#[test]
fn forced_transfer_emits_a_standard_transfer_and_admin_transfer() {
    let s = setup();
    s.client.admin_transfer(&s.alice, &s.bob, &7, &s.xfer_admin);
    assert_eq!(
        names(&s.e, &s.id),
        syms(&s.e, &["transfer", "admin_transfer"])
    );
    let ev = AdminTransfer {
        from: s.alice.clone(),
        to: s.bob.clone(),
        amount: 7,
    };
    assert_eq!(emitted(&s.e, &s.id)[1], ev.to_xdr(&s.e, &s.id));
}

#[test]
fn ordinary_transfer_emits_only_the_standard_transfer() {
    let s = setup();
    s.client.transfer(&s.alice, &s.listed, &1);
    assert_eq!(names(&s.e, &s.id), syms(&s.e, &["transfer"]));
}
