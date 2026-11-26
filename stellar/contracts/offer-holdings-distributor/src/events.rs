//! Events specific to `offer-holdings-distributor`. Each name is the struct
//! name in snake case; `#[topic]` fields become topics, the rest a map in
//! the data, which the SDK encodes with its keys in sorted name order. The
//! payment asset's own `transfer` is emitted before `deposited`, once per
//! paid holder before `claimed`, and before `withdrawn`; OpenZeppelin's
//! `paused` and `unpaused` before `pause_changed`.
use soroban_sdk::{contractevent, Address, BytesN};

/// The distributor's whole configuration, once, at construction.
#[contractevent]
pub struct Configured {
    pub owner: Address,
    pub pauser: Address,
    pub upgrader: Address,
    pub token: Address,
    pub payment_asset: Address,
}

/// Money the owner put in, after the payment asset's `transfer` from the
/// owner to the distributor.
#[contractevent]
pub struct Deposited {
    pub amount: i128,
}

/// A holder a claim could not pay: an account on the token's whitelist, or
/// one whose payment the asset refused. Its share stays in the distributor
/// and its record is unchanged.
#[contractevent]
pub struct ClaimSkipped {
    #[topic]
    pub holder: Address,
}

/// One claim call: `count` holders were paid `total` together. A paid
/// holder has no event of its own here; the payment asset's `transfer`
/// from the distributor to the holder is the record.
#[contractevent]
pub struct Claimed {
    pub count: u32,
    pub total: i128,
}

/// A balance the owner took out, after the asset's `transfer` from the
/// distributor to the owner.
#[contractevent]
pub struct Withdrawn {
    #[topic]
    pub asset: Address,
    pub amount: i128,
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
