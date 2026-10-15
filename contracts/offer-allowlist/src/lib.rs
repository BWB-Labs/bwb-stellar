//! Offer allowlist: who may invest and up to how much. One global instance
//! shared by every offering. Soroban port of Base's `OfferSaleAllowlist`.
//!
//! Each investor has an allocation: a cap in USDC stroops and whether they
//! may invest. Usage is counted per investor per UTC calendar month (the
//! period, ADR 0007), inside the contract, so a new month starts from zero
//! by itself and there is no reset call. A purchase consumes room in the
//! current period and returns that period; a refund gives each period's
//! amount back to that period and fails when a period cannot account for it.
//!
//! Access control is OpenZeppelin's (ADR 0008): the platform multisig is the
//! admin, `controller` is held by the platform hot key and by every sale
//! contract and is its own role-admin, so a controller can grant and revoke
//! `controller`. The admin alone upgrades, and `renounce_admin` always
//! fails, so the allowlist can never become un-upgradeable.
#![no_std]

mod errors;
mod events;
mod period;
mod storage;
mod types;

use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, Address, BytesN, Env, Symbol, Vec,
};
use stellar_access::access_control::{self, AccessControl, AccessControlError};
use stellar_contract_utils::upgradeable::{self, Upgradeable};
use stellar_macros::only_role;

pub use crate::errors::OfferAllowlistError;
use crate::events::{AllocationConsumed, AllocationRestored, AllocationSet, Initialized, Upgraded};
pub use crate::types::{Allocation, AllocationEntry, AllocationView, RestoreEntry};

// Contract metadata that goes on the compiled file.
contractmeta!(key = "binver", val = env!("CARGO_PKG_VERSION"));

/// Version of the storage layout this code writes and reads.
const SCHEMA_VERSION: u32 = 1;

/// The role that writes allocations, consumes and restores room, and grants
/// and revokes itself. Held by the platform hot key and by each offering's
/// sale contract.
pub const CONTROLLER: &str = "controller";

/// Most entries in one `set_allocations` or `restore` call. Bounded by the
/// per-transaction event limit, not by storage.
pub const MAX_BATCH: u32 = 30;

#[contract]
pub struct OfferAllowlist;

#[contractimpl]
impl OfferAllowlist {
    /// Sets `admin` as the access-control admin, makes `controller` its own
    /// role-admin and grants it to each address in `controllers`.
    ///
    /// Emits, in this order: `role_admin_changed` for `controller` (with an
    /// empty previous role), one `role_granted` per controller, then
    /// `initialized`.
    pub fn __constructor(e: &Env, admin: Address, controllers: Vec<Address>) {
        let controller = controller(e);
        access_control::set_admin(e, &admin);
        access_control::set_role_admin_no_auth(e, &controller, &controller);
        for account in controllers.iter() {
            access_control::grant_role_no_auth(e, &account, &controller, &admin);
        }
        upgradeable::set_schema_version(e, SCHEMA_VERSION);
        storage::extend_instance_ttl(e);
        Initialized { admin, controllers }.publish(e);
    }

    /// Writes each investor's cap and enabled flag, at most `MAX_BATCH`
    /// entries. Usage is untouched, so a cap rewrite at a new exchange rate
    /// never gives or takes room already used this period. Duplicates in
    /// one list: last wins. An empty list succeeds as a no-op.
    #[only_role(caller, "controller")]
    pub fn set_allocations(e: &Env, entries: Vec<AllocationEntry>, caller: Address) {
        if entries.len() > MAX_BATCH {
            panic_with_error!(e, OfferAllowlistError::BatchTooLarge);
        }
        for entry in entries.iter() {
            if entry.max < 0 {
                panic_with_error!(e, OfferAllowlistError::InvalidAmount);
            }
            storage::set_allocation(
                e,
                &entry.investor,
                &Allocation {
                    schema: storage::ALLOCATION_SCHEMA,
                    max: entry.max,
                    enabled: entry.enabled,
                },
            );
            AllocationSet {
                investor: entry.investor,
                caller: caller.clone(),
                enabled: entry.enabled,
                max: entry.max,
            }
            .publish(e);
        }
        touched_by_controller(e, &caller);
    }

