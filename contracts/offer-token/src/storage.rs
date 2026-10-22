//! The token's own storage: the four powers, the offering URI and the
//! whitelist, plus the expiry policy and the caller checks built on them.
//! Balances, allowances, metadata, supply, the pause flag and the schema
//! version live in OpenZeppelin's storage.
use soroban_sdk::{contracttype, panic_with_error, Address, Env, String};
use stellar_tokens::fungible::FungibleStorageKey;

use crate::errors::OfferTokenError;
use crate::types::PowerHolders;

/// The keys of the token's own storage entries: the four powers and the URI
/// in instance storage, one persistent entry per whitelisted account.
/// OpenZeppelin keeps balances, allowances, metadata, supply, pause and
/// schema version under its own keys.
#[contracttype]
pub enum DataKey {
    Admin,
    XferAdmin,
    Pauser,
    Upgrader,
    OfferUri,
    /// Present (and `true`) only for whitelisted accounts.
    Whitelisted(Address),
    /// The account the supply was minted to: the only one that may burn.
    InitialHolder,
}

/// Ledgers in a day, at about 5 seconds per ledger (86,400 / 5). Storage
/// lifetimes are counted in ledgers, so this converts days into that unit.
/// Approximate: ledger close time varies slightly.
const DAY_IN_LEDGERS: u32 = 17_280;

/// How far below the network maximum an entry may fall before a call tops
/// it up. Rent is linear in ledgers, so topping up to the maximum costs the
/// same in total as many short extensions; it just keeps a quiet offering
/// from archiving between calls.
const TTL_THRESHOLD_BELOW_MAX: u32 = 30 * DAY_IN_LEDGERS;

fn ttl_policy(e: &Env) -> (u32, u32) {
    let max = e.storage().max_ttl();
    (max.saturating_sub(TTL_THRESHOLD_BELOW_MAX), max)
}

pub fn extend_instance_ttl(e: &Env) {
    let (threshold, to) = ttl_policy(e);
    e.storage().instance().extend_ttl(threshold, to);
}

/// Extends one persistent entry under the policy above. The entry must
/// exist: callers only pass keys they have just read or written.
fn extend_persistent_ttl<K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(e: &Env, key: &K) {
    let (threshold, to) = ttl_policy(e);
    e.storage().persistent().extend_ttl(key, threshold, to);
}

/// Extends a balance a transfer or mint just wrote, so it exists.
/// OpenZeppelin keeps its own 30-day rule for balances it alone touches.
///
/// A wrapper over `extend_persistent_ttl`, kept for the single
/// responsibility principle: only this module knows where OpenZeppelin
/// stores a balance (`FungibleStorageKey::Balance`), so callers ask to
/// extend an account's balance without depending on that key.
pub fn extend_balance_ttl(e: &Env, account: &Address) {
    extend_persistent_ttl(e, &FungibleStorageKey::Balance(account.clone()));
}

/// Stores the four powers, once, at construction. Each keeps its own key,
/// so reading one power doesn't load the others.
pub fn set_powers(e: &Env, holders: &PowerHolders) {
    let i = e.storage().instance();
    i.set(&DataKey::Admin, &holders.admin);
    i.set(&DataKey::XferAdmin, &holders.xfer_admin);
    i.set(&DataKey::Pauser, &holders.pauser);
    i.set(&DataKey::Upgrader, &holders.upgrader);
}

// The four powers are set by the constructor and never removed, so these
// reads cannot fail.

pub fn admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn xfer_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::XferAdmin).unwrap()
}

pub fn pauser(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Pauser).unwrap()
}

pub fn upgrader(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Upgrader).unwrap()
}

/// Stored once, at construction, next to the powers.
pub fn set_initial_holder(e: &Env, holder: &Address) {
    e.storage().instance().set(&DataKey::InitialHolder, holder);
}

pub fn initial_holder(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::InitialHolder).unwrap()
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
        extend_persistent_ttl(e, &key);
    }
    listed
}

pub fn set_whitelisted(e: &Env, account: &Address, allowed: bool) {
    let key = DataKey::Whitelisted(account.clone());
    if allowed {
        e.storage().persistent().set(&key, &true);
        extend_persistent_ttl(e, &key);
    } else {
        e.storage().persistent().remove(&key);
    }
}
