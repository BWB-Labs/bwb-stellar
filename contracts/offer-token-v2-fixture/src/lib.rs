//! TEST FIXTURE, never deployed. A second binary for `offer-token`'s upgrade
//! test: it declares the v1 storage layout by hand, exposes reads over it and
//! adds `v2_marker`, so the test can prove the new code runs on the old
//! state. If v1's layout changes without this file following, the upgrade
//! test fails, which is the point.
#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, MuxedAddress, String};
use stellar_contract_utils::{pausable, upgradeable};
use stellar_tokens::fungible::{Base, FungibleToken};

/// Must match `offer-token`'s `DataKey` variant by variant.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    XferAdmin,
    Pauser,
    Upgrader,
    OfferUri,
    Whitelisted(Address),
}

#[contract]
pub struct OfferTokenV2Fixture;

fn get(e: &Env, key: &DataKey) -> Address {
    e.storage().instance().get(key).unwrap()
}

#[contractimpl]
impl OfferTokenV2Fixture {
    pub fn v2_marker() -> u32 {
        2
    }

    pub fn admin(e: &Env) -> Address {
        get(e, &DataKey::Admin)
    }

    pub fn xfer_admin(e: &Env) -> Address {
        get(e, &DataKey::XferAdmin)
    }

    pub fn pauser(e: &Env) -> Address {
        get(e, &DataKey::Pauser)
    }

    pub fn upgrader(e: &Env) -> Address {
        get(e, &DataKey::Upgrader)
    }

    pub fn offer_uri(e: &Env) -> String {
        e.storage().instance().get(&DataKey::OfferUri).unwrap()
    }

    pub fn is_transfer_whitelisted(e: &Env, account: Address) -> bool {
        e.storage().persistent().has(&DataKey::Whitelisted(account))
    }

    pub fn paused(e: &Env) -> bool {
        pausable::paused(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

#[contractimpl(contracttrait)]
impl FungibleToken for OfferTokenV2Fixture {
    type ContractType = Base;
}
