//! Offer token: the investor's quotas in one offering. One instance per
//! offering. Soroban port of Base's `OfferERC20`.
//!
//! A SEP-41 token with 0 decimals whose whole supply is minted once, at
//! construction, to the initial holder (the offering's sale contract). An
//! ordinary transfer passes only if the sender or the recipient is
//! whitelisted, so investors can only send quotas back to operational
//! accounts.
//!
//! Four powers, each a stored address fixed at construction with no setter
//! (ADR 0005): `admin` manages the whitelist and the offering URI, pauses and
//! unpauses; `xfer_admin` moves tokens between any two accounts, ignoring
//! pause and the whitelist; `pauser` pauses but never unpauses; `upgrader`
//! replaces the code. There is no access-control root, so no holder can grant
//! itself another's power. Which account holds each is decided per
//! deployment.
//!
//! There is no burn and no later mint: `burn` and `burn_from` exist for the
//! SEP-41 shape and always fail.
#![no_std]

mod errors;
pub mod events;
mod storage;
mod whitelist;

use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, Address, BytesN, Env, MuxedAddress,
    String, Vec,
};
use stellar_contract_utils::{
    pausable::{self, Pausable},
    upgradeable::{self, Upgradeable},
};
use stellar_macros::when_not_paused;
use stellar_tokens::fungible::{emit_transfer, Base, ContractOverrides, FungibleToken};

pub use crate::errors::OfferTokenError;
use crate::events::{
    AdminTransfer, Initialized, OfferUriUpdated, PauseChanged, Upgraded, WhitelistUpdated,
};
use crate::whitelist::Whitelisted;

contractmeta!(key = "binver", val = env!("CARGO_PKG_VERSION"));

/// Version of the storage layout this code writes and reads.
const SCHEMA_VERSION: u32 = 1;

#[contract]
pub struct OfferToken;

