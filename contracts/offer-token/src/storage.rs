//! The token's own storage: the four powers, the offering URI and the
//! whitelist, plus the expiry policy and the caller checks built on them.
//! Balances, allowances, metadata, supply, the pause flag and the schema
//! version live in OpenZeppelin's storage.
use soroban_sdk::{contracttype, panic_with_error, Address, Env, String};
use stellar_tokens::fungible::FungibleStorageKey;

use crate::errors::OfferTokenError;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    XferAdmin,
    Pauser,
    Upgrader,
    OfferUri,
    /// Present (and `true`) only for whitelisted accounts.
    Whitelisted(Address),
}

const DAY_IN_LEDGERS: u32 = 17_280;
/// How far below the network maximum an entry may fall before a call tops
/// it up. Rent is linear in ledgers, so topping up to the maximum costs the
/// same in total as many short extensions; it just keeps a quiet offering
/// from archiving between calls.
const TOP_UP_MARGIN: u32 = 30 * DAY_IN_LEDGERS;

fn ttl_policy(e: &Env) -> (u32, u32) {
    let max = e.storage().max_ttl();
    (max.saturating_sub(TOP_UP_MARGIN), max)
}

pub fn extend_instance(e: &Env) {
    let (threshold, to) = ttl_policy(e);
    e.storage().instance().extend_ttl(threshold, to);
}

fn extend_existing<K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(e: &Env, key: &K) {
    let (threshold, to) = ttl_policy(e);
    e.storage().persistent().extend_ttl(key, threshold, to);
}

/// Extends a balance a transfer or mint just wrote, so it exists.
/// OpenZeppelin keeps its own 30-day rule for balances it alone touches.
pub fn extend_balance(e: &Env, account: &Address) {
    extend_existing(e, &FungibleStorageKey::Balance(account.clone()));
}

pub fn set_powers(
    e: &Env,
    admin: &Address,
    xfer_admin: &Address,
    pauser: &Address,
    upgrader: &Address,
) {
    let i = e.storage().instance();
    i.set(&DataKey::Admin, admin);
    i.set(&DataKey::XferAdmin, xfer_admin);
    i.set(&DataKey::Pauser, pauser);
    i.set(&DataKey::Upgrader, upgrader);
}

fn get(e: &Env, key: &DataKey) -> Address {
    // Set by the constructor and never removed.
    e.storage().instance().get(key).unwrap()
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

/// Requires `caller`'s signature and that it is one of the stored holders
/// of a power. The signature comes first, so a wrong account learns the code
/// only after signing.
pub fn require_power(e: &Env, caller: &Address, holders: &[Address], err: OfferTokenError) {
    caller.require_auth();
    if !holders.contains(caller) {
        panic_with_error!(e, err);
    }
}

pub fn offer_uri(e: &Env) -> String {
    e.storage().instance().get(&DataKey::OfferUri).unwrap()
}

pub fn set_offer_uri(e: &Env, uri: &String) {
    e.storage().instance().set(&DataKey::OfferUri, uri);
}

pub fn is_whitelisted(e: &Env, account: &Address) -> bool {
    let key = DataKey::Whitelisted(account.clone());
    let listed = e.storage().persistent().has(&key);
    if listed {
        extend_existing(e, &key);
    }
    listed
}

pub fn set_whitelisted(e: &Env, account: &Address, allowed: bool) {
    let key = DataKey::Whitelisted(account.clone());
    if allowed {
        e.storage().persistent().set(&key, &true);
        extend_existing(e, &key);
    } else {
        e.storage().persistent().remove(&key);
    }
}
