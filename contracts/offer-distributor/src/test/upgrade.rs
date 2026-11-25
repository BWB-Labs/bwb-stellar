//! The code swap keeps every piece of state. The target is the v2 fixture
//! crate, a different binary that reads the v1 layout, built by
//! `scripts/test.sh` before the tests run.

#[cfg(fixture_missing)]
#[test]
fn upgrade_fixture_is_built() {
    panic!(
        "the upgrade fixture is not built: run scripts/test.sh, which builds \
         offer-distributor-upgrade-fixture with the Stellar CLI and then runs cargo test"
    );
}

#[cfg(not(fixture_missing))]
mod with_fixture {
    use soroban_sdk::{
        testutils::{storage::Instance as _, Events as _, Ledger as _},
        BytesN, Event as _,
    };

    use super::super::{err, setup, USDC};
    use crate::events::Upgraded;
    use crate::OfferDistributorError;

    mod v2 {
        soroban_sdk::contractimport!(
            file = "../../target/fixtures/offer_distributor_upgrade_fixture.wasm"
        );
    }

    #[test]
    fn storage_survives_the_code_swap() {
        let s = setup();
        s.distribute(1, &[(&s.alice, USDC)]);
        s.distribute(2, &[(&s.bob, USDC)]);
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
        let powers = v2.powers();
        assert_eq!(powers.owner, s.owner);
        assert_eq!(powers.pauser, s.pauser);
        assert_eq!(powers.upgrader, s.upgrader);
        assert!(v2.is_paid(&s.dist(1)));
        assert!(v2.is_paid(&s.dist(2)));
        assert!(!v2.is_paid(&s.dist(3)));
        assert!(v2.paused());
        assert_eq!(v2.schema_version(), 1);
    }

    #[test]
    fn the_upgraded_code_can_still_upgrade() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.client.upgrade(&hash, &s.upgrader);
        let v2 = v2::Client::new(&s.e, &s.id);
        assert_eq!(
            err(v2.try_upgrade(&hash, &s.owner)),
            OfferDistributorError::NotUpgrader.into()
        );
        v2.upgrade(&hash, &s.upgrader);
        assert_eq!(v2.v2_marker(), 2);
    }

    #[test]
    #[should_panic(expected = "Error(Storage, MissingValue)")]
    fn upgrading_to_a_hash_that_was_never_uploaded_fails() {
        let s = setup();
        s.client
            .upgrade(&BytesN::from_array(&s.e, &[7; 32]), &s.upgrader);
    }

    /// The owner and the pauser both hold a power here and neither may
    /// swap the code; the old code keeps running, so the new binary's
    /// function is not there to call.
    #[test]
    fn a_non_upgrader_cannot_swap_the_code() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        for caller in [&s.owner, &s.pauser] {
            assert_eq!(
                err(s.client.try_upgrade(&hash, caller)),
                OfferDistributorError::NotUpgrader.into()
            );
        }
        assert!(v2::Client::new(&s.e, &s.id).try_v2_marker().is_err());
    }

    /// An upgrade is a state-changing call like the others: it tops the
    /// instance up after a quiet month.
    #[test]
    fn an_upgrade_tops_the_instance_up() {
        let s = setup();
        let hash = s.e.deployer().upload_contract_wasm(v2::WASM);
        s.e.ledger().with_mut(|l| l.sequence_number += 40 * 17_280);
        let ttl = || {
            s.e.as_contract(&s.id, || s.e.storage().instance().get_ttl())
        };
        let max = s.e.as_contract(&s.id, || s.e.storage().max_ttl());
        assert!(ttl() < max);
        s.client.upgrade(&hash, &s.upgrader);
        assert_eq!(ttl(), max);
    }
}
