//! A quiet contract must not archive between touches: every call tops the
//! instance and the entries it touches up to the network maximum once they
//! fall a month below it, the caller's own role entry included, and a grant
//! tops up the granted entry. Reads of the allowlist's own entries extend
//! nothing. An entry that did archive is restored by the call that touches
//! it (protocol 23), then topped up like any other.
use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Ledger as _,
    },
    Address, Env,
};
use stellar_access::access_control::AccessControlStorageKey;

use super::{allocator, at, period, sale_role, setup};
use crate::storage::DataKey;
use soroban_sdk::Symbol;

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

fn receipts_ttl(e: &Env, id: &Address, sale: &Address, who: &Address) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&DataKey::Receipts(sale.clone(), who.clone()))
    })
}

fn role_ttl(e: &Env, id: &Address, who: &Address, role: Symbol) -> u32 {
    e.as_contract(id, || {
        e.storage()
            .persistent()
            .get_ttl(&AccessControlStorageKey::HasRole(who.clone(), role))
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
fn consume_extends_the_allocation_the_usage_and_the_sales_role() {
    let s = setup();
    s.allow(&s.alice, 100);
    advance(&s.e, 40 * DAY);
    assert!(allocation_ttl(&s.e, &s.id, &s.alice) < max_ttl(&s.e, &s.id));
    assert!(role_ttl(&s.e, &s.id, &s.sale, sale_role(&s.e)) < max_ttl(&s.e, &s.id));
    s.consume(&s.alice, &1);
    let max = max_ttl(&s.e, &s.id);
    assert_eq!(allocation_ttl(&s.e, &s.id, &s.alice), max);
    assert_eq!(usage_ttl(&s.e, &s.id, &s.alice, period(2026, 10)), max);
    assert_eq!(role_ttl(&s.e, &s.id, &s.sale, sale_role(&s.e)), max);
}

#[test]
fn consume_gives_the_receipts_entry_the_maximum_ttl() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &1);
    let max = max_ttl(&s.e, &s.id);
    assert_eq!(receipts_ttl(&s.e, &s.id, &s.sale, &s.alice), max);
    advance(&s.e, 40 * DAY);
    assert!(receipts_ttl(&s.e, &s.id, &s.sale, &s.alice) < max);
    s.consume(&s.alice, &1);
    assert_eq!(receipts_ttl(&s.e, &s.id, &s.sale, &s.alice), max);
}

#[test]
fn set_allocations_extends_the_allocators_role() {
    let s = setup();
    advance(&s.e, 40 * DAY);
    assert!(role_ttl(&s.e, &s.id, &s.hot_key, allocator(&s.e)) < max_ttl(&s.e, &s.id));
    s.allow(&s.alice, 100);
    assert_eq!(
        role_ttl(&s.e, &s.id, &s.hot_key, allocator(&s.e)),
        max_ttl(&s.e, &s.id)
    );
}

#[test]
fn a_grant_tops_the_granted_entry_up_to_the_maximum() {
    let s = setup();
    // At construction and at every grant, not OpenZeppelin's default life.
    assert_eq!(
        role_ttl(&s.e, &s.id, &s.hot_key, allocator(&s.e)),
        max_ttl(&s.e, &s.id)
    );
    advance(&s.e, 40 * DAY);
    let next_sale = s.e.register(super::SaleStub, ());
    s.client
        .grant_role(&next_sale, &sale_role(&s.e), &s.hot_key);
    assert_eq!(
        role_ttl(&s.e, &s.id, &next_sale, sale_role(&s.e)),
        max_ttl(&s.e, &s.id)
    );
}

#[test]
fn the_sales_role_outlives_open_zeppelins_ninety_days() {
    let s = setup();
    s.allow(&s.alice, 100);
    let n = s.consume(&s.alice, &1);
    // A refund after a long silence still finds the role alive, and renews it.
    advance(&s.e, 200 * DAY);
    assert!(role_ttl(&s.e, &s.id, &s.sale, sale_role(&s.e)) > 90 * DAY);
    s.restore(&s.alice, &[n]);
    assert_eq!(
        role_ttl(&s.e, &s.id, &s.sale, sale_role(&s.e)),
        max_ttl(&s.e, &s.id)
    );
}

#[test]
fn restore_extends_the_entries_it_writes() {
    let s = setup();
    s.allow(&s.alice, 100);
    let n = s.consume(&s.alice, &1);
    s.consume(&s.alice, &9);
    advance(&s.e, 40 * DAY);
    s.restore(&s.alice, &[n]);
    let p = period(2026, 10);
    let max = max_ttl(&s.e, &s.id);
    assert_eq!(usage_ttl(&s.e, &s.id, &s.alice, p), max);
    assert_eq!(receipts_ttl(&s.e, &s.id, &s.sale, &s.alice), max);
}

#[test]
fn reads_extend_nothing() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &10);
    advance(&s.e, 40 * DAY);
    let allocation = allocation_ttl(&s.e, &s.id, &s.alice);
    let usage = usage_ttl(&s.e, &s.id, &s.alice, period(2026, 10));
    let instance = instance_ttl(&s.e, &s.id);
    let receipts = receipts_ttl(&s.e, &s.id, &s.sale, &s.alice);
    s.client.allocation(&s.alice);
    s.client.remaining(&s.alice);
    s.client.can_invest(&s.alice, &1);
    s.client.usage(&s.alice, &period(2026, 10));
    s.client.receipt(&s.sale, &s.alice, &0);
    s.client.receipts(&s.sale, &s.alice);
    assert_eq!(allocation_ttl(&s.e, &s.id, &s.alice), allocation);
    assert_eq!(usage_ttl(&s.e, &s.id, &s.alice, period(2026, 10)), usage);
    assert_eq!(instance_ttl(&s.e, &s.id), instance);
    assert_eq!(receipts_ttl(&s.e, &s.id, &s.sale, &s.alice), receipts);
}

