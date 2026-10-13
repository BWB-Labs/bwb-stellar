//! Events specific to `offer-token`. Each name is the struct name in snake
//! case; `#[topic]` fields become topics, the rest a map in the data.
//! OpenZeppelin's `transfer`, `mint`, `approve`, `paused` and `unpaused` are
//! emitted alongside.
use soroban_sdk::{contractevent, Address, BytesN, String};

/// The token's whole configuration, once, at construction.
#[contractevent]
pub struct Initialized {
    pub admin: Address,
    pub xfer_admin: Address,
    pub pauser: Address,
    pub upgrader: Address,
    pub initial_holder: Address,
    pub supply: i128,
    pub offer_uri: String,
}

/// An account became or stopped being an operational account.
#[contractevent]
pub struct OperationalAccountUpdated {
    #[topic]
    pub account: Address,
    pub allowed: bool,
}

/// The offering URI was replaced.
#[contractevent]
pub struct OfferUriUpdated {
    pub uri: String,
}

/// Transfers were paused or resumed, and by whom.
#[contractevent]
pub struct PauseChanged {
    #[topic]
    pub caller: Address,
    pub paused: bool,
}

/// `xfer_admin` moved tokens. A standard `transfer` is emitted too.
#[contractevent]
pub struct AdminTransfer {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// Emitted by the old code just before it is replaced.
#[contractevent]
pub struct Upgraded {
    pub new_wasm_hash: BytesN<32>,
    pub from_schema_version: u32,
}
