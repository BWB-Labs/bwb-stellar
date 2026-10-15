//! The allowlist's own storage: one allocation per investor and one usage
//! entry per `(investor, period)`, plus the expiry policy. The admin, the
//! roles and the schema version live in OpenZeppelin's storage.
use soroban_sdk::{contracttype, Address, Env, IntoVal, Symbol, Val};
use stellar_access::access_control::AccessControlStorageKey;

use crate::types::Allocation;

/// Version of the `Allocation` struct this code writes.
pub const ALLOCATION_SCHEMA: u32 = 1;

/// The keys of the allowlist's own persistent entries. OpenZeppelin keeps
/// the admin (instance), the roles (persistent) and the schema version
/// (instance) under its own keys.
#[contracttype]
pub enum DataKey {
    /// The investor's cap and enabled flag.
    Allocation(Address),
    /// The investor's usage in one period, as a plain `i128`. Absent means
    /// 0. Never written again once the period is over, so it archives on
    /// its own.
    Usage(Address, u32),
}

/// Ledgers in a day, at about 5 seconds per ledger (86,400 / 5). Storage
/// lifetimes are counted in ledgers, so this converts days into that unit.
/// Approximate: ledger close time varies slightly.
const DAY_IN_LEDGERS: u32 = 17_280;

/// How far below the network maximum an entry may fall before a call tops
/// it up. Rent is linear in ledgers, so topping up to the maximum costs the
/// same in total as many short extensions; it just keeps a quiet entry
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
fn extend_persistent_ttl<K: IntoVal<Env, Val>>(e: &Env, key: &K) {
    let (threshold, to) = ttl_policy(e);
    e.storage().persistent().extend_ttl(key, threshold, to);
}

/// Extends the caller's own role entry, which the role check has just
/// read. OpenZeppelin extends it only to 90 days; a sale contract that is
/// quiet for months (a late cooling-off, late refunds) must still find its
/// `controller` alive. Only this module knows where OpenZeppelin keeps a
/// role membership (`AccessControlStorageKey::HasRole`).
pub fn extend_role_ttl(e: &Env, account: &Address, role: &Symbol) {
    extend_persistent_ttl(
        e,
        &AccessControlStorageKey::HasRole(account.clone(), role.clone()),
    );
}

/// The investor's allocation, or the zero allocation when there is none.
/// A read extends nothing: simulation must not pay rent.
pub fn allocation(e: &Env, investor: &Address) -> Allocation {
    e.storage()
        .persistent()
        .get(&DataKey::Allocation(investor.clone()))
        .unwrap_or(Allocation {
            schema: ALLOCATION_SCHEMA,
            max: 0,
            enabled: false,
        })
}

pub fn set_allocation(e: &Env, investor: &Address, allocation: &Allocation) {
    let key = DataKey::Allocation(investor.clone());
    e.storage().persistent().set(&key, allocation);
    extend_persistent_ttl(e, &key);
}

/// The investor's usage in `period`. Absent means 0. A read extends nothing.
pub fn usage(e: &Env, investor: &Address, period: u32) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::Usage(investor.clone(), period))
        .unwrap_or(0)
}

pub fn set_usage(e: &Env, investor: &Address, period: u32, amount: i128) {
    let key = DataKey::Usage(investor.clone(), period);
    e.storage().persistent().set(&key, &amount);
    extend_persistent_ttl(e, &key);
}

/// Extends an allocation entry a write has just touched, so it exists.
pub fn extend_allocation_ttl(e: &Env, investor: &Address) {
    extend_persistent_ttl(e, &DataKey::Allocation(investor.clone()));
}
