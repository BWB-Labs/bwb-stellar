//! The allowlist's value types: the stored allocation, the view the backend
//! reads, and the entries of the two batched calls. Each is a
//! `#[contracttype]` struct, so callers pass named fields (JSON on the CLI)
//! and two fields of the same type can't be swapped by position.
use soroban_sdk::{contracttype, Address};

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

/// One entry of `restore`: an amount to give back to the period it was
/// consumed in, the period `consume` returned at the time.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreEntry {
    pub period: u32,
    /// Must be positive.
    pub amount: i128,
}
