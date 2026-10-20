//! The code swap keeps every piece of state. The target is the v2 fixture
//! crate, a different binary that reads the v1 layout, built by
//! `scripts/test.sh` before the tests run.

#[cfg(fixture_missing)]
#[test]
fn upgrade_fixture_is_built() {
    panic!(
        "the upgrade fixture is not built: run scripts/test.sh, which builds \
         offer-allowlist-upgrade-fixture with the Stellar CLI and then runs cargo test"
    );
}

#[cfg(not(fixture_missing))]
mod with_fixture {
    use soroban_sdk::{testutils::Events as _, vec, Event as _};

    use super::super::{allocator, at, code, err, period, sale_role, setup, UNAUTHORIZED};
    use crate::events::Upgraded;
    use crate::OfferAllowlistError;

    mod v2 {
        soroban_sdk::contractimport!(
            file = "../../target/fixtures/offer_allowlist_upgrade_fixture.wasm"
        );
    }

    #[test]
    fn storage_survives_the_code_swap() {
        let s = setup();
        s.allow(&s.alice, 100);
        at(&s.e, 2026, 9, 1);
        s.sale().buy(&s.id, &s.alice, &40);
        at(&s.e, 2026, 10, 15);
        let october = s.sale().buy(&s.id, &s.alice, &25);
        s.client.set_allocations(
            &vec![&s.e, super::super::entry(&s.bob, 7, false)],
            &s.hot_key,
        );

        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.client.upgrade(&hash, &s.admin);
        let ev = Upgraded {
            from_schema_version: 1,
            new_wasm_hash: hash.clone(),
        };
        assert_eq!(
            s.e.events().all().filter_by_contract(&s.id).events(),
            [ev.to_xdr(&s.e, &s.id)]
        );

        let v2 = v2::Client::new(&s.e, &s.id);
        assert_eq!(v2.v2_marker(), 2);
        let alice = v2.allocation(&s.alice);
        assert_eq!((alice.schema, alice.max, alice.enabled), (1, 100, true));
        let bob = v2.allocation(&s.bob);
        assert_eq!((bob.schema, bob.max, bob.enabled), (1, 7, false));
        assert_eq!(v2.usage(&s.alice, &period(2026, 9)), 40);
        assert_eq!(v2.usage(&s.alice, &period(2026, 10)), 25);
        assert_eq!(v2.usage(&s.bob, &period(2026, 10)), 0);
        let receipts = v2.receipt_book(&s.sale, &s.alice);
        assert_eq!(receipts.next, 2);
        let receipt = receipts.receipts.get(october).unwrap();
        assert_eq!((receipt.amount, receipt.period), (25, period(2026, 10)));
        assert!(v2.has_role(&s.hot_key, &allocator(&s.e)).is_some());
        assert!(v2.has_role(&s.sale, &sale_role(&s.e)).is_some());
        assert_eq!(v2.get_role_member_count(&allocator(&s.e)), 1);
        assert_eq!(v2.get_role_member_count(&sale_role(&s.e)), 1);
        assert_eq!(v2.get_role_admin(&sale_role(&s.e)), Some(allocator(&s.e)));
        assert_eq!(v2.get_role_admin(&allocator(&s.e)), None);
        assert_eq!(v2.get_admin(), Some(s.admin.clone()));
        assert_eq!(v2.schema_version(), 1);
    }

    #[test]
    fn the_upgraded_code_can_still_upgrade() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.client.upgrade(&hash, &s.admin);
        let v2 = v2::Client::new(&s.e, &s.id);
        // Same binary again: proves the gate and the swap survive the swap.
        assert_eq!(err(v2.try_upgrade(&hash, &s.hot_key)), code(UNAUTHORIZED));
        v2.upgrade(&hash, &s.admin);
        assert_eq!(v2.v2_marker(), 2);
    }

    /// A version that let the admin renounce would freeze upgrades for
    /// good; the fixture keeps v1's override, and so must every release.
    #[test]
    fn the_upgraded_code_still_blocks_renounce_admin() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.client.upgrade(&hash, &s.admin);
        let v2 = v2::Client::new(&s.e, &s.id);
        assert_eq!(
            err(v2.try_renounce_admin()),
            OfferAllowlistError::RenounceBlocked.into()
        );
        assert_eq!(v2.get_admin(), Some(s.admin.clone()));
        v2.upgrade(&hash, &s.admin);
    }

    #[test]
    #[should_panic(expected = "Error(Storage, MissingValue)")]
    fn upgrading_to_a_hash_that_was_never_uploaded_fails() {
        let s = setup();
        let unknown = soroban_sdk::BytesN::from_array(&s.e, &[7; 32]);
        s.client.upgrade(&unknown, &s.admin);
    }

    #[test]
    fn a_non_admin_cannot_swap_the_code() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        for operator in [&s.hot_key, &s.alice] {
            assert_eq!(
                err(s.client.try_upgrade(&hash, operator)),
                code(UNAUTHORIZED)
            );
        }
        assert_eq!(s.client.schema_version(), 1);
        // Still the v1 code: restore exists and works.
        s.allow(&s.alice, 10);
        let receipt = s.sale().buy(&s.id, &s.alice, &10);
        s.sale().refund(&s.id, &s.alice, &vec![&s.e, receipt]);
    }
}
