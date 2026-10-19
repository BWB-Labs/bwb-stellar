//! The allowlist's value types: the stored allocation, the view the backend
//! reads, the entry of `set_allocations` and the stored receipts. Each is a
//! `#[contracttype]` struct, so callers pass named fields (JSON on the CLI)
//! and two fields of the same type can't be swapped by position.
use soroban_sdk::{contracttype, Address, Map};

/// An investor's entry, one persistent record per investor. Usage is kept
/// apart, one entry per period (ADR 0007), so writing the allocation never
/// touches it.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allocation {
    /// Version of this struct's layout.
    pub schema: u32,
    /// The cap, in USDC stroops: the most the investor may have in usage
    /// within one period. 0 is valid and means no room.
    pub max: i128,
    /// Whether the investor may invest at all.
    pub enabled: bool,
}

/// What `allocation(investor)` returns: the stored cap and flag together
/// with the current period and its usage, so the investor page needs one
/// read. An investor with no entry reads as cap 0, not enabled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationView {
    /// The current period's usage.
    pub consumed: i128,
    pub enabled: bool,
    pub max: i128,
    /// The current period, the one `consumed` belongs to.
    pub period: u32,
}

/// One entry of `set_allocations`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationEntry {
    pub investor: Address,
    /// The cap, in USDC stroops. Must be non-negative.
    pub max: i128,
    pub enabled: bool,
}

/// The record of one purchase, kept in its investor's `Receipts` under the
/// number `consume` returned. Who bought and through which sale contract
/// are in the entry's key.
#[contracttype]
#[derive(Debug, PartialEq)]
pub struct Receipt {
    /// What the purchase used, in USDC stroops. Positive.
    pub amount: i128,
    /// The period the amount was charged to.
    pub period: u32,
}

/// One investor's receipts in one sale contract: one persistent entry per
/// `(sale, investor)`, so a purchase only ever touches its own investor's
/// entry. `restore` removes the items it gives back, so each purchase is
/// given back at most once; the entry stays when `items` empties, because
/// it holds `next` and numbers are never reused.
#[contracttype]
pub struct Receipts {
    /// The number the next purchase gets. Never decreases.
    pub next: u32,
    /// The live receipts by number, at most `MAX_RECEIPTS`.
    pub items: Map<u32, Receipt>,
}
