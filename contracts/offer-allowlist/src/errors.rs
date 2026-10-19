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
    /// A negative cap, a non-positive amount or an empty restore list. Also
    /// two states the contract never reaches: an investor out of receipt
    /// numbers in one sale contract (`u32::MAX` purchases), and a receipt
    /// larger than its period's usage, which is always the sum of its live
    /// receipts.
    InvalidAmount = 6102,
    /// `restore` names a receipt the calling sale contract does not hold
    /// for that investor: never issued, already restored, or repeated in
    /// the same list. Another sale contract's receipts live under another
    /// key, so they are never found either.
    ReceiptNotFound = 6103,
    /// More than `MAX_BATCH` entries in `set_allocations`, or more than
    /// `MAX_RESTORE` receipts in `restore`.
    BatchTooLarge = 6104,
    /// `renounce_admin` always fails: an admin-less allowlist could never be
    /// upgraded.
    RenounceBlocked = 6105,
    /// A ledger timestamp whose period does not fit a `u32`, which no real
    /// ledger time reaches.
    InvalidPeriod = 6106,
    /// `consume` for an investor who already has `MAX_RECEIPTS` live
    /// receipts in the calling sale contract. A restore frees room.
    TooManyReceipts = 6107,
}