    /// Uses `amount` of the investor's room in the current period and
    /// returns that period, so the sale contract can keep the amount paid
    /// per period without computing dates. The investor must be enabled and
    /// `amount` must fit `max − usage` (the room). `caller` is normally the
    /// sale contract, authorized as the direct cross-contract caller.
    #[only_role(caller, "controller")]
    pub fn consume(e: &Env, investor: Address, amount: i128, caller: Address) -> u32 {
        if amount <= 0 {
            panic_with_error!(e, OfferAllowlistError::InvalidAmount);
        }
        let allocation = storage::allocation(e, &investor);
        if !allocation.enabled {
            panic_with_error!(e, OfferAllowlistError::InvestorNotEnabled);
        }
        let period = current_period(e);
        let used = storage::usage(e, &investor, period);
        // Compared as `amount` against the room rather than `used + amount`
        // against `max`, so the sum can't overflow. The room is negative when
        // the cap was lowered below this period's usage; then nothing fits.
        if amount > allocation.max - used {
            panic_with_error!(e, OfferAllowlistError::AllocationExceeded);
        }
        let consumed = used + amount;
        storage::set_usage(e, &investor, period, consumed);
        storage::extend_allocation_ttl(e, &investor);
        touched_by_controller(e, &caller);
        AllocationConsumed {
            investor,
            caller,
            amount,
            consumed,
            period,
        }
        .publish(e);
        period
    }

    /// Gives room back, per period: each entry names a period `consume`
    /// returned and the amount to return to it. 1 to `MAX_BATCH` entries,
    /// each amount positive and each period at or before the current one.
    /// Entries are summed per period; a period whose usage can't account
    /// for its sum fails the whole call (6103), which with no reset call can
    /// only be a double restore or a wrong amount. No `enabled` check: a
    /// disabled investor can still be refunded. One `allocation_restored`
    /// per period touched.
    #[only_role(caller, "controller")]
    pub fn restore(e: &Env, investor: Address, entries: Vec<RestoreEntry>, caller: Address) {
        if entries.is_empty() {
            panic_with_error!(e, OfferAllowlistError::InvalidAmount);
        }
        if entries.len() > MAX_BATCH {
            panic_with_error!(e, OfferAllowlistError::BatchTooLarge);
        }
        let current = current_period(e);
        // Sums per period, in first-seen order. At most MAX_BATCH entries,
        // so the quadratic scan is cheaper than a map.
        let mut sums: Vec<(u32, i128)> = Vec::new(e);
        for entry in entries.iter() {
            if entry.amount <= 0 {
                panic_with_error!(e, OfferAllowlistError::InvalidAmount);
            }
            if entry.period > current {
                panic_with_error!(e, OfferAllowlistError::InvalidPeriod);
            }
            let found = sums.iter().position(|(p, _)| p == entry.period);
            match found {
                Some(i) => {
                    let (p, sum) = sums.get_unchecked(i as u32);
                    let Some(sum) = sum.checked_add(entry.amount) else {
                        panic_with_error!(e, OfferAllowlistError::InvalidAmount);
                    };
                    sums.set(i as u32, (p, sum));
                }
                None => sums.push_back((entry.period, entry.amount)),
            }
        }
        for (period, amount) in sums.iter() {
            let used = storage::usage(e, &investor, period);
            if amount > used {
                panic_with_error!(e, OfferAllowlistError::RestoreExceedsUsage);
            }
            let consumed = used - amount;
            storage::set_usage(e, &investor, period, consumed);
            AllocationRestored {
                investor: investor.clone(),
                caller: caller.clone(),
                amount,
                consumed,
                period,
            }
            .publish(e);
        }
        touched_by_controller(e, &caller);
    }

    /// The investor's cap and flag with the current period and its usage.
    /// An investor with no entry reads as cap 0, not enabled.
    pub fn allocation(e: &Env, investor: Address) -> AllocationView {
        let allocation = storage::allocation(e, &investor);
        let period = current_period(e);
        AllocationView {
            consumed: storage::usage(e, &investor, period),
            enabled: allocation.enabled,
            max: allocation.max,
            period,
        }
    }

    /// The investor's usage in `period`, any period. 0 when absent.
    pub fn usage(e: &Env, investor: Address, period: u32) -> i128 {
        storage::usage(e, &investor, period)
    }

    /// The room: `max − usage` in the current period when the investor is
    /// enabled, else 0. Never negative, even when the cap was lowered below
    /// this period's usage.
    pub fn remaining(e: &Env, investor: Address) -> i128 {
        let allocation = storage::allocation(e, &investor);
        if !allocation.enabled {
            return 0;
        }
        let used = storage::usage(e, &investor, current_period(e));
        (allocation.max - used).max(0)
    }

    /// Whether `consume(investor, amount, …)` would pass its amount,
    /// eligibility and room checks at this ledger time. It cannot see the
    /// caller's role. False for a non-positive amount.
    pub fn can_invest(e: &Env, investor: Address, amount: i128) -> bool {
        if amount <= 0 {
            return false;
        }
        let allocation = storage::allocation(e, &investor);
        if !allocation.enabled {
            return false;
        }
        let used = storage::usage(e, &investor, current_period(e));
        amount <= allocation.max - used
    }