#[contractimpl]
impl OfferToken {
    /// Sets the four powers, mints the whole supply to `initial_holder` and
    /// whitelists it together with `whitelist` (duplicates ignored).
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        e: &Env,
        admin: Address,
        xfer_admin: Address,
        pauser: Address,
        upgrader: Address,
        name: String,
        symbol: String,
        supply: i128,
        initial_holder: Address,
        whitelist: Vec<Address>,
        offer_uri: String,
    ) {
        if supply <= 0 {
            panic_with_error!(e, OfferTokenError::InvalidSupply);
        }
        storage::set_powers(e, &admin, &xfer_admin, &pauser, &upgrader);
        storage::set_offer_uri(e, &offer_uri);
        upgradeable::set_schema_version(e, SCHEMA_VERSION);
        Base::set_metadata(e, 0, name, symbol);
        storage::extend_instance(e);

        Initialized {
            admin,
            xfer_admin,
            pauser,
            upgrader,
            initial_holder: initial_holder.clone(),
            supply,
            offer_uri,
        }
        .publish(e);

        Base::mint(e, &initial_holder, supply);
        storage::extend_balances(e, &initial_holder, &initial_holder);

        whitelist_if_new(e, &initial_holder);
        for account in whitelist.iter() {
            whitelist_if_new(e, &account);
        }
    }

    /// Adds `account` to the whitelist or removes it. Emits every time, as
    /// Base does, even when nothing changes.
    pub fn set_transfer_whitelist(e: &Env, account: Address, allowed: bool, caller: Address) {
        storage::require_power(e, &caller, &storage::admin(e), OfferTokenError::NotAdmin);
        storage::set_whitelisted(e, &account, allowed);
        storage::extend_instance(e);
        WhitelistUpdated { account, allowed }.publish(e);
    }

    pub fn set_offer_uri(e: &Env, uri: String, caller: Address) {
        storage::require_power(e, &caller, &storage::admin(e), OfferTokenError::NotAdmin);
        storage::set_offer_uri(e, &uri);
        storage::extend_instance(e);
        OfferUriUpdated { uri }.publish(e);
    }

    /// Moves `amount` from `from` to `to`, ignoring pause and the whitelist:
    /// court orders, lost keys, estates, manual returns.
    pub fn admin_transfer(e: &Env, from: Address, to: Address, amount: i128, caller: Address) {
        storage::require_power(
            e,
            &caller,
            &storage::xfer_admin(e),
            OfferTokenError::NotXferAdmin,
        );
        if from == to || amount <= 0 {
            panic_with_error!(e, OfferTokenError::InvalidAdminTransfer);
        }
        Base::update(e, Some(&from), Some(&to), amount);
        storage::extend_balances(e, &from, &to);
        storage::extend_instance(e);
        emit_transfer(e, &from, &to, None, amount);
        AdminTransfer { from, to, amount }.publish(e);
    }

    /// SEP-41 shape only: the supply is fixed.
    pub fn burn(e: &Env, _from: Address, _amount: i128) {
        panic_with_error!(e, OfferTokenError::BurnDisabled);
    }

    /// SEP-41 shape only: the supply is fixed.
    pub fn burn_from(e: &Env, _spender: Address, _from: Address, _amount: i128) {
        panic_with_error!(e, OfferTokenError::BurnDisabled);
    }

    pub fn is_transfer_whitelisted(e: &Env, account: Address) -> bool {
        storage::is_whitelisted(e, &account)
    }

    pub fn offer_uri(e: &Env) -> String {
        storage::offer_uri(e)
    }

    pub fn admin(e: &Env) -> Address {
        storage::admin(e)
    }

    pub fn xfer_admin(e: &Env) -> Address {
        storage::xfer_admin(e)
    }

    pub fn pauser(e: &Env) -> Address {
        storage::pauser(e)
    }

    pub fn upgrader(e: &Env) -> Address {
        storage::upgrader(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

fn whitelist_if_new(e: &Env, account: &Address) {
    if !storage::is_whitelisted(e, account) {
        storage::set_whitelisted(e, account, true);
        WhitelistUpdated {
            account: account.clone(),
            allowed: true,
        }
        .publish(e);
    }
}

#[contractimpl(contracttrait)]
impl FungibleToken for OfferToken {
    type ContractType = Whitelisted;

    #[when_not_paused]
    fn transfer(e: &Env, from: Address, to: MuxedAddress, amount: i128) {
        Self::ContractType::transfer(e, &from, &to, amount);
        storage::extend_instance(e);
    }

    #[when_not_paused]
    fn transfer_from(e: &Env, spender: Address, from: Address, to: Address, amount: i128) {
        Self::ContractType::transfer_from(e, &spender, &from, &to, amount);
        storage::extend_instance(e);
    }

    fn approve(e: &Env, owner: Address, spender: Address, amount: i128, live_until_ledger: u32) {
        Self::ContractType::approve(e, &owner, &spender, amount, live_until_ledger);
        storage::extend_instance(e);
    }
}

#[contractimpl]
impl Pausable for OfferToken {
    fn paused(e: &Env) -> bool {
        pausable::paused(e)
    }

    /// `admin` or `pauser` may pause.
    fn pause(e: &Env, caller: Address) {
        caller.require_auth();
        if caller != storage::admin(e) && caller != storage::pauser(e) {
            panic_with_error!(e, OfferTokenError::NotPauser);
        }
        pausable::pause(e);
        storage::extend_instance(e);
        PauseChanged {
            caller,
            paused: true,
        }
        .publish(e);
    }

    /// Only `admin` unpauses.
    fn unpause(e: &Env, caller: Address) {
        storage::require_power(e, &caller, &storage::admin(e), OfferTokenError::NotAdmin);
        pausable::unpause(e);
        storage::extend_instance(e);
        PauseChanged {
            caller,
            paused: false,
        }
        .publish(e);
    }
}

#[contractimpl]
impl Upgradeable for OfferToken {
    /// Replaces the code once this call succeeds. The constructor does not
    /// run again; storage is kept as is.
    fn upgrade(e: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        storage::require_power(
            e,
            &operator,
            &storage::upgrader(e),
            OfferTokenError::NotUpgrader,
        );
        storage::extend_instance(e);
        Upgraded {
            new_wasm_hash: new_wasm_hash.clone(),
            from_schema_version: upgradeable::get_schema_version(e),
        }
        .publish(e);
        upgradeable::upgrade(e, &new_wasm_hash);
    }
}

mod test;
