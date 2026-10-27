//! The sale's stored and passed values. Each is a `#[contracttype]`, so the
//! CLI takes it as one named JSON value and fields can't be swapped by
//! position.
use soroban_sdk::{contracttype, Address, Map};

/// Who holds each of the sale's three powers. Fixed at construction, no
/// setter: changing a holder takes an upgrade, as on the token.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PowerHolders {
    /// The tokenizer's treasury: closes, reopens and cancels the offering,
    /// withdraws, pauses and unpauses.
    pub owner: Address,
    /// The platform multisig: pauses, never unpauses.
    pub pauser: Address,
    /// The platform multisig: replaces the code.
    pub upgrader: Address,
}

/// The contracts one sale works with, fixed at construction.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Contracts {
    /// The offering's `offer-token`, whose whole supply this contract holds.
    pub token: Address,
    /// The SEP-41 asset investors pay with (USDC in production).
    pub payment_asset: Address,
    /// The global `offer-allowlist`.
    pub allowlist: Address,
}

/// The offering's terms, fixed at construction.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SaleConfig {
    /// Base units of the payment asset per token. Positive.
    pub price: i128,
    /// Seconds an investor may undo a purchase for, counted from it. At
    /// least one day.
    pub cooling_off_secs: u64,
    /// Share of the supply that must be sold for the offering to succeed,
    /// 10 to 100.
    pub min_success_percent: u32,
}

/// Where the offering is in its lifecycle. No state goes back except
/// Closing to Active: by the owner's `reopen`, or by itself when a
/// cooling-off refund leaves the offering below its minimum.
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaleState {
    /// Deployed, not yet selling.
    Preparing,
    /// Buying open.
    Active,
    /// Buying closed; cooling-off windows running out.
    Closing,
    /// Decided a success: release and payout, nothing undone.
    Successful,
    /// Decided a failure: refunds and token withdrawal.
    Failed,
}

/// One purchase, kept under its allowlist receipt number.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Purchase {
    pub tokens: i128,
    pub paid: i128,
    /// Ledger time of the purchase; its window runs from here.
    pub at: u64,
}

/// What an investor holds between buying and release or refund: the
/// totals and each live purchase by receipt number.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reservation {
    /// Layout version of this struct.
    pub schema: u32,
    pub tokens: i128,
    pub paid: i128,
    pub purchases: Map<u32, Purchase>,
}

/// The offering's accounting: the supply for sale, and the running
/// figures of tokens sold and released and of money raised and withdrawn.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SaleAccounting {
    /// The token's supply when the offering was activated; the tokens for
    /// sale. 0 before.
    pub supply: i128,
    /// Tokens sold: reserved or released, net of refunds. Only the unsold
    /// ones are burned.
    pub sold_tokens: i128,
    /// Payment received, net of refunds. Withdrawals don't reduce it.
    pub raised: i128,
    /// Tokens already delivered by `release`.
    pub released_tokens: i128,
    /// Payment already taken by `withdraw_payment`.
    pub withdrawn: i128,
}

/// One leg of a payout.
#[contracttype]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Payout {
    pub recipient: Address,
    pub amount: i128,
}