    /// The period of the current ledger time: `year * 12 + (month − 1)` of
    /// the UTC calendar month.
    pub fn current_period(e: &Env) -> u32 {
        current_period(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

/// The period of the ledger's timestamp. Fails with `InvalidPeriod` when it
/// does not fit a `u32`, which no real ledger time reaches.
fn current_period(e: &Env) -> u32 {
    match period::from_timestamp(e.ledger().timestamp()) {
        Some(period) => period,
        None => panic_with_error!(e, OfferAllowlistError::InvalidPeriod),
    }
}

fn controller(e: &Env) -> Symbol {
    Symbol::new(e, CONTROLLER)
}

/// After a controller call: keeps the instance and the caller's own role
/// entry alive under the allowlist's policy. The role check has already
/// read the entry, so it exists.
fn touched_by_controller(e: &Env, caller: &Address) {
    storage::extend_role_ttl(e, caller, &controller(e));
    storage::extend_instance_ttl(e);
}

/// OpenZeppelin's role functions, each delegated to the library so that one
/// of them, `renounce_admin`, can be written to fail. State-changing calls
/// also keep the instance alive; reads extend nothing of the allowlist's
/// own (OpenZeppelin extends the role entry it reads, to 90 days).
#[contractimpl]
impl AccessControl for OfferAllowlist {
    fn has_role(e: &Env, account: Address, role: Symbol) -> Option<u32> {
        access_control::has_role(e, &account, &role)
    }

    fn get_existing_roles(e: &Env) -> Vec<Symbol> {
        access_control::get_existing_roles(e)
    }

    fn get_role_member_count(e: &Env, role: Symbol) -> u32 {
        access_control::get_role_member_count(e, &role)
    }

    fn get_role_member(e: &Env, role: Symbol, index: u32) -> Address {
        access_control::get_role_member(e, &role, index)
    }

    fn get_role_admin(e: &Env, role: Symbol) -> Option<Symbol> {
        access_control::get_role_admin(e, &role)
    }

    fn get_admin(e: &Env) -> Option<Address> {
        access_control::get_admin(e)
    }

    /// `caller` must be the admin or hold the role's admin role. Granting a
    /// role already held succeeds silently, with no event.
    fn grant_role(e: &Env, account: Address, role: Symbol, caller: Address) {
        access_control::grant_role(e, &account, &role, &caller);
        storage::extend_instance_ttl(e);
    }

    /// `caller` must be the admin or hold the role's admin role. Revoking a
    /// role the account does not hold fails with 2007.
    fn revoke_role(e: &Env, account: Address, role: Symbol, caller: Address) {
        access_control::revoke_role(e, &account, &role, &caller);
        storage::extend_instance_ttl(e);
    }

    fn renounce_role(e: &Env, role: Symbol, caller: Address) {
        access_control::renounce_role(e, &role, &caller);
        storage::extend_instance_ttl(e);
    }

    /// Two-step admin transfer with a deadline; the admin authorizes.
    fn transfer_admin_role(e: &Env, new_admin: Address, live_until_ledger: u32) {
        access_control::transfer_admin_role(e, &new_admin, live_until_ledger);
        storage::extend_instance_ttl(e);
    }

    /// The pending admin authorizes.
    fn accept_admin_transfer(e: &Env) {
        access_control::accept_admin_transfer(e);
        storage::extend_instance_ttl(e);
    }

    /// Admin only. Moving `controller`'s role-admin to a role nobody holds
    /// leaves the admin as the only account that grants or revokes
    /// `controller`: the containment step for a leaked hot key.
    fn set_role_admin(e: &Env, role: Symbol, admin_role: Symbol) {
        access_control::set_role_admin(e, &role, &admin_role);
        storage::extend_instance_ttl(e);
    }

    /// Always fails: an admin-less allowlist could never be upgraded.
    fn renounce_admin(e: &Env) {
        panic_with_error!(e, OfferAllowlistError::RenounceBlocked);
    }
}

#[contractimpl]
impl Upgradeable for OfferAllowlist {
    /// Replaces the code once this call succeeds. `operator` must be the
    /// admin and authorize; anyone else fails with 2000. The constructor
    /// does not run again; storage is kept as is. No `migrate` in v1.
    fn upgrade(e: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        if access_control::get_admin(e) != Some(operator.clone()) {
            panic_with_error!(e, AccessControlError::Unauthorized);
        }
        operator.require_auth();
        storage::extend_instance_ttl(e);
        Upgraded {
            from_schema_version: upgradeable::get_schema_version(e),
            new_wasm_hash: new_wasm_hash.clone(),
        }
        .publish(e);
        upgradeable::upgrade(e, &new_wasm_hash);
    }
}

mod test;
