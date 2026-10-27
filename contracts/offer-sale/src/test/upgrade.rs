//! The code swap keeps every piece of state. The target is the v2 fixture
//! crate, a different binary that reads the v1 layout, built by
//! `scripts/test.sh` before the tests run.

#[cfg(fixture_missing)]
#[test]
fn upgrade_fixture_is_built() {
    panic!(
        "the upgrade fixture is not built: run scripts/test.sh, which builds \
         offer-sale-upgrade-fixture with the Stellar CLI and then runs cargo test"
    );
}

#[cfg(not(fixture_missing))]
mod with_fixture {
    use soroban_sdk::{testutils::Events as _, BytesN, Event as _};

    use super::super::{err, setup, PRICE, SUPPLY};
    use crate::events::Upgraded;
    use crate::OfferSaleError;

    mod v2 {
        soroban_sdk::contractimport!(
            file = "../../target/fixtures/offer_sale_upgrade_fixture.wasm"
        );
    }

    #[test]
    fn storage_survives_the_code_swap() {
        let s = setup();
        let first = s.buy(&s.alice, 10);
        s.advance(100);
        let second = s.buy(&s.alice, 5);
        s.buy(&s.bob, super::super::SUCCESS_TOKENS);
        s.client.close(&s.owner);
        s.client.pause(&s.platform);

        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.client.upgrade(&hash, &s.platform);
        let ev = Upgraded {
            new_wasm_hash: hash.clone(),
            from_schema_version: 1,
        };
        assert_eq!(
            s.e.events().all().filter_by_contract(&s.id).events(),
            [ev.to_xdr(&s.e, &s.id)]
        );

        let v2 = v2::Client::new(&s.e, &s.id);
        assert_eq!(v2.v2_marker(), 2);
        assert_eq!(v2.state(), v2::SaleState::Closing);
        let accounting = v2.accounting();
        assert_eq!(accounting.supply, SUPPLY);
        let sold = 15 + super::super::SUCCESS_TOKENS;
        assert_eq!(
            (accounting.sold_tokens, accounting.raised),
            (sold, sold * PRICE)
        );
        let config = v2.config();
        assert_eq!(config.price, PRICE);
        assert_eq!(config.min_success_percent, super::super::MIN_PERCENT);
        let powers = v2.powers();
        assert_eq!(powers.owner, s.owner);
        assert_eq!(powers.upgrader, s.platform);
        assert_eq!(v2.contracts().allowlist, s.allowlist.address);
        assert_eq!(v2.last_purchase_at(), super::super::T0 + 100);
        let reservation = v2.reservation(&s.alice).unwrap();
        assert_eq!(
            (reservation.schema, reservation.tokens, reservation.paid),
            (1, 15, 15 * PRICE)
        );
        assert_eq!(reservation.purchases.get(first).unwrap().tokens, 10);
        assert_eq!(reservation.purchases.get(second).unwrap().tokens, 5);
        assert_eq!(
            v2.reservation(&s.bob).unwrap().tokens,
            super::super::SUCCESS_TOKENS
        );
        assert!(v2.paused());
        assert_eq!(v2.schema_version(), 1);
    }

    #[test]
    fn the_upgraded_code_can_still_upgrade() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.client.upgrade(&hash, &s.platform);
        let v2 = v2::Client::new(&s.e, &s.id);
        assert_eq!(
            err(v2.try_upgrade(&hash, &s.owner)),
            OfferSaleError::NotUpgrader.into()
        );
        v2.upgrade(&hash, &s.platform);
        assert_eq!(v2.v2_marker(), 2);
    }

    #[test]
    #[should_panic(expected = "Error(Storage, MissingValue)")]
    fn upgrading_to_a_hash_that_was_never_uploaded_fails() {
        let s = setup();
        s.client
            .upgrade(&BytesN::from_array(&s.e, &[7; 32]), &s.platform);
    }

    #[test]
    fn a_non_upgrader_cannot_swap_the_code() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        assert_eq!(
            err(s.client.try_upgrade(&hash, &s.owner)),
            OfferSaleError::NotUpgrader.into()
        );
        assert_eq!(s.client.schema_version(), 1);
    }
}
