//! Events specific to `offer-distributor`. Each name is the struct name in
//! snake case; `#[topic]` fields become topics, the rest a map in the data,
//! which the SDK encodes with its keys in sorted name order. The paid
//! asset's own `transfer` is emitted before each `distributed`, and
//! OpenZeppelin's `paused` and `unpaused` before `pause_changed`.
use soroban_sdk::{contractevent, Address, BytesN};

/// The distributor's whole configuration, once, at construction.
#[contractevent]
pub struct Configured {
    pub owner: Address,
    pub pauser: Address,
    pub upgrader: Address,
}

/// One payout of a distribution, after the asset's `transfer` from the
/// owner to `recipient`.
#[contractevent]
pub struct Distributed {
    #[topic]
    pub id: BytesN<32>,
    #[topic]
    pub recipient: Address,
    pub asset: Address,
    pub amount: i128,
}

/// A distribution paid in full: `total` is the sum of its `count` payouts.
/// Emitted once, after the last `distributed`.
#[contractevent]
pub struct DistributionCompleted {
    #[topic]
    pub id: BytesN<32>,
    pub asset: Address,
    pub total: i128,
    pub count: u32,
}

/// The distributor was paused or resumed, and by whom.
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
