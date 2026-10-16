use soroban_sdk::contracterror;

/// Contract errors of `offer-allowlist`, in BWB's 6100–6199 range.
/// OpenZeppelin's access-control codes surface unchanged: 2000
/// `Unauthorized` (a caller without the role, a non-admin operator on
/// `upgrade`) and 2007 `RoleNotHeld` (revoking a role not held). The
/// allowlist document lists both.
#[contracterror]
#[repr(u32)]
pub enum OfferAllowlistError {
    /// `consume` for a disabled or unknown investor. The eligibility
    /// rejection.
    InvestorNotEnabled = 6100,
    /// `consume` beyond the investor's room this period. The allocation
    /// rejection.
    AllocationExceeded = 6101,
    /// A negative cap, a non-positive amount, an empty restore list, or a
    /// restore sum that overflows.
    InvalidAmount = 6102,
    /// A period cannot account for the sum restored to it.
    RestoreExceedsUsage = 6103,
    /// More than `MAX_BATCH` entries in `set_allocations` or `restore`.
    BatchTooLarge = 6104,
    /// `renounce_admin` always fails: an admin-less allowlist could never be
    /// upgraded.
    RenounceBlocked = 6105,
    /// A restore period after the current one, or a ledger timestamp whose
    /// period does not fit a `u32`.
    InvalidPeriod = 6106,
}
