use soroban_sdk::contracterror;

/// Contract errors of `offer-sale`, in BWB's 6200–6299 range. Codes from the
/// contracts it calls surface unchanged: the allowlist's 61xx, the token's
/// 60xx, OpenZeppelin's (100–104 token, 1000–1001 pause) and the payment
/// asset's; the sale document lists them.
#[contracterror]
#[repr(u32)]
pub enum OfferSaleError {
    /// `activate` outside Preparing.
    NotPreparing = 6200,
    /// `buy` or `close` outside Active.
    NotActive = 6201,
    /// `release` or `withdraw_payment` outside Successful.
    NotSuccessful = 6202,
    /// `refund` or `withdraw_tokens` outside Failed.
    NotFailed = 6203,
    /// `activate` while the sale contract does not hold the token's whole
    /// supply.
    InventoryMissing = 6204,
    /// An amount that is not positive: a purchase, a payout, a token
    /// withdrawal.
    NonPositiveAmount = 6205,
    /// What arrived for a purchase is not an exact multiple of the price.
    NotWholeTokens = 6206,
    /// A purchase beyond the tokens left.
    ExceedsSupply = 6207,
    /// No payment arrived for a purchase.
    NothingReceived = 6209,
    /// `cooling_off_refund` with no purchase still inside its window.
    CoolingOffClosed = 6212,
    /// `cooling_off_refund` by an investor with nothing reserved.
    NothingReserved = 6213,
    /// `finalize` while some purchase's cooling-off window is still open.
    CoolingOffOpen = 6214,
    /// More investors than `MAX_RELEASE` in `release`, more than
    /// `MAX_REFUND` or more event units than `REFUND_UNITS` in `refund`,
    /// or more than `MAX_PAYOUTS` payouts.
    BatchTooLarge = 6215,
    /// A constructor argument outside its limits.
    InvalidConfig = 6216,
    /// `finalize` toward Successful while the sale contract holds fewer
    /// tokens than it sold: `xfer_admin` moved some out.
    TokensMissing = 6217,
    /// `withdraw_payment` beyond `raised − withdrawn`.
    ExceedsWithdrawable = 6218,
    /// `activate` before the sale contract holds `sale` on the allowlist.
    NotSale = 6219,
    /// A purchase by an investor who already holds `MAX_PURCHASES` live
    /// purchases in this offering.
    TooManyPurchases = 6220,
    /// `finalize` or `reopen` outside Closing.
    NotClosing = 6221,
    /// The caller signed but is not the stored `owner`.
    NotOwner = 6222,
    /// The caller signed but is neither the stored `pauser` nor `owner`.
    NotPauser = 6223,
    /// The caller signed but is not the stored `upgrader`.
    NotUpgrader = 6224,
    /// `cancel` or `cooling_off_refund` once the offering is decided
    /// (Successful or Failed), or before it is activated.
    NotOpen = 6225,
}
