//! Offer allowlist: who may invest and up to how much. One global instance
//! shared by every offering. Soroban port of Base's `OfferSaleAllowlist`.
//!
//! Each investor has an allocation: a cap in USDC stroops and whether they
//! may invest. Usage is counted per investor per UTC calendar month (the
//! period, ADR 0007), inside the contract, so a new month starts from zero
//! by itself and there is no reset call. A purchase consumes room in the
//! current period and leaves a receipt, numbered per sale contract and
//! investor; a refund names one investor's receipts, and each gives its
//! amount back to its own period once, and only when the sale contract that
//! made it asks.
//!
//! Access control is OpenZeppelin's (ADR 0008), with two roles: `allocator`
//! (the platform hot key) writes allocations and is the role-admin of
//! `sale`, so the backend wires each new sale contract itself; `sale` (each
//! offering's sale contract) consumes room and grants nothing. Restoring
//! needs no role: a caller can only ever give back receipts it created, so
//! a sale contract whose `sale` was revoked still refunds its investors.
//! Only the admin grants `allocator`. The admin alone upgrades, and
//! `renounce_admin` always fails, so the allowlist can never become
//! un-upgradeable.
#![no_std]

mod errors;
mod events;
mod period;
mod storage;
mod types;

use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, Address, BytesN, Env, Map, Symbol, Vec,
};
use stellar_access::access_control::{self, AccessControl, AccessControlError};
use stellar_contract_utils::upgradeable::{self, Upgradeable};
use stellar_macros::only_role;

pub use crate::errors::OfferAllowlistError;
use crate::events::{AllocationConsumed, AllocationRestored, AllocationSet, Initialized, Upgraded};
pub use crate::types::{Allocation, AllocationEntry, AllocationView, Receipt, ReceiptBook};

// Contract metadata that goes on the compiled file.
contractmeta!(key = "binver", val = env!("CARGO_PKG_VERSION"));

/// Version of the storage layout this code writes and reads.
const SCHEMA_VERSION: u32 = 1;

/// The role that writes allocations and grants and revokes `sale`. Held by
/// the platform hot key; granted only by the admin. The `#[only_role]`
/// attributes below repeat the names as literals because the macro takes
/// one; the constants are for everything else.
pub const ALLOCATOR: &str = "allocator";

/// The role that consumes room. Held by each offering's sale contract;
/// granted and revoked by an allocator or the admin. Grants nothing.
/// Revoking it stops the sale contract's purchases, not its refunds:
/// `restore` takes no role.
pub const SALE: &str = "sale";

/// Most entries in one `set_allocations` call. At 30 it writes 31 entries
/// (the entries plus the instance) and emits about 7.2 KB of events, well
/// inside the network's per-transaction limits (200 write entries, 400
/// footprint entries and 16 KB of events on testnet, 2026-10-20).
pub const MAX_BATCH: u32 = 30;

/// Most receipts in one `restore` call. A restore writes the investor's
/// receipt book once and one usage entry per month it touches, so 20
/// receipts in 20 different months write 21 entries and emit 5,840 B of
/// events, leaving most of the transaction to the calling sale contract.
pub const MAX_RESTORE: u32 = 20;

/// Most live receipts one investor may hold in one sale contract. Bounds
/// the receipt book, which every purchase and refund of that investor
/// reads and writes whole: at 100 it stays far under the 64 KB ledger
/// entry limit.
pub const MAX_RECEIPTS: u32 = 100;

#[contract]
pub struct OfferAllowlist;

#[contractimpl]
impl OfferAllowlist {
    /// Sets `admin` as the access-control admin, makes `allocator` the
    /// role-admin of `sale` and grants `allocator` to each address in
    /// `allocators`. `allocator` itself has no role-admin, so only the admin
    /// grants or revokes it.
    ///
    /// Emits, in this order: `role_admin_changed` for `sale` (with an empty
    /// previous role), one `role_granted` per allocator, then `initialized`.
    pub fn __constructor(e: &Env, admin: Address, allocators: Vec<Address>) {
        let allocator = role(e, ALLOCATOR);
        access_control::set_admin(e, &admin);
        access_control::set_role_admin_no_auth(e, &role(e, SALE), &allocator);
        for account in allocators.iter() {
            access_control::grant_role_no_auth(e, &account, &allocator, &admin);
            storage::extend_role_ttl(e, &account, &allocator);
        }
        upgradeable::set_schema_version(e, SCHEMA_VERSION);
        storage::extend_instance_ttl(e);
        Initialized { admin, allocators }.publish(e);
    }

