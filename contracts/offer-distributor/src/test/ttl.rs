//! A quiet contract must not archive between touches: every state-changing
//! call tops the instance up to the network maximum once it falls a month
//! below it, and a paid id's marker is written with the maximum lifetime.
//! Reads extend nothing.
use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Ledger as _,
    },
    Address, BytesN, Env,
};

use super::{setup, USDC};
use crate::storage::DataKey;

const DAY: u32 = 17_280;

fn instance_ttl(e: &Env, id: &Address) -> u32 {
    e.as_contract(id, || e.storage().instance().get_ttl())
}

fn marker_ttl(e: &Env, id: &Address, dist: &BytesN<32>) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&DataKey::Paid(dist.clone()))
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
fn a_distribution_after_a_quiet_month_tops_the_instance_up_again() {
    let s = setup();
    advance(&s.e, 40);
    assert!(instance_ttl(&s.e, &s.id) < max_ttl(&s.e, &s.id));
    s.distribute(1, &[(&s.alice, USDC)]);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

/// The top-up waits until the instance has fallen a month below the
/// maximum: a call ten days after the last one extends nothing.
#[test]
fn a_distribution_within_the_month_does_not_extend_the_instance() {
    let s = setup();
    advance(&s.e, 10);
    let before = instance_ttl(&s.e, &s.id);
    assert_eq!(before, max_ttl(&s.e, &s.id) - 10 * DAY);
    s.distribute(1, &[(&s.alice, USDC)]);
    assert_eq!(instance_ttl(&s.e, &s.id), before);
}

#[test]
fn a_paid_id_is_recorded_with_the_maximum_ttl() {
    let s = setup();
    advance(&s.e, 40);
    s.distribute(1, &[(&s.alice, USDC)]);
    assert_eq!(marker_ttl(&s.e, &s.id, &s.dist(1)), max_ttl(&s.e, &s.id));
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
    s.distribute(1, &[(&s.alice, USDC)]);
    advance(&s.e, 40);
    let instance = instance_ttl(&s.e, &s.id);
    let marker = marker_ttl(&s.e, &s.id, &s.dist(1));
    s.client.is_paid(&s.dist(1));
    s.client.is_paid(&s.dist(2));
    s.client.powers();
    s.client.paused();
    s.client.schema_version();
    assert_eq!(instance_ttl(&s.e, &s.id), instance);
    assert_eq!(marker_ttl(&s.e, &s.id, &s.dist(1)), marker);
}
