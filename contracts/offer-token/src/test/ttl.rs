//! A quiet contract must not archive between touches: every call tops the
//! instance and the entries it touches up to the network maximum once they
//! fall a month below it.
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Ledger as _,
    },
    Address, Env, String,
};
use stellar_tokens::fungible::FungibleStorageKey;

use super::setup;

const DAY: u32 = 17_280;

fn instance_ttl(e: &Env, id: &Address) -> u32 {
    e.as_contract(id, || e.storage().instance().get_ttl())
}

fn balance_ttl(e: &Env, id: &Address, who: &Address) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&FungibleStorageKey::Balance(who.clone()))
    })
}

fn whitelist_ttl(e: &Env, id: &Address, who: &Address) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&crate::storage::DataKey::Whitelisted(who.clone()))
    })
}

fn max_ttl(e: &Env, id: &Address) -> u32 {
    e.as_contract(id, || e.storage().max_ttl())
}

fn advance(e: &Env, days: u32) {
    e.ledger().with_mut(|l| l.sequence_number += days * DAY);
}

#[test]
fn construction_extends_the_instance_to_the_maximum() {
    let s = setup();
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn a_call_after_a_quiet_month_tops_the_instance_up_again() {
    let s = setup();
    advance(&s.e, 40);
    assert!(instance_ttl(&s.e, &s.id) < max_ttl(&s.e, &s.id));
    s.client
        .set_offer_uri(&String::from_str(&s.e, "ipfs://later"), &s.admin);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn a_call_within_the_month_does_not_pay_rent_again() {
    let s = setup();
    let before = instance_ttl(&s.e, &s.id);
    advance(&s.e, 10);
    s.client
        .set_offer_uri(&String::from_str(&s.e, "ipfs://soon"), &s.admin);
    assert_eq!(instance_ttl(&s.e, &s.id), before - 10 * DAY);
}

#[test]
fn transfers_extend_both_balances_to_the_maximum() {
    let s = setup();
    advance(&s.e, 40);
    s.client.transfer(&s.alice, &s.listed, &1);
    assert_eq!(balance_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
    assert_eq!(balance_ttl(&s.e, &s.id, &s.listed), max_ttl(&s.e, &s.id));
}

#[test]
fn transfer_from_extends_both_balances_to_the_maximum() {
    let s = setup();
    let spender = Address::generate(&s.e);
    s.client.approve(&s.alice, &spender, &5, &(60 * DAY));
    advance(&s.e, 40);
    s.client.transfer_from(&spender, &s.alice, &s.listed, &1);
    assert_eq!(balance_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
    assert_eq!(balance_ttl(&s.e, &s.id, &s.listed), max_ttl(&s.e, &s.id));
}

#[test]
fn a_transfer_extends_the_whitelist_entry_it_checked() {
    let s = setup();
    advance(&s.e, 40);
    assert!(whitelist_ttl(&s.e, &s.id, &s.listed) < max_ttl(&s.e, &s.id));
    s.client.transfer(&s.alice, &s.listed, &1);
    assert_eq!(whitelist_ttl(&s.e, &s.id, &s.listed), max_ttl(&s.e, &s.id));
}

#[test]
fn a_burn_extends_the_holders_balance_and_the_instance() {
    let s = setup();
    advance(&s.e, 40);
    assert!(balance_ttl(&s.e, &s.id, &s.sale) < max_ttl(&s.e, &s.id));
    s.client.burn(&s.sale, &1);
    assert_eq!(balance_ttl(&s.e, &s.id, &s.sale), max_ttl(&s.e, &s.id));
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn whitelisting_writes_the_entry_with_the_maximum_ttl() {
    let s = setup();
    advance(&s.e, 40);
    s.client.set_transfer_whitelist(&s.alice, &true, &s.admin);
    assert_eq!(whitelist_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
}

#[test]
fn forced_transfers_extend_both_balances_to_the_maximum() {
    let s = setup();
    advance(&s.e, 40);
    s.client.admin_transfer(&s.alice, &s.bob, &1, &s.xfer_admin);
    assert_eq!(balance_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
    assert_eq!(balance_ttl(&s.e, &s.id, &s.bob), max_ttl(&s.e, &s.id));
}
