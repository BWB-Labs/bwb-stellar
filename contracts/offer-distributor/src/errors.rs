use soroban_sdk::contracterror;

/// Contract errors of `offer-distributor`, in BWB's 6300–6399 range. Codes
/// from the contracts it calls surface unchanged: OpenZeppelin's pause
/// codes (1000–1001) and the paid asset's; the distributor document lists
/// them.
#[contracterror]
#[repr(u32)]
pub enum OfferDistributorError {
    /// The caller signed but is not the stored `owner`.
    NotOwner = 6300,
    /// The caller signed but is neither the stored `pauser` nor `owner`.
    NotPauser = 6301,
    /// The caller signed but is not the stored `upgrader`.
    NotUpgrader = 6302,
    /// A distribution with no payouts.
    EmptyBatch = 6303,
    /// More than `MAX_PAYOUTS` payouts in one distribution.
    BatchTooLarge = 6304,
    /// A payout whose amount is not positive.
    NonPositiveAmount = 6305,
    /// A payout to the distributor itself: it never holds money.
    InvalidRecipient = 6306,
    /// The distribution id was already paid. A correction is a new
    /// distribution with a new id.
    AlreadyPaid = 6307,
    /// The payouts add up to more than an `i128` holds.
    AmountOverflow = 6308,
}
