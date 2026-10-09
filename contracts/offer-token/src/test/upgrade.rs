//! The code swap keeps every piece of state. The target is the v2 fixture
//! crate, a different binary that reads the v1 layout, built by
//! `scripts/test.sh` before the tests run.

#[cfg(fixture_missing)]
#[test]
fn upgrade_fixture_is_built() {
    panic!(
        "the upgrade fixture is not built: run scripts/test.sh, which builds \
         offer-token-v2-fixture with the Stellar CLI and then runs cargo test"
    );
}

#[cfg(not(fixture_missing))]
mod with_fixture {
    use soroban_sdk::{testutils::Events as _, Event as _, String};

    use super::super::{err, setup, SUPPLY};
    use crate::events::Upgraded;
    use crate::OfferTokenError;

    mod v2 {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/offer_token_v2_fixture.wasm"
        );
    }

    #[test]
    fn storage_survives_the_code_swap() {
        let s = setup();
        let uri = String::from_str(&s.e, "ipfs://before-upgrade");
        s.client.set_offer_uri(&uri, &s.admin);
        s.client.set_transfer_whitelist(&s.alice, &true, &s.admin);
        s.client.pause(&s.pauser);

        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.client.upgrade(&hash, &s.upgrader);
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
        assert_eq!(v2.total_supply(), SUPPLY);
        assert_eq!(v2.balance(&s.alice), 100);
        assert_eq!(v2.balance(&s.sale), SUPPLY - 200);
        assert_eq!(v2.decimals(), 0);
        assert_eq!(v2.name(), String::from_str(&s.e, "Edificio Aurora"));
        assert_eq!(v2.offer_uri(), uri);
        assert!(v2.is_transfer_whitelisted(&s.alice));
        assert!(v2.is_transfer_whitelisted(&s.listed));
        assert!(!v2.is_transfer_whitelisted(&s.bob));
        assert_eq!(v2.admin(), s.admin);
        assert_eq!(v2.xfer_admin(), s.xfer_admin);
        assert_eq!(v2.pauser(), s.pauser);
        assert_eq!(v2.upgrader(), s.upgrader);
        assert!(v2.paused());
        assert_eq!(v2.schema_version(), 1);
    }

    #[test]
    fn a_non_upgrader_cannot_swap_the_code() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        let r = s.client.try_upgrade(&hash, &s.admin);
        assert_eq!(err(r), OfferTokenError::NotUpgrader.into());
        assert_eq!(s.client.schema_version(), 1);
    }
}
