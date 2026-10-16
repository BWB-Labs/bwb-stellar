//! TEST FIXTURE, never deployed. A second binary for `offer-allowlist`'s
//! upgrade test: it declares the v1 storage layout by hand, exposes reads
//! over it and adds `v2_marker`, so the test can prove the new code runs on
//! the old state. If v1's layout changes without this file following, the
//! upgrade test fails, which is the point.
#![no_std]

// `Symbol` and `Vec` are used by the trait's default methods.
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, BytesN, Env,
    Symbol, Vec,
};
use stellar_access::access_control::{self, AccessControl, AccessControlError};
use stellar_contract_utils::upgradeable;

/// Must match `offer-allowlist`'s `DataKey` variant by variant.
#[contracttype]
pub enum DataKey {
    Allocation(Address),
    Usage(Address, u32),
}

/// Must match `offer-allowlist`'s `Allocation` field by field.
#[contracttype]
pub struct Allocation {
    pub schema: u32,
    pub max: i128,
    pub enabled: bool,
}

/// Same code as v1's `RenounceBlocked`.
#[contracterror]
#[repr(u32)]
pub enum FixtureError {
    RenounceBlocked = 6105,
}

#[contract]
pub struct OfferAllowlistV2Fixture;

#[contractimpl]
impl OfferAllowlistV2Fixture {
    pub fn v2_marker() -> u32 {
        2
    }

    /// The upgraded code must itself stay upgradeable, gated by the same
    /// stored admin, or one bad release would freeze the contract.
    pub fn upgrade(e: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        if access_control::get_admin(e) != Some(operator.clone()) {
            panic_with_error!(e, AccessControlError::Unauthorized);
        }
        operator.require_auth();
        upgradeable::upgrade(e, &new_wasm_hash);
    }

    /// The stored v1 struct. The test only reads investors it wrote.
    pub fn allocation(e: &Env, investor: Address) -> Allocation {
        e.storage()
            .persistent()
            .get(&DataKey::Allocation(investor))
            .unwrap()
    }

    pub fn usage(e: &Env, investor: Address, period: u32) -> i128 {
        e.storage()
            .persistent()
            .get(&DataKey::Usage(investor, period))
            .unwrap_or(0)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

/// OpenZeppelin's defaults read the v1 role storage. `renounce_admin` is
/// overridden to fail as in v1: every future version must keep this
/// override, or one release could make the allowlist admin-less and
/// therefore un-upgradeable (the upgrade test checks it).
#[contractimpl(contracttrait)]
impl AccessControl for OfferAllowlistV2Fixture {
    fn renounce_admin(e: &Env) {
        panic_with_error!(e, FixtureError::RenounceBlocked);
    }
}
