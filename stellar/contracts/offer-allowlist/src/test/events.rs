//! Each entrypoint emits exactly the events the allowlist document lists,
//! in the documented order, OpenZeppelin's included.
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    vec, xdr, Address, Env, Event as _, Symbol, TryFromVal,
};
use stellar_access::access_control::{RoleAdminChanged, RoleGranted, RoleRevoked};

use super::{allocator, at, entry, period, sale_role, setup};
use crate::events::{AllocationConsumed, AllocationRestored, AllocationSet, Initialized};
use crate::OfferAllowlist;

extern crate std;
use std::vec::Vec;

/// The events the allowlist emitted in the last invocation.
fn emitted(e: &Env, id: &Address) -> Vec<xdr::ContractEvent> {
    e.events().all().filter_by_contract(id).events().to_vec()
}

/// The first topic of each event the allowlist emitted, i.e. the names.
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
fn construction_emits_the_role_setup_then_initialized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let hot_key = Address::generate(&e);
    let other = Address::generate(&e);
    let allocators = vec![&e, hot_key.clone(), other.clone()];
    let id = e.register(OfferAllowlist, (admin.clone(), allocators.clone()));

    assert_eq!(
        names(&e, &id),
        syms(
            &e,
            &[
                "role_admin_changed",
                "role_granted",
                "role_granted",
                "initialized"
            ]
        )
    );
    let evs = emitted(&e, &id);
    // The previous role-admin is the empty symbol: the role had none.
    let reparented = RoleAdminChanged {
        role: sale_role(&e),
        previous_admin_role: Symbol::new(&e, ""),
        new_admin_role: allocator(&e),
    };
    assert_eq!(evs[0], reparented.to_xdr(&e, &id));
    for (i, account) in [&hot_key, &other].into_iter().enumerate() {
        let granted = RoleGranted {
            role: allocator(&e),
            account: account.clone(),
            caller: admin.clone(),
        };
        assert_eq!(evs[1 + i], granted.to_xdr(&e, &id));
    }
    let init = Initialized {
        admin: admin.clone(),
        allocators,
    };
    assert_eq!(evs[3], init.to_xdr(&e, &id));
}

#[test]
fn construction_with_no_allocators_still_reparents_sale() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let none: soroban_sdk::Vec<Address> = vec![&e];
    let id = e.register(OfferAllowlist, (admin.clone(), none));
    assert_eq!(
        names(&e, &id),
        syms(&e, &["role_admin_changed", "initialized"])
    );
}

#[test]
fn set_allocations_emits_one_allocation_set_per_entry() {
    let s = setup();
    s.client.set_allocations(
        &vec![&s.e, entry(&s.alice, 100, true), entry(&s.bob, 0, false)],
        &s.hot_key,
    );
    let alice = AllocationSet {
        investor: s.alice.clone(),
        caller: s.hot_key.clone(),
        enabled: true,
        max: 100,
    };
    let bob = AllocationSet {
        investor: s.bob.clone(),
        caller: s.hot_key.clone(),
        enabled: false,
        max: 0,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [alice.to_xdr(&s.e, &s.id), bob.to_xdr(&s.e, &s.id)]
    );
}

#[test]
fn consume_emits_allocation_consumed_with_the_usage_after_the_period_and_the_receipt() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.sale().buy(&s.id, &s.alice, &30);
    let receipt = s.sale().buy(&s.id, &s.alice, &25);
    let ev = AllocationConsumed {
        investor: s.alice.clone(),
        caller: s.sale.clone(),
        amount: 25,
        consumed: 55,
        period: period(2026, 10),
        receipt,
    };
    assert_eq!(receipt, 1);
    assert_eq!(emitted(&s.e, &s.id), [ev.to_xdr(&s.e, &s.id)]);
}

