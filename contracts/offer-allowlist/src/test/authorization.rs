//! Who may do what: the admin over every role, an allocator over
//! allocations and over `sale`, a sale over room only, the sale stub as a
//! direct caller with no authorization entry, and the admin alone over
//! upgrades. A caller without the role fails with OpenZeppelin's 2000; a
//! missing signature fails at authorization.
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    vec, Address, BytesN, IntoVal, Symbol,
};

use super::{
    allocator, code, entry, err, give_back, period, sale_role, setup, SaleStubClient, UNAUTHORIZED,
};
use crate::OfferAllowlistError;

#[test]
fn admin_grants_and_revokes_any_role() {
    let s = setup();
    let auditor = Symbol::new(&s.e, "auditor");
    s.client.grant_role(&s.alice, &auditor, &s.admin);
    assert_eq!(s.client.has_role(&s.alice, &auditor), Some(0));
    s.client.revoke_role(&s.alice, &auditor, &s.admin);
    assert_eq!(s.client.has_role(&s.alice, &auditor), None);

    s.client.grant_role(&s.sale, &sale_role(&s.e), &s.admin);
    assert!(s.client.has_role(&s.sale, &sale_role(&s.e)).is_some());
    s.client.revoke_role(&s.sale, &sale_role(&s.e), &s.admin);
    assert_eq!(s.client.has_role(&s.sale, &sale_role(&s.e)), None);

    let second = Address::generate(&s.e);
    s.client.grant_role(&second, &allocator(&s.e), &s.admin);
    assert_eq!(s.client.get_role_member_count(&allocator(&s.e)), 2);
    s.client.revoke_role(&s.hot_key, &allocator(&s.e), &s.admin);
    assert_eq!(s.client.has_role(&s.hot_key, &allocator(&s.e)), None);
    s.client
        .set_allocations(&vec![&s.e, entry(&s.alice, 1, true)], &second);
}

#[test]
fn an_allocator_grants_and_revokes_sale() {
    let s = setup();
    let next_sale = s.e.register(super::SaleStub, ());
    s.client
        .grant_role(&next_sale, &sale_role(&s.e), &s.hot_key);
    assert!(s.client.has_role(&next_sale, &sale_role(&s.e)).is_some());
    assert_eq!(s.client.get_role_member_count(&sale_role(&s.e)), 2);
    s.client
        .revoke_role(&next_sale, &sale_role(&s.e), &s.hot_key);
    assert_eq!(s.client.has_role(&next_sale, &sale_role(&s.e)), None);
}

#[test]
fn an_allocator_cannot_grant_allocator_or_any_other_role() {
    let s = setup();
    let other = Address::generate(&s.e);
    let r = s
        .client
        .try_grant_role(&other, &allocator(&s.e), &s.hot_key);
    assert_eq!(err(r), code(UNAUTHORIZED));
    let r = s
        .client
        .try_revoke_role(&s.hot_key, &allocator(&s.e), &s.hot_key);
    assert_eq!(err(r), code(UNAUTHORIZED));
    let auditor = Symbol::new(&s.e, "auditor");
    let r = s.client.try_grant_role(&other, &auditor, &s.hot_key);
    assert_eq!(err(r), code(UNAUTHORIZED));
    assert_eq!(s.client.get_role_admin(&allocator(&s.e)), None);
}

#[test]
fn an_allocator_cannot_consume_or_restore() {
    let s = setup();
    s.allow(&s.alice, 100);
    let r = s.client.try_consume(&s.alice, &1, &s.hot_key);
    assert_eq!(err(r), code(UNAUTHORIZED));
    let r = s.client.try_restore(
        &s.alice,
        &vec![&s.e, give_back(period(2026, 10), 1)],
        &s.hot_key,
    );
    assert_eq!(err(r), code(UNAUTHORIZED));
}

#[test]
fn a_sale_cannot_write_allocations_grant_or_revoke() {
    let s = setup();
    let r = s
        .client
        .try_set_allocations(&vec![&s.e, entry(&s.bob, 1, true)], &s.sale);
    assert_eq!(err(r), code(UNAUTHORIZED));
    let other = Address::generate(&s.e);
    for role in [sale_role(&s.e), allocator(&s.e)] {
        let r = s.client.try_grant_role(&other, &role, &s.sale);
        assert_eq!(err(r), code(UNAUTHORIZED));
    }
    let r = s
        .client
        .try_revoke_role(&s.hot_key, &allocator(&s.e), &s.sale);
    assert_eq!(err(r), code(UNAUTHORIZED));
    let r = s.client.try_revoke_role(&s.sale, &sale_role(&s.e), &s.sale);
    assert_eq!(err(r), code(UNAUTHORIZED));
}

