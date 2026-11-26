//! The distributor's own storage: the powers, the two contracts and the
//! per-quota counter in instance storage, and one persistent record per
//! holder that was ever paid, plus the expiry policy and the power check
//! built on them. The pause flag and the schema version live in
//! OpenZeppelin's storage.
use soroban_sdk::{contracttype, panic_with_error, Address, Env};

use crate::errors::OfferHoldingsDistributorError;
use crate::types::{Contracts, PowerHolders};

/// The keys of the distributor's own storage entries.
#[contracttype]
pub enum DataKey {
    Powers,
    Contracts,
    /// What one quota has earned from every deposit so far, times
    /// `MAGNITUDE`. Only grows.
    PerQuota,
    /// What a holder was paid so far. Never removed. An expired entry is
    /// archived, not deleted, and comes back with the same value, so a
    /// holder's record cannot read as zero again.
    Credited(Address),
}

/// Ledgers in a day, at about 5 seconds per ledger (86,400 / 5). Storage
/// lifetimes are counted in ledgers, so this converts days into that unit.
const DAY_IN_LEDGERS: u32 = 17_280;

/// How far below the network maximum an entry may fall before a call tops
/// it up: the same rule as the other contracts.
const TTL_THRESHOLD_BELOW_MAX: u32 = 30 * DAY_IN_LEDGERS;

fn ttl_policy(e: &Env) -> (u32, u32) {
    let max = e.storage().max_ttl();
    (max.saturating_sub(TTL_THRESHOLD_BELOW_MAX), max)
}

pub fn extend_instance_ttl(e: &Env) {
    let (threshold, to) = ttl_policy(e);
    e.storage().instance().extend_ttl(threshold, to);
}

pub fn set_powers(e: &Env, holders: &PowerHolders) {
    e.storage().instance().set(&DataKey::Powers, holders);
}

/// Set by the constructor and never removed, so this read cannot fail.
pub fn powers(e: &Env) -> PowerHolders {
    e.storage().instance().get(&DataKey::Powers).unwrap()
}

pub fn set_contracts(e: &Env, contracts: &Contracts) {
    e.storage().instance().set(&DataKey::Contracts, contracts);
}

/// Set by the constructor and never removed, so this read cannot fail.
pub fn contracts(e: &Env) -> Contracts {
    e.storage().instance().get(&DataKey::Contracts).unwrap()
}

/// Zero until the first deposit.
pub fn per_quota(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::PerQuota).unwrap_or(0)
}

pub fn set_per_quota(e: &Env, value: i128) {
    e.storage().instance().set(&DataKey::PerQuota, &value);
}

/// What `holder` was paid so far; zero for a holder never paid. A read
/// extends nothing.
pub fn credited(e: &Env, holder: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::Credited(holder.clone()))
        .unwrap_or(0)
}

/// Writes the holder's record with the longest lifetime the network gives.
pub fn set_credited(e: &Env, holder: &Address, value: i128) {
    let key = DataKey::Credited(holder.clone());
    e.storage().persistent().set(&key, &value);
    let (threshold, to) = ttl_policy(e);
    e.storage().persistent().extend_ttl(&key, threshold, to);
}

/// Requires `caller`'s signature and that it is one of the stored holders
/// of a power. The signature comes first, so a wrong account learns the
/// code only after signing, as on the other contracts.
pub fn require_power(
    e: &Env,
    caller: &Address,
    holders: &[Address],
    err: OfferHoldingsDistributorError,
) {
    caller.require_auth();
    if !holders.contains(caller) {
        panic_with_error!(e, err);
    }
}