#[test]
fn a_restore_emits_one_event_per_receipt_in_list_order() {
    let s = setup();
    s.allow(&s.alice, 100);
    at(&s.e, 2026, 9, 20);
    let september = s.sale().buy(&s.id, &s.alice, &70);
    at(&s.e, 2026, 10, 15);
    let october = s.sale().buy(&s.id, &s.alice, &10);
    let more = s.sale().buy(&s.id, &s.alice, &5);
    s.sale()
        .refund(&s.id, &s.alice, &vec![&s.e, more, september, october]);
    // Each event carries its receipt, its period and that period's usage
    // after this receipt, so two receipts of one month show the usage fall
    // step by step.
    let restored = |amount, consumed, period, receipt| {
        AllocationRestored {
            investor: s.alice.clone(),
            caller: s.sale.clone(),
            amount,
            consumed,
            period,
            receipt,
        }
        .to_xdr(&s.e, &s.id)
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            restored(5, 10, period(2026, 10), more),
            restored(70, 0, period(2026, 9), september),
            restored(10, 0, period(2026, 10), october),
        ]
    );
}

#[test]
fn role_changes_emit_open_zeppelins_events() {
    let s = setup();
    let next_sale = s.e.register(super::SaleStub, ());
    s.client
        .grant_role(&next_sale, &sale_role(&s.e), &s.hot_key);
    let granted = RoleGranted {
        role: sale_role(&s.e),
        account: next_sale.clone(),
        caller: s.hot_key.clone(),
    };
    assert_eq!(emitted(&s.e, &s.id), [granted.to_xdr(&s.e, &s.id)]);

    s.client.revoke_role(&next_sale, &sale_role(&s.e), &s.admin);
    let revoked = RoleRevoked {
        role: sale_role(&s.e),
        account: next_sale.clone(),
        caller: s.admin.clone(),
    };
    assert_eq!(emitted(&s.e, &s.id), [revoked.to_xdr(&s.e, &s.id)]);

    let nobody = Symbol::new(&s.e, "nobody");
    s.client.set_role_admin(&sale_role(&s.e), &nobody);
    let reparented = RoleAdminChanged {
        role: sale_role(&s.e),
        previous_admin_role: allocator(&s.e),
        new_admin_role: nobody,
    };
    assert_eq!(emitted(&s.e, &s.id), [reparented.to_xdr(&s.e, &s.id)]);
}

#[test]
fn an_admin_transfer_emits_initiated_then_completed() {
    let s = setup();
    let next = Address::generate(&s.e);
    s.client.transfer_admin_role(&next, &1_000);
    assert_eq!(
        names(&s.e, &s.id),
        syms(&s.e, &["admin_transfer_initiated"])
    );
    s.client.accept_admin_transfer();
    assert_eq!(
        names(&s.e, &s.id),
        syms(&s.e, &["admin_transfer_completed"])
    );
}

/// The XDR size of each allowlist event, which the interface document's
/// batch arithmetic rests on. Asserted so the documents can't drift from
/// the code; `-- --nocapture` prints them.
#[test]
fn event_sizes_match_the_interface_document() {
    use soroban_sdk::xdr::{Limits, WriteXdr};
    let s = setup();
    let size = |ev: xdr::ContractEvent| ev.to_xdr(Limits::none()).unwrap().len();
    let set = AllocationSet {
        investor: s.alice.clone(),
        caller: s.hot_key.clone(),
        enabled: true,
        max: 1,
    };
    let consumed = AllocationConsumed {
        investor: s.alice.clone(),
        caller: s.hot_key.clone(),
        amount: 1,
        consumed: 1,
        period: 1,
        receipt: 1,
    };
    let restored = AllocationRestored {
        investor: s.alice.clone(),
        caller: s.hot_key.clone(),
        amount: 1,
        consumed: 1,
        period: 1,
        receipt: 1,
    };
    let sizes = (
        size(set.to_xdr(&s.e, &s.id)),
        size(consumed.to_xdr(&s.e, &s.id)),
        size(restored.to_xdr(&s.e, &s.id)),
    );
    std::println!(
        "allocation_set {} B, allocation_consumed {} B, allocation_restored {} B",
        sizes.0,
        sizes.1,
        sizes.2
    );
    assert_eq!(sizes, (240, 292, 292));
}
