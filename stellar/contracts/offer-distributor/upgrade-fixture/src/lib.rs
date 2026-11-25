//! TEST FIXTURE, never deployed. A second binary for `offer-distributor`'s
//! upgrade test: it declares the v1 storage layout by hand, exposes reads
//! over it and adds `v2_marker`, so the test can prove the new code runs on
//! the old state. If v1's layout changes without this file following, the
//! upgrade test fails, which is the point.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, BytesN, Env,
};
use stellar_contract_utils::{pausable, upgradeable};

/// Must match `offer-distributor`'s `DataKey` variant by variant.
#[contracttype]
pub enum DataKey {
    Powers,
    Paid(BytesN<32>),
}

/// Must match `offer-distributor`'s type field by field.
#[contracttype]
pub struct PowerHolders {
    pub owner: Address,
    pub pauser: Address,
    pub upgrader: Address,
}

/// Same code as v1's `NotUpgrader`.
#[contracterror]
#[repr(u32)]
pub enum FixtureError {
    NotUpgrader = 6302,
}

#[contract]
pub struct OfferDistributorV2Fixture;

#[contractimpl]
impl OfferDistributorV2Fixture {
    pub fn v2_marker() -> u32 {
        2
    }

    /// The upgraded code must itself stay upgradeable, gated by the same
    /// stored `upgrader`, or one bad release would freeze the contract.
    pub fn upgrade(e: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        operator.require_auth();
        if operator != Self::powers(e).upgrader {
            panic_with_error!(e, FixtureError::NotUpgrader);
        }
        upgradeable::upgrade(e, &new_wasm_hash);
    }

    pub fn powers(e: &Env) -> PowerHolders {
        e.storage().instance().get(&DataKey::Powers).unwrap()
    }

    pub fn is_paid(e: &Env, id: BytesN<32>) -> bool {
        e.storage().persistent().has(&DataKey::Paid(id))
    }

    pub fn paused(e: &Env) -> bool {
        pausable::paused(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}
