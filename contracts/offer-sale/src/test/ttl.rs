//! A quiet contract must not archive between touches: every call tops the
//! instance and the entries it touches up to the network maximum once they
//! fall a month below it. Reads extend nothing.
use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Ledger as _,
    },
    Address, Env,
};

use super::setup;
use crate::storage::DataKey;

const DAY: u32 = 17_280;

fn instance_ttl(e: &Env, id: &Address) -> u32 {
    e.as_contract(id, || e.storage().instance().get_ttl())
}

fn reservation_ttl(e: &Env, id: &Address, who: &Address) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&DataKey::Reservation(who.clone()))
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
fn a_purchase_after_a_quiet_month_tops_the_instance_up_again() {
    let s = setup();
    advance(&s.e, 40);
    assert!(instance_ttl(&s.e, &s.id) < max_ttl(&s.e, &s.id));
    s.buy(&s.alice, 1);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn a_purchase_writes_the_reservation_with_the_maximum_ttl() {
    let s = setup();
    advance(&s.e, 40);
    s.buy(&s.alice, 1);
    assert_eq!(reservation_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
}

#[test]
fn a_second_purchase_and_a_partial_refund_top_the_reservation_up() {
    let s = setup();
    s.buy(&s.alice, 1);
    advance(&s.e, 40);
    assert!(reservation_ttl(&s.e, &s.id, &s.alice) < max_ttl(&s.e, &s.id));
    s.buy(&s.alice, 1);
    assert_eq!(reservation_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
    s.e.ledger().with_mut(|l| l.timestamp += 6 * 86_400);
    s.buy(&s.alice, 1);
    advance(&s.e, 40);
    s.client.cooling_off_refund(&s.alice);
    assert_eq!(reservation_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn the_owners_calls_and_finalize_top_the_instance_up() {
    let s = setup();
    s.buy(&s.alice, 750);
    advance(&s.e, 40);
    s.client.close(&s.owner);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
    s.past_cooling_off();
    advance(&s.e, 40);
    s.client.finalize();
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
    advance(&s.e, 40);
    s.client
        .withdraw_payment(&s.payouts(&[(&s.owner, 1)]), &s.owner);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn reads_extend_nothing() {
    let s = setup();
    s.buy(&s.alice, 1);
    advance(&s.e, 40);
    let instance = instance_ttl(&s.e, &s.id);
    let reservation = reservation_ttl(&s.e, &s.id, &s.alice);
    s.client.reservation(&s.alice);
    s.client.accounting();
    s.client.state();
    s.client.finalizable_at();
    assert_eq!(instance_ttl(&s.e, &s.id), instance);
    assert_eq!(reservation_ttl(&s.e, &s.id, &s.alice), reservation);
}
