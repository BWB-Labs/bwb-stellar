//! Events specific to `offer-sale`. Each name is the struct name in snake
//! case; `#[topic]` fields become topics, the rest a map in the data,
//! which the SDK encodes with its keys in sorted name order. The totals
//! that ride on `purchased` and `refunded` let an indexer rebuild the
//! offering's state from any later event. OpenZeppelin's `paused` and
//! `unpaused`, the token's `transfer` and `burn` and the payment asset's
//! `transfer` are emitted alongside.
use soroban_sdk::{contractevent, Address, BytesN, Symbol};

/// The sale's whole configuration, once, at construction.
#[contractevent]
pub struct Configured {
    pub owner: Address,
    pub pauser: Address,
    pub upgrader: Address,
    pub token: Address,
    pub payment_asset: Address,
    pub allowlist: Address,
    pub price: i128,
    pub cooling_off_secs: u64,
    pub min_success_percent: u32,
}

/// Buying opened; `supply` is the tokens for sale.
#[contractevent]
pub struct Activated {
    pub supply: i128,
}

/// Every transition, with the states as symbols: `preparing`, `active`,
/// `closing`, `successful`, `failed`.
#[contractevent]
pub struct StateChanged {
    pub from: Symbol,
    pub to: Symbol,
}

/// One purchase. `paid` and `tokens` are this purchase; `reserved_*` the
/// investor's totals afterwards; `raised` and `sold_tokens` the offering's.
#[contractevent]
pub struct Purchased {
    #[topic]
    pub investor: Address,
    pub paid: i128,
    pub tokens: i128,
    pub receipt: u32,
    pub at: u64,
    pub reserved_paid: i128,
    pub reserved_tokens: i128,
    pub raised: i128,
    pub sold_tokens: i128,
}

/// Buying closed: `reason` is `owner` or `sold_out`.
#[contractevent]
pub struct Closed {
    pub reason: Symbol,
    pub raised: i128,
    pub sold_tokens: i128,
}

/// Buying reopened from Closing: `reason` is `owner` (the owner's
/// `reopen`) or `below_minimum` (a cooling-off refund left the offering
/// below its minimum).
#[contractevent]
pub struct Reopened {
    pub reason: Symbol,
    pub raised: i128,
    pub sold_tokens: i128,
}

/// The offering succeeded; `burned` unsold tokens were destroyed.
#[contractevent]
pub struct Finalized {
    pub raised: i128,
    pub sold_tokens: i128,
    pub burned: i128,
}

/// `finalize` found the offering below its minimum.
#[contractevent]
pub struct FinalizedFailed {
    pub raised: i128,
    pub sold_tokens: i128,
    pub min_success_percent: u32,
}

/// The owner cancelled the offering.
#[contractevent]
pub struct Cancelled {
    pub raised: i128,
    pub sold_tokens: i128,
}

/// One investor's tokens delivered.
#[contractevent]
pub struct Released {
    #[topic]
    pub investor: Address,
    pub paid: i128,
    pub tokens: i128,
}

/// Money back to one investor: `reason` is `cooling_off` (the purchases
/// still in their window) or `failed` (everything). `raised` and
/// `sold_tokens` are the offering's totals afterwards.
#[contractevent]
pub struct Refunded {
    #[topic]
    pub investor: Address,
    pub paid: i128,
    pub tokens: i128,
    pub reason: Symbol,
    pub raised: i128,
    pub sold_tokens: i128,
}

/// One payout leg.
#[contractevent]
pub struct PaymentWithdrawn {
    #[topic]
    pub recipient: Address,
    pub amount: i128,
}

/// The owner took tokens out of a failed offering.
#[contractevent]
pub struct TokensWithdrawn {
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// The sale was paused or resumed, and by whom.
#[contractevent]
pub struct PauseChanged {
    #[topic]
    pub caller: Address,
    pub paused: bool,
}

/// Emitted by the old code just before it is replaced.
#[contractevent]
pub struct Upgraded {
    pub new_wasm_hash: BytesN<32>,
    pub from_schema_version: u32,
}