#[test]
fn an_untouched_old_month_is_left_to_expire() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.consume(&s.alice, &10);
    advance(&s.e, 40 * DAY);
    at(&s.e, 2026, 11, 24);
    s.consume(&s.alice, &10);
    // November's write touched nothing of October's usage. The receipts
    // entry is shared by every purchase of the investor, so it was topped
    // up; an investor who stops buying leaves it to expire.
    let max = max_ttl(&s.e, &s.id);
    assert_eq!(
        usage_ttl(&s.e, &s.id, &s.alice, period(2026, 10)),
        max - 40 * DAY
    );
    assert_eq!(receipts_ttl(&s.e, &s.id, &s.sale, &s.alice), max);
}

/// An archived usage or receipts entry is not gone. Since protocol 23
/// (CAP-0066) the call that touches it restores it: simulation lists the
/// key in the read-write footprint and the transaction pays the
/// restoration. The test environment runs in that recording mode, so a
/// late refund of an archived receipt and month passes, and the allowlist
/// then tops both entries up to the maximum.
#[test]
fn a_late_refund_of_an_archived_receipt_restores_and_tops_up_both_entries() {
    let s = setup();
    s.allow(&s.alice, 100);
    let late = s.consume(&s.alice, &4);
    s.consume(&s.alice, &6);
    let october = period(2026, 10);
    let max = max_ttl(&s.e, &s.id);
    // A later write keeps the instance, the allocation and the hot key's
    // role alive without touching October's usage.
    advance(&s.e, 40 * DAY);
    s.allow(&s.alice, 100);
    // Past October's lifetime, before the instance's.
    advance(&s.e, max - 40 * DAY + 1);
    assert!(instance_ttl(&s.e, &s.id) > 0);
    s.restore(&s.alice, &[late]);
    assert_eq!(s.client.usage(&s.alice, &october), 6);
    assert_eq!(s.receipt(&s.alice, late), None);
    assert_eq!(receipts_ttl(&s.e, &s.id, &s.sale, &s.alice), max);
    assert_eq!(
        usage_ttl(&s.e, &s.id, &s.alice, october),
        max_ttl(&s.e, &s.id)
    );
}