#[test]
fn a_stranger_cannot_write_consume_or_restore() {
    let s = setup();
    s.allow(&s.alice, 100);
    for caller in [&s.admin, &s.alice] {
        let r = s
            .client
            .try_set_allocations(&vec![&s.e, entry(&s.bob, 1, true)], caller);
        assert_eq!(err(r), code(UNAUTHORIZED));
        let r = s.client.try_consume(&s.alice, &1, caller);
        assert_eq!(err(r), code(UNAUTHORIZED));
        let r = s.client.try_restore(
            &s.alice,
            &vec![&s.e, give_back(period(2026, 10), 1)],
            caller,
        );
        assert_eq!(err(r), code(UNAUTHORIZED));
    }
}

/// A missing signature fails in the host's authorization check. A direct
/// call surfaces it as `Error(Auth, InvalidAction)`; a `try_` call rewraps
/// non-contract errors, so these tests use the direct call.
macro_rules! needs_signature {
    ($name:ident, |$s:ident| $call:expr) => {
        #[test]
        #[should_panic(expected = "Error(Auth, InvalidAction)")]
        fn $name() {
            let $s = setup();
            $s.allow(&$s.alice, 100);
            $s.e.set_auths(&[]);
            $call;
        }
    };
}

needs_signature!(set_allocations_needs_the_allocators_signature, |s| s
    .client
    .set_allocations(&vec![&s.e, entry(&s.bob, 1, true)], &s.hot_key));
needs_signature!(consume_needs_the_sales_authorization, |s| s
    .client
    .consume(&s.alice, &1, &s.sale));
needs_signature!(restore_needs_the_sales_authorization, |s| s.client.restore(
    &s.alice,
    &vec![&s.e, give_back(period(2026, 10), 1)],
    &s.sale
));
needs_signature!(grant_role_needs_the_callers_signature, |s| s
    .client
    .grant_role(&s.alice, &sale_role(&s.e), &s.hot_key));
needs_signature!(set_role_admin_needs_the_admins_signature, |s| s
    .client
    .set_role_admin(&sale_role(&s.e), &Symbol::new(&s.e, "nobody")));
needs_signature!(upgrade_needs_the_admins_signature, |s| s
    .client
    .upgrade(&BytesN::from_array(&s.e, &[0; 32]), &s.admin));

#[test]
fn the_allocators_signature_alone_writes_allocations() {
    let s = setup();
    let entries = vec![&s.e, entry(&s.alice, 100, true)];
    let args = (entries.clone(), &s.hot_key).into_val(&s.e);
    s.e.mock_auths(&[MockAuth {
        address: &s.hot_key,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "set_allocations",
            args,
            sub_invokes: &[],
        },
    }]);
    s.client.set_allocations(&entries, &s.hot_key);
    assert_eq!(s.client.allocation(&s.alice).max, 100);
}

#[test]
fn the_sale_consumes_and_restores_as_the_direct_caller_with_no_auth_entry() {
    let s = setup();
    s.allow(&s.alice, 100);
    // No mocks at all from here: the only authorization is the stub being
    // the contract that invokes the allowlist.
    s.e.set_auths(&[]);
    let sale = s.sale();
    assert_eq!(sale.buy(&s.id, &s.alice, &60), period(2026, 10));
    assert_eq!(s.client.remaining(&s.alice), 40);
    sale.refund(
        &s.id,
        &s.alice,
        &vec![&s.e, give_back(period(2026, 10), 60)],
    );
    assert_eq!(s.client.remaining(&s.alice), 100);
}

#[test]
fn a_sale_without_the_role_cannot_consume() {
    let s = setup();
    s.allow(&s.alice, 100);
    let unwired = s.e.register(super::SaleStub, ());
    s.e.set_auths(&[]);
    let sale = SaleStubClient::new(&s.e, &unwired);
    assert_eq!(err(sale.try_buy(&s.id, &s.alice, &1)), code(UNAUTHORIZED));
}

#[test]
fn nobody_can_consume_in_a_sales_name_from_outside() {
    let s = setup();
    s.allow(&s.alice, 100);
    s.e.set_auths(&[]);
    // The investor, or any account, naming the sale as caller: the sale's
    // authorization is only satisfied when the sale itself is the caller.
    assert!(s.client.try_consume(&s.alice, &1, &s.sale).is_err());
}

#[test]
fn only_the_admin_upgrades() {
    let s = setup();
    let hash = BytesN::from_array(&s.e, &[0; 32]);
    for operator in [&s.hot_key, &s.sale, &s.alice] {
        let r = s.client.try_upgrade(&hash, operator);
        assert_eq!(err(r), code(UNAUTHORIZED));
    }
}

#[test]
fn renounce_admin_is_blocked() {
    let s = setup();
    assert_eq!(
        err(s.client.try_renounce_admin()),
        OfferAllowlistError::RenounceBlocked.into()
    );
    assert_eq!(s.client.get_admin(), Some(s.admin.clone()));
}

