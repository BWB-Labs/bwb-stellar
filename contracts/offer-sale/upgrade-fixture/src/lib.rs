//! TEST FIXTURE, never deployed. A second binary for `offer-sale`'s upgrade
//! test: it declares the v1 storage layout by hand, exposes reads over it and
//! adds `v2_marker`, so the test can prove the new code runs on the old
//! state. If v1's layout changes without this file following, the upgrade
//! test fails, which is the point.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, BytesN, Env,
    Map,
};
use stellar_contract_utils::{pausable, upgradeable};

/// Must match `offer-sale`'s `DataKey` variant by variant.
#[contracttype]
pub enum DataKey {
    Powers,
    Contracts,
    Config,
    State,
    Accounting,
    LastPurchaseAt,
    Reservation(Address),
}

/// Must match `offer-sale`'s types field by field.
#[contracttype]
pub struct PowerHolders {
    pub owner: Address,
    pub pauser: Address,
    pub upgrader: Address,
}

#[contracttype]
pub struct Contracts {
    pub token: Address,
    pub payment_asset: Address,
    pub allowlist: Address,
}

#[contracttype]
pub struct SaleConfig {
    pub price: i128,
    pub cooling_off_secs: u64,
    pub min_success_percent: u32,
}

#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaleState {
    Preparing,
    Active,
    Closing,
    Successful,
    Failed,
}

#[contracttype]
#[derive(Clone)]
pub struct Purchase {
    pub tokens: i128,
    pub paid: i128,
    pub at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct Reservation {
    pub schema: u32,
    pub tokens: i128,
    pub paid: i128,
    pub purchases: Map<u32, Purchase>,
}

#[contracttype]
#[derive(Clone)]
pub struct SaleAccounting {
    pub supply: i128,
    pub sold_tokens: i128,
    pub raised: i128,
    pub released_tokens: i128,
    pub withdrawn: i128,
}

/// Same code as v1's `NotUpgrader`.
#[contracterror]
#[repr(u32)]
pub enum FixtureError {
    NotUpgrader = 6224,
}

#[contract]
pub struct OfferSaleV2Fixture;

#[contractimpl]
impl OfferSaleV2Fixture {
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

    pub fn config(e: &Env) -> SaleConfig {
        e.storage().instance().get(&DataKey::Config).unwrap()
    }

    pub fn state(e: &Env) -> SaleState {
        e.storage().instance().get(&DataKey::State).unwrap()
    }

    pub fn accounting(e: &Env) -> SaleAccounting {
        e.storage().instance().get(&DataKey::Accounting).unwrap()
    }

    pub fn last_purchase_at(e: &Env) -> u64 {
        e.storage()
            .instance()
            .get(&DataKey::LastPurchaseAt)
            .unwrap_or(0)
    }

    pub fn reservation(e: &Env, investor: Address) -> Option<Reservation> {
        e.storage()
            .persistent()
            .get(&DataKey::Reservation(investor))
    }

    pub fn paused(e: &Env) -> bool {
        pausable::paused(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}
