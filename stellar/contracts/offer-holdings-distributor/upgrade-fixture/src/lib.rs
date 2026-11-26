//! TEST FIXTURE, never deployed. A second binary for
//! `offer-holdings-distributor`'s upgrade test: it declares the v1 storage
//! layout by hand, exposes reads over it and adds `v2_marker`, so the test
//! can prove the new code runs on the old state. If v1's layout changes
//! without this file following, the upgrade test fails, which is the point.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, BytesN, Env,
};
use stellar_contract_utils::{pausable, upgradeable};

/// Must match `offer-holdings-distributor`'s `DataKey` variant by variant.
#[contracttype]
pub enum DataKey {
    Powers,
    Contracts,
    PerQuota,
    Credited(Address),
}

/// Must match `offer-holdings-distributor`'s type field by field.
#[contracttype]
pub struct PowerHolders {
    pub owner: Address,
    pub pauser: Address,
    pub upgrader: Address,
}

/// Must match `offer-holdings-distributor`'s type field by field.
#[contracttype]
pub struct Contracts {
    pub token: Address,
    pub payment_asset: Address,
}

/// Same code as v1's `NotUpgrader`.
#[contracterror]
#[repr(u32)]
pub enum FixtureError {
    NotUpgrader = 6402,
}

#[contract]
pub struct OfferHoldingsDistributorV2Fixture;

#[contractimpl]
impl OfferHoldingsDistributorV2Fixture {
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

    pub fn contracts(e: &Env) -> Contracts {
        e.storage().instance().get(&DataKey::Contracts).unwrap()
    }

    pub fn cumulative_per_quota(e: &Env) -> i128 {
        e.storage().instance().get(&DataKey::PerQuota).unwrap_or(0)
    }

    pub fn credited(e: &Env, holder: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&DataKey::Credited(holder))
            .unwrap_or(0)
    }

    pub fn paused(e: &Env) -> bool {
        pausable::paused(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}
