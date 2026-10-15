//! A quiet contract must not archive between touches: every call tops the
//! instance and the entries it touches up to the network maximum once they
//! fall a month below it, the caller's own role entry included. Reads
//! extend nothing. An entry that did archive must be restored before the
//! call that touches it.
use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Ledger as _,
    },
    vec, Address, Env,
};
use stellar_access::access_control::AccessControlStorageKey;

use super::{at, controller, give_back, period, setup};
use crate::storage::DataKey;

const DAY: u32 = 17_280;

fn instance_ttl(e: &Env, id: &Address) -> u32 {
    e.as_contract(id, || e.storage().instance().get_ttl())
}

fn allocation_ttl(e: &Env, id: &Address, who: &Address) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&DataKey::Allocation(who.clone()))
    })
}

fn usage_ttl(e: &Env, id: &Address, who: &Address, period: u32) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&DataKey::Usage(who.clone(), period))
    })
}

fn role_ttl(e: &Env, id: &Address, who: &Address) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&AccessControlStorageKey::HasRole(
                who.clone(),
                controller(e),
            ))
    })
}

fn max_ttl(e: &Env, id: &Address) -> u32 {
    e.as_contract(id, || e.storage().max_ttl())
}

fn advance(e: &Env, ledgers: u32) {
    e.ledger().with_mut(|l| l.sequence_number += ledgers);
}

#[test]
fn construction_extends_the_instance_to_the_maximum() {
    let s = setup();
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn a_write_after_a_quiet_month_tops_the_instance_up_again() {
    let s = setup();
    advance(&s.e, 40 * DAY);
    assert!(instance_ttl(&s.e, &s.id) < max_ttl(&s.e, &s.id));
    s.allow(&s.alice, 100);
    assert_eq!(instance_ttl(&s.e, &s.id), max_ttl(&s.e, &s.id));
}

#[test]
fn a_write_within_the_month_does_not_pay_rent_again() {
    let s = setup();
    let before = instance_ttl(&s.e, &s.id);
    advance(&s.e, 10 * DAY);
    s.allow(&s.alice, 100);
    assert_eq!(instance_ttl(&s.e, &s.id), before - 10 * DAY);
}

#[test]
fn writing_an_allocation_gives_it_the_maximum_ttl() {
    let s = setup();
    advance(&s.e, 40 * DAY);
    s.allow(&s.alice, 100);
    assert_eq!(allocation_ttl(&s.e, &s.id, &s.alice), max_ttl(&s.e, &s.id));
}

#[test]
fn consume_extends_the_allocation_the_usage_and_the_callers_role() {
    let s = setup();
    s.allow(&s.alice, 100);
    advance(&s.e, 40 * DAY);
    assert!(allocation_ttl(&s.e, &s.id, &s.alice) < max_ttl(&s.e, &s.id));
    s.client.consume(&s.alice, &1, &s.hot_key);
    let max = max_ttl(&s.e, &s.id);
    assert_eq!(allocation_ttl(&s.e, &s.id, &s.alice), max);
    assert_eq!(usage_ttl(&s.e, &s.id, &s.alice, period(2026, 10)), max);
    assert_eq!(role_ttl(&s.e, &s.id, &s.hot_key), max);
}

#[test]
fn the_callers_role_outlives_open_zeppelins_ninety_days() {
    let s = setup();
    s.grant_sale();
    // Granted through OpenZeppelin alone, the entry has its default life.
    assert!(role_ttl(&s.e, &s.id, &s.sale) < 90 * DAY);
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &1, &s.sale);
    assert_eq!(role_ttl(&s.e, &s.id, &s.sale), max_ttl(&s.e, &s.id));
    // A refund after a long silence still finds the role alive, and renews it.
    advance(&s.e, 200 * DAY);
    s.client.restore(
        &s.alice,
        &vec![&s.e, give_back(period(2026, 10), 1)],
        &s.sale,
    );
    assert_eq!(role_ttl(&s.e, &s.id, &s.sale), max_ttl(&s.e, &s.id));
}

#[test]
fn restore_extends_the_usage_entries_it_touches() {
    let s = setup();
    s.allow(&s.alice, 100);
    let p = s.client.consume(&s.alice, &10, &s.hot_key);
    advance(&s.e, 40 * DAY);
    s.client
        .restore(&s.alice, &vec![&s.e, give_back(p, 1)], &s.hot_key);
    assert_eq!(usage_ttl(&s.e, &s.id, &s.alice, p), max_ttl(&s.e, &s.id));
}

#[test]
fn reads_extend_nothing() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.client.consume(&s.alice, &10, &s.hot_key);
    advance(&s.e, 40 * DAY);
    let allocation = allocation_ttl(&s.e, &s.id, &s.alice);
    let usage = usage_ttl(&s.e, &s.id, &s.alice, period(2026, 10));
    let instance = instance_ttl(&s.e, &s.id);
    s.client.allocation(&s.alice);
    s.client.remaining(&s.alice);
    s.client.can_invest(&s.alice, &1);
    s.client.usage(&s.alice, &period(2026, 10));
    assert_eq!(allocation_ttl(&s.e, &s.id, &s.alice), allocation);
    assert_eq!(usage_ttl(&s.e, &s.id, &s.alice, period(2026, 10)), usage);
    assert_eq!(instance_ttl(&s.e, &s.id), instance);
}

#[test]
fn an_untouched_old_month_is_left_to_expire() {
    let s = setup();
    s.allow(&s.alice, 100);
    let september = s.client.consume(&s.alice, &10, &s.hot_key);
    advance(&s.e, 40 * DAY);
    at(&s.e, 2026, 11, 24);
    s.client.consume(&s.alice, &10, &s.hot_key);
    // November's write touched nothing of October's.
    assert_eq!(
        usage_ttl(&s.e, &s.id, &s.alice, september),
        max_ttl(&s.e, &s.id) - 40 * DAY
    );
}

/// An archived usage entry is not gone. Since protocol 23 (CAP-0066) the
/// call that touches it restores it: simulation lists the key in the
/// read-write footprint and the transaction pays the restoration. The test
/// environment runs in that recording mode, so a late refund of an archived
/// month passes, and the allowlist then tops the entry up to the maximum.
#[test]
fn a_late_refund_of_an_archived_month_restores_the_entry_and_tops_it_up() {
    let s = setup();
    s.allow(&s.alice, 100);
    let october = s.client.consume(&s.alice, &10, &s.hot_key);
    let max = max_ttl(&s.e, &s.id);
    // A later write keeps the instance, the allocation and the hot key's
    // role alive without touching October's usage.
    advance(&s.e, 40 * DAY);
    s.allow(&s.alice, 100);
    // Past October's lifetime, before the instance's.
    advance(&s.e, max - 40 * DAY + 1);
    assert!(instance_ttl(&s.e, &s.id) > 0);
    s.client
        .restore(&s.alice, &vec![&s.e, give_back(october, 4)], &s.hot_key);
    assert_eq!(s.client.usage(&s.alice, &october), 6);
    assert_eq!(
        usage_ttl(&s.e, &s.id, &s.alice, october),
        max_ttl(&s.e, &s.id)
    );
}