#[test]
fn the_admin_moves_in_two_steps_and_the_new_admin_is_still_blocked() {
    let s = setup();
    let next = Address::generate(&s.e);
    s.client.transfer_admin_role(&next, &1_000);
    assert_eq!(s.client.get_admin(), Some(s.admin.clone()));
    s.client.accept_admin_transfer();
    assert_eq!(s.client.get_admin(), Some(next.clone()));
    assert_eq!(
        err(s.client.try_renounce_admin()),
        OfferAllowlistError::RenounceBlocked.into()
    );
    // The old admin lost the upgrade power with the role.
    let hash = BytesN::from_array(&s.e, &[0; 32]);
    assert_eq!(
        err(s.client.try_upgrade(&hash, &s.admin)),
        code(UNAUTHORIZED)
    );
}

/// The runbook in the allowlist document, step by step, against what a
/// leaked hot key can do before the multisig acts: write any allocation,
/// revoke a sale, grant `sale` to an accomplice.
#[test]
fn the_admin_contains_a_leaked_hot_key() {
    let s = setup();
    s.allow(&s.alice, 100);
    let p = s.sale().buy(&s.id, &s.alice, &100);

    // What the leaked key does before anyone notices.
    let accomplice = s.e.register(super::SaleStub, ());
    s.client
        .grant_role(&accomplice, &sale_role(&s.e), &s.hot_key);
    s.client.revoke_role(&s.sale, &sale_role(&s.e), &s.hot_key);
    let mallory = Address::generate(&s.e);
    s.client
        .set_allocations(&vec![&s.e, entry(&mallory, i128::MAX, true)], &s.hot_key);
    // The legitimate sale's refunds fail until it is re-granted.
    let r = s
        .sale()
        .try_refund(&s.id, &s.alice, &vec![&s.e, give_back(p, 100)]);
    assert_eq!(err(r), code(UNAUTHORIZED));

    // Optional first: freeze sale grants while cleaning up.
    s.client
        .set_role_admin(&sale_role(&s.e), &Symbol::new(&s.e, "nobody"));
    let r = s
        .client
        .try_grant_role(&s.sale, &sale_role(&s.e), &s.hot_key);
    assert_eq!(err(r), code(UNAUTHORIZED));

    // Step 1: revoke the leaked allocator, grant the replacement.
    s.client.revoke_role(&s.hot_key, &allocator(&s.e), &s.admin);
    let fresh = Address::generate(&s.e);
    s.client.grant_role(&fresh, &allocator(&s.e), &s.admin);
    let r = s
        .client
        .try_set_allocations(&vec![&s.e, entry(&s.bob, 1, true)], &s.hot_key);
    assert_eq!(err(r), code(UNAUTHORIZED));
    s.client
        .set_allocations(&vec![&s.e, entry(&mallory, 0, false)], &fresh);

    // Step 2: enumerate `sale`, revoke the unknown member, re-grant the
    // legitimate sale the leaked key revoked.
    let n = s.client.get_role_member_count(&sale_role(&s.e));
    assert_eq!(n, 1);
    let member = s.client.get_role_member(&sale_role(&s.e), &0);
    assert_eq!(member, accomplice);
    s.client.revoke_role(&member, &sale_role(&s.e), &s.admin);
    s.client.grant_role(&s.sale, &sale_role(&s.e), &s.admin);
    assert_eq!(s.client.get_role_member_count(&sale_role(&s.e)), 1);

    // Back to normal: the new allocator wires sales again.
    s.client.set_role_admin(&sale_role(&s.e), &allocator(&s.e));
    s.sale()
        .refund(&s.id, &s.alice, &vec![&s.e, give_back(p, 100)]);
    assert_eq!(s.client.remaining(&s.alice), 100);
    let another = Address::generate(&s.e);
    s.client.grant_role(&another, &sale_role(&s.e), &fresh);
}

#[test]
fn only_the_admin_reparents_a_role() {
    let s = setup();
    s.e.mock_auths(&[MockAuth {
        address: &s.hot_key,
        invoke: &MockAuthInvoke {
            contract: &s.id,
            fn_name: "set_role_admin",
            args: (sale_role(&s.e), Symbol::new(&s.e, "nobody")).into_val(&s.e),
            sub_invokes: &[],
        },
    }]);
    // set_role_admin takes no caller: it requires the stored admin's
    // signature, which the hot key's cannot stand in for.
    let r = s
        .client
        .try_set_role_admin(&sale_role(&s.e), &Symbol::new(&s.e, "nobody"));
    assert!(r.is_err());
    assert_eq!(
        s.client.get_role_admin(&sale_role(&s.e)),
        Some(allocator(&s.e))
    );
}
