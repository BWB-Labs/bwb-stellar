//! Events specific to `offer-allowlist`. Each name is the struct name in
//! snake case; `#[topic]` fields become topics, the rest a map in the data,
//! which the SDK encodes with its keys in sorted name order. OpenZeppelin's
//! `role_granted`, `role_revoked`, `role_admin_changed` and the admin
//! transfer events are emitted alongside.
use soroban_sdk::{contractevent, Address, BytesN, Vec};

/// The allowlist's configuration, once, at construction, after the role
/// events the constructor emits.
#[contractevent]
pub struct Initialized {
    pub admin: Address,
    pub controllers: Vec<Address>,
}

/// An investor's cap and enabled flag were written. Usage is untouched.
#[contractevent]
pub struct AllocationSet {
    #[topic]
    pub investor: Address,
    pub caller: Address,
    pub enabled: bool,
    pub max: i128,
}

/// A purchase used `amount` of the investor's room in `period`; `consumed`
/// is that period's usage after the call. `caller` identifies the offering.
#[contractevent]
pub struct AllocationConsumed {
    #[topic]
    pub investor: Address,
    #[topic]
    pub caller: Address,
    pub amount: i128,
    pub consumed: i128,
    pub period: u32,
}

/// A refund gave `amount` back to `period`; `consumed` is that period's
/// usage after the call. One per period a `restore` call touches.
#[contractevent]
pub struct AllocationRestored {
    #[topic]
    pub investor: Address,
    #[topic]
    pub caller: Address,
    pub amount: i128,
    pub consumed: i128,
    pub period: u32,
}

/// Emitted by the old code just before it is replaced.
#[contractevent]
pub struct Upgraded {
    pub from_schema_version: u32,
    pub new_wasm_hash: BytesN<32>,
}
