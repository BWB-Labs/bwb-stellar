use soroban_sdk::contracterror;

/// Contract errors of `offer-token`, in BWB's 6000–6099 range. OpenZeppelin's
/// own codes (100–103 token, 1000–1001 pause) can surface too; the token
/// document lists both.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OfferTokenError {
    /// Neither the sender nor the recipient is whitelisted.
    TransferNotWhitelisted = 6000,
    /// A forced transfer with `from == to` or a non-positive amount.
    InvalidAdminTransfer = 6001,
    /// `burn` and `burn_from` exist for SEP-41 shape only; supply is fixed.
    BurnDisabled = 6002,
    /// The caller signed but is not the stored `admin`.
    NotAdmin = 6003,
    /// The caller signed but is not the stored `xfer_admin`.
    NotXferAdmin = 6004,
    /// The caller signed but is neither the stored `pauser` nor `admin`.
    NotPauser = 6005,
    /// The caller signed but is not the stored `upgrader`.
    NotUpgrader = 6006,
    /// The constructor received a supply that is zero or negative.
    InvalidSupply = 6007,
}
