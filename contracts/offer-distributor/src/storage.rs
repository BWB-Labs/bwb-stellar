//! The distributor's own storage: the powers in instance storage and one
//! persistent marker per paid distribution, plus the expiry policy and the
//! power check built on them. The pause flag and the schema version live
//! in OpenZeppelin's storage. The contract holds no balance, so there is
//! no accounting to keep.
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env};

use crate::errors::OfferDistributorError;
use crate::types::PowerHolders;

/// The keys of the distributor's own storage entries.
#[contracttype]
pub enum DataKey {
    Powers,
    /// One per distribution id that was paid. Never removed: an id is paid
    /// at most once for the life of the contract.
    Paid(BytesN<32>),
}

/// Ledgers in a day, at about 5 seconds per ledger (86,400 / 5). Storage
/// lifetimes are counted in ledgers, so this converts days into that unit.
const DAY_IN_LEDGERS: u32 = 17_280;

/// How far below the network maximum an entry may fall before a call tops
/// it up: the same rule as the token, the allowlist and the sale.
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

/// Requires `caller`'s signature and that it is one of the stored holders
/// of a power. The signature comes first, so a wrong account learns the
/// code only after signing, as on the token and the sale.
pub fn require_power(e: &Env, caller: &Address, holders: &[Address], err: OfferDistributorError) {
    caller.require_auth();
    if !holders.contains(caller) {
        panic_with_error!(e, err);
    }
}

/// Whether a distribution with this id was paid. A read extends nothing.
pub fn is_paid(e: &Env, id: &BytesN<32>) -> bool {
    e.storage().persistent().has(&DataKey::Paid(id.clone()))
}

/// Records the id as paid, with the longest lifetime the network gives. An
/// archived marker can be restored but never written again, so the id
/// stays refused even if nobody keeps the entry alive.
pub fn mark_paid(e: &Env, id: &BytesN<32>) {
    let key = DataKey::Paid(id.clone());
    e.storage().persistent().set(&key, &true);
    let (threshold, to) = ttl_policy(e);
    e.storage().persistent().extend_ttl(&key, threshold, to);
}