    /// Writes each investor's cap and enabled flag, at most `MAX_BATCH`
    /// entries. Usage is untouched, so a cap change never gives or takes
    /// room already used this period. Duplicates in one list: last wins. An
    /// empty list succeeds as a no-op.
    #[only_role(caller, "allocator")]
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
        extend_caller_role_ttl(e, &caller, ALLOCATOR);
    }

    /// Uses `amount` of the investor's room in the current period, records
    /// the purchase as a receipt in the investor's receipt book under the
    /// calling sale contract and returns the receipt's number, which the
    /// sale contract keeps with the purchase to refund it later. Numbers
    /// count from 0 per sale contract and investor and are never reused.
    /// The investor must be enabled, `amount` must fit `max − usage` (the
    /// room) and the investor may hold at most `MAX_RECEIPTS` live
    /// receipts in this sale contract. `caller` is normally the sale
    /// contract, authorized as the direct cross-contract caller.
    #[only_role(caller, "sale")]
    pub fn consume(e: &Env, investor: Address, amount: i128, caller: Address) -> u32 {
        if amount <= 0 {
            panic_with_error!(e, OfferAllowlistError::InvalidAmount);
        }
        let (allocation, period, used) = allocation_now(e, &investor);
        if !allocation.enabled {
            panic_with_error!(e, OfferAllowlistError::InvestorNotEnabled);
        }
        if amount > room(&allocation, used) {
            panic_with_error!(e, OfferAllowlistError::AllocationExceeded);
        }
        let mut book =
            storage::receipt_book(e, &caller, &investor).unwrap_or_else(|| ReceiptBook {
                next: 0,
                receipts: Map::new(e),
            });
        if book.receipts.len() >= MAX_RECEIPTS {
            panic_with_error!(e, OfferAllowlistError::TooManyReceipts);
        }
        let number = book.next;
        // u32::MAX purchases by one investor in one sale contract is out of
        // reach; failing keeps a wrapped counter from reusing a number.
        let Some(next) = number.checked_add(1) else {
            panic_with_error!(e, OfferAllowlistError::InvalidAmount);
        };
        book.receipts.set(number, Receipt { amount, period });
        book.next = next;
        let consumed = used + amount;
        storage::set_usage(e, &investor, period, consumed);
        storage::set_receipt_book(e, &caller, &investor, &book);
        storage::extend_allocation_ttl(e, &investor);
        extend_caller_role_ttl(e, &caller, SALE);
        AllocationConsumed {
            investor,
            caller,
            amount,
            consumed,
            period,
            receipt: number,
        }
        .publish(e);
        number
    }

    /// Gives back whole purchases of one investor: each number names one of
    /// the investor's receipt book under the calling sale contract, whose
    /// amount goes back to the investor's usage in its own period. 1 to
    /// `MAX_RESTORE` numbers. Each receipt is removed as it is restored, so
    /// a number that is unknown, already restored or repeated in the list
    /// fails the whole call (6103), as does an investor the sale contract
    /// never sold to. The receipt book is kept once empty: it holds
    /// `next`. No `enabled` check: a disabled investor can still be
    /// refunded. One `allocation_restored` per receipt, in list order.
    ///
    /// `caller` must authorize but needs no role. The lookup is keyed by
    /// `caller`, so it only ever finds receipts `caller` created while
    /// holding `sale`; a sale contract whose role was revoked still
    /// refunds, and anyone else finds nothing (6103).
    pub fn restore(e: &Env, investor: Address, receipts: Vec<u32>, caller: Address) {
        caller.require_auth();
        if receipts.is_empty() {
            panic_with_error!(e, OfferAllowlistError::InvalidAmount);
        }
        if receipts.len() > MAX_RESTORE {
            panic_with_error!(e, OfferAllowlistError::BatchTooLarge);
        }
        let Some(mut book) = storage::receipt_book(e, &caller, &investor) else {
            panic_with_error!(e, OfferAllowlistError::ReceiptNotFound);
        };
        for number in receipts.iter() {
            let Some(receipt) = book.receipts.get(number) else {
                panic_with_error!(e, OfferAllowlistError::ReceiptNotFound);
            };
            give_back(e, &investor, &caller, number, &receipt);
            book.receipts.remove(number);
        }
        finish_restore(e, &investor, &caller, &book);
    }

    /// Gives back every live purchase of one investor under the calling
    /// sale contract: the whole receipt book in one call, each receipt's
    /// amount back to its own period, as `restore` does for named numbers.
    /// For a failed offering, where the sale contract refunds everyone, it
    /// replaces up to five `restore` calls per investor with one; the
    /// caller bounds the live receipts, since each one costs an event. An
    /// investor the sale contract never sold to, or already gave everything
    /// back to, fails with 6103, so a double refund stays visible. Returns
    /// how many receipts were restored. One `allocation_restored` per
    /// receipt, in number order. Same gate as `restore`: `caller`
    /// authorizes, no role.
    pub fn restore_all(e: &Env, investor: Address, caller: Address) -> u32 {
        caller.require_auth();
        let Some(book) = storage::receipt_book(e, &caller, &investor) else {
            panic_with_error!(e, OfferAllowlistError::ReceiptNotFound);
        };
        if book.receipts.is_empty() {
            panic_with_error!(e, OfferAllowlistError::ReceiptNotFound);
        }
        for (number, receipt) in book.receipts.iter() {
            give_back(e, &investor, &caller, number, &receipt);
        }
        let emptied = ReceiptBook {
            next: book.next,
            receipts: Map::new(e),
        };
        finish_restore(e, &investor, &caller, &emptied);
        book.receipts.len()
    }

    /// The investor's cap and flag with the current period and its usage.
    /// An investor with no entry reads as cap 0, not enabled.
    pub fn allocation(e: &Env, investor: Address) -> AllocationView {
        let (allocation, period, used) = allocation_now(e, &investor);
        AllocationView {
            consumed: used,
            enabled: allocation.enabled,
            max: allocation.max,
            period,
        }
    }

    /// The investor's usage in `period`, any period. 0 when absent.
    pub fn usage(e: &Env, investor: Address, period: u32) -> i128 {
        storage::usage(e, &investor, period)
    }

    /// The investor's receipt `receipt` in sale contract `sale`: the amount
    /// and period of that purchase. None once restored, and for a number
    /// never issued.
    pub fn receipt(e: &Env, sale: Address, investor: Address, receipt: u32) -> Option<Receipt> {
        storage::receipt_book(e, &sale, &investor)?
            .receipts
            .get(receipt)
    }

    /// The investor's receipt book in sale contract `sale`: the next number
    /// and every live receipt by number. None if the sale contract never
    /// sold to the investor; an empty map once everything was refunded.
    pub fn receipt_book(e: &Env, sale: Address, investor: Address) -> Option<ReceiptBook> {
        storage::receipt_book(e, &sale, &investor)
    }

    /// The room: `max − usage` in the current period when the investor is
    /// enabled, else 0. Never negative, even when the cap was lowered below
    /// this period's usage.
    pub fn remaining(e: &Env, investor: Address) -> i128 {
        let (allocation, _, used) = allocation_now(e, &investor);
        if !allocation.enabled {
            return 0;
        }
        room(&allocation, used).max(0)
    }

    /// Whether `consume(investor, amount, …)` would pass its amount,
    /// eligibility and room checks at this ledger time. It cannot see the
    /// caller's role. False for a non-positive amount.
    pub fn can_invest(e: &Env, investor: Address, amount: i128) -> bool {
        if amount <= 0 {
            return false;
        }
        let (allocation, _, used) = allocation_now(e, &investor);
        allocation.enabled && amount <= room(&allocation, used)
    }

    /// The period of the current ledger time: `year * 12 + (month − 1)` of
    /// the UTC calendar month.
    pub fn current_period(e: &Env) -> u32 {
        period_now(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

/// The period of the ledger's timestamp. Fails with `InvalidPeriod` when it
/// does not fit a `u32`, which no real ledger time reaches.
fn period_now(e: &Env) -> u32 {
    match period::from_timestamp(e.ledger().timestamp()) {
        Some(period) => period,
        None => panic_with_error!(e, OfferAllowlistError::InvalidPeriod),
    }
}

/// What every room computation starts from: the investor's allocation (or
/// the zero one), the current period and the investor's usage in it. Reads
/// only; extends nothing.
fn allocation_now(e: &Env, investor: &Address) -> (Allocation, u32, i128) {
    let allocation = storage::allocation(e, investor);
    let period = period_now(e);
    let used = storage::usage(e, investor, period);
    (allocation, period, used)
}

/// `max − usage`, the room, possibly negative when the cap was lowered below
/// this period's usage. An amount is compared against it rather than
/// `used + amount` against `max`, so the sum can't overflow.
fn room(allocation: &Allocation, used: i128) -> i128 {
    allocation.max - used
}

fn role(e: &Env, name: &str) -> Symbol {
    Symbol::new(e, name)
}

/// One receipt back to its period: subtracts its amount from the investor's
/// usage there and emits `allocation_restored`. The caller removes the
/// receipt from the book.
fn give_back(e: &Env, investor: &Address, caller: &Address, number: u32, receipt: &Receipt) {
    let used = storage::usage(e, investor, receipt.period);
    // Usage is the sum of its period's live receipts, so it always covers
    // one of them; fail rather than write a negative usage if that ever
    // stops holding.
    if receipt.amount > used {
        panic_with_error!(e, OfferAllowlistError::InvalidAmount);
    }
    let consumed = used - receipt.amount;
    storage::set_usage(e, investor, receipt.period, consumed);
    AllocationRestored {
        investor: investor.clone(),
        caller: caller.clone(),
        amount: receipt.amount,
        consumed,
        period: receipt.period,
        receipt: number,
    }
    .publish(e);
}

/// After a restore: writes the book once and keeps the caller's role entry
/// (while held) and the instance alive.
fn finish_restore(e: &Env, investor: &Address, caller: &Address, book: &ReceiptBook) {
    storage::set_receipt_book(e, caller, investor, book);
    storage::extend_role_ttl_if_held(e, caller, &role(e, SALE));
    storage::extend_instance_ttl(e);
}

/// After a role-gated write: keeps the instance and the caller's own entry
/// for `role_name` alive under the allowlist's policy. The role check has
/// already read the entry, so it exists.
fn extend_caller_role_ttl(e: &Env, caller: &Address, role_name: &str) {
    storage::extend_role_ttl(e, caller, &role(e, role_name));
    storage::extend_instance_ttl(e);
}

/// OpenZeppelin's role functions, each delegated to the library so that one
/// of them, `renounce_admin`, can be written to fail. State-changing calls
/// also keep the instance alive. The getters are not rent-free: `has_role`,
/// `get_role_admin`, `get_role_member_count`, `get_role_member` and
/// `get_existing_roles` each extend the role entry they read to 90 days,
/// OpenZeppelin's own rule.
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

    /// `caller` must be the admin or hold the role's admin role (an
    /// allocator, for `sale`). Granting a role already book succeeds
    /// silently, with no event. The granted entry is topped up to the
    /// network maximum, so a freshly wired sale contract does not start at
    /// the minimum lifetime.
    fn grant_role(e: &Env, account: Address, role: Symbol, caller: Address) {
        access_control::grant_role(e, &account, &role, &caller);
        storage::extend_role_ttl(e, &account, &role);
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

    /// Admin only. Moving `sale`'s role-admin to a role nobody holds
    /// freezes sale grants while a leaked hot key is cleaned up; moving it
    /// back to `allocator` restores the backend's wiring.
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
        // Membership before the signature, the order `#[only_role]` uses on
        // the role-gated writes, so every gate on this contract reads alike.
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
