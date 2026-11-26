use soroban_sdk::contracterror;

/// Contract errors of `offer-holdings-distributor`, in BWB's 6400–6499
/// range. Codes from the contracts it calls surface unchanged:
/// OpenZeppelin's pause codes (1000–1001), the quota token's and the
/// payment asset's; the distributor document lists them.
#[contracterror]
#[repr(u32)]
pub enum OfferHoldingsDistributorError {
    /// The caller signed but is not the stored `owner`.
    NotOwner = 6400,
    /// The caller signed but is neither the stored `pauser` nor `owner`.
    NotPauser = 6401,
    /// The caller signed but is not the stored `upgrader`.
    NotUpgrader = 6402,
    /// A claim with no holders.
    EmptyBatch = 6403,
    /// More than `MAX_HOLDERS` holders in one claim.
    BatchTooLarge = 6404,
    /// A deposit, or a withdrawal, whose amount is not positive.
    NonPositiveAmount = 6405,
    /// A deposit while the quota token has no supply: there is nobody to
    /// divide it among.
    NoSupply = 6406,
    /// A deposit too large for the per-quota counter, or a read of a share
    /// too large for an `i128`.
    AmountOverflow = 6407,
}
