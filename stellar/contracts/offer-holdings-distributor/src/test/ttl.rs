//! A quiet contract must not archive between touches: every state-changing
//! call tops the instance up to the network maximum once it falls a month
//! below it, and a holder's record is written with the maximum lifetime.
//! Reads extend nothing.
use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Ledger as _,
    },
    Address, Env,
};

use super::{setup, SUPPLY, USDC};
use crate::storage::DataKey;

const DAY: u32 = 17_280;

fn instance_ttl(e: &Env, id: &Address) -> u32 {
    e.as_contract(id, || e.storage().instance().get_ttl())
}

fn record_ttl(e: &Env, id: &Address, holder: &Address) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&DataKey::Credited(holder.clone()))
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
fn a_deposit_after_a_quiet_month_tops_the_instance_up_again() {
    let s = setup();
    advance(&s.e, 40);
    assert!(instance_ttl(&s.e, &s.id) < max_ttl(&s.e, &s.id));
    s.deposit(USDC);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

/// The top-up waits until the instance has fallen a month below the
/// maximum: a call ten days after the last one extends nothing.
#[test]
fn a_deposit_within_the_month_does_not_extend_the_instance() {
    let s = setup();
    advance(&s.e, 10);
    let before = instance_ttl(&s.e, &s.id);
    assert_eq!(before, max_ttl(&s.e, &s.id) - 10 * DAY);
    s.deposit(USDC);
    assert_eq!(instance_ttl(&s.e, &s.id), before);
}

#[test]
fn a_claim_and_a_withdrawal_top_the_instance_up() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    advance(&s.e, 40);
    s.claim(&[&s.alice]);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
    advance(&s.e, 40);
    s.client.withdraw(&s.usdc.address, &USDC, &s.owner);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn a_paid_holders_record_is_written_with_the_maximum_ttl() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    advance(&s.e, 40);
    s.claim(&[&s.alice]);
    assert_eq!(record_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
}

#[test]
fn pause_and_unpause_top_the_instance_up() {
    let s = setup();
    advance(&s.e, 40);
    s.client.pause(&s.pauser);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
    advance(&s.e, 40);
    s.client.unpause(&s.owner);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn reads_extend_nothing() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice]);
    advance(&s.e, 40);
    let instance = instance_ttl(&s.e, &s.id);
    let record = record_ttl(&s.e, &s.id, &s.alice);
    s.client.claimable(&s.alice);
    s.client.credited(&s.alice);
    s.client.cumulative_per_quota();
    s.client.contracts();
    s.client.powers();
    s.client.paused();
    s.client.schema_version();
    assert_eq!(instance_ttl(&s.e, &s.id), instance);
    assert_eq!(record_ttl(&s.e, &s.id, &s.alice), record);
}
