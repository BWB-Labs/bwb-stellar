//! The sale's own storage: the powers, the contracts it works with, the
//! terms, the state and totals in instance storage, one persistent
//! reservation per investor, plus the expiry policy and the power check
//! built on them. The pause flag and the schema version live in
//! OpenZeppelin's storage.
use soroban_sdk::{contracttype, panic_with_error, Address, Env};

use crate::errors::OfferSaleError;
use crate::types::{Contracts, PowerHolders, Reservation, SaleConfig, SaleState, Totals};

/// Layout version of the `Reservation` struct.
pub const RESERVATION_SCHEMA: u32 = 1;

/// The keys of the sale's own storage entries.
#[contracttype]
pub enum DataKey {
    Powers,
    Contracts,
    Config,
    State,
    Totals,
    /// Ledger time of the latest purchase; `finalize` waits for its window.
    LastPurchaseAt,
    /// One per investor with something reserved; removed on release or
    /// full refund.
    Reservation(Address),
}

/// Ledgers in a day, at about 5 seconds per ledger (86,400 / 5). Storage
/// lifetimes are counted in ledgers, so this converts days into that unit.
const DAY_IN_LEDGERS: u32 = 17_280;

/// How far below the network maximum an entry may fall before a call tops
/// it up: the same rule as the token and the allowlist.
const TTL_THRESHOLD_BELOW_MAX: u32 = 30 * DAY_IN_LEDGERS;

fn ttl_policy(e: &Env) -> (u32, u32) {
    let max = e.storage().max_ttl();
    (max.saturating_sub(TTL_THRESHOLD_BELOW_MAX), max)
}

pub fn extend_instance_ttl(e: &Env) {
    let (threshold, to) = ttl_policy(e);
    e.storage().instance().extend_ttl(threshold, to);
}

fn extend_reservation_ttl(e: &Env, investor: &Address) {
    let (threshold, to) = ttl_policy(e);
    e.storage()
        .persistent()
        .extend_ttl(&DataKey::Reservation(investor.clone()), threshold, to);
}

// The instance values are set by the constructor (or `activate`, for the
// totals' supply) and never removed, so these reads cannot fail.

pub fn set_powers(e: &Env, holders: &PowerHolders) {
    e.storage().instance().set(&DataKey::Powers, holders);
}

pub fn powers(e: &Env) -> PowerHolders {
    e.storage().instance().get(&DataKey::Powers).unwrap()
}

pub fn set_contracts(e: &Env, contracts: &Contracts) {
    e.storage().instance().set(&DataKey::Contracts, contracts);
}

pub fn contracts(e: &Env) -> Contracts {
    e.storage().instance().get(&DataKey::Contracts).unwrap()
}

pub fn set_config(e: &Env, config: &SaleConfig) {
    e.storage().instance().set(&DataKey::Config, config);
}

pub fn config(e: &Env) -> SaleConfig {
    e.storage().instance().get(&DataKey::Config).unwrap()
}

pub fn set_state(e: &Env, state: SaleState) {
    e.storage().instance().set(&DataKey::State, &state);
}

pub fn state(e: &Env) -> SaleState {
    e.storage().instance().get(&DataKey::State).unwrap()
}

pub fn set_totals(e: &Env, totals: &Totals) {
    e.storage().instance().set(&DataKey::Totals, totals);
}

pub fn totals(e: &Env) -> Totals {
    e.storage().instance().get(&DataKey::Totals).unwrap()
}

pub fn set_last_purchase_at(e: &Env, at: u64) {
    e.storage().instance().set(&DataKey::LastPurchaseAt, &at);
}

/// 0 before the first purchase.
pub fn last_purchase_at(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::LastPurchaseAt)
        .unwrap_or(0)
}

/// Requires `caller`'s signature and that it is one of the stored holders
/// of a power. The signature comes first, so a wrong account learns the
/// code only after signing, as on the token.
pub fn require_power(e: &Env, caller: &Address, holders: &[Address], err: OfferSaleError) {
    caller.require_auth();
    if !holders.contains(caller) {
        panic_with_error!(e, err);
    }
}

/// The investor's reservation, if they hold one. A read extends nothing.
pub fn reservation(e: &Env, investor: &Address) -> Option<Reservation> {
    e.storage()
        .persistent()
        .get(&DataKey::Reservation(investor.clone()))
}

/// Writes the reservation, or removes it once nothing is left so a
/// released or refunded investor leaves nothing behind to pay rent on.
pub fn set_reservation(e: &Env, investor: &Address, reservation: &Reservation) {
    let key = DataKey::Reservation(investor.clone());
    if reservation.purchases.is_empty() {
        e.storage().persistent().remove(&key);
        return;
    }
    e.storage().persistent().set(&key, reservation);
    extend_reservation_ttl(e, investor);
}

pub fn remove_reservation(e: &Env, investor: &Address) {
    e.storage()
        .persistent()
        .remove(&DataKey::Reservation(investor.clone()));
}
