//! Offer distributor: pays a list of recipients in one call. One instance
//! per offering. Soroban port of Base's push `TokenDistributor`, with the
//! payment made atomic.
//!
//! The owner sends a distribution: an id, an asset and at most
//! `MAX_PAYOUTS` `{recipient, amount}`. In that same call the contract
//! moves each amount from the owner's account straight to its recipient,
//! so it never holds a balance, and the owner's authorization covers the
//! exact list. Either every recipient is paid or nothing happens. The
//! contract knows nothing about what a distribution is for: a payout to an
//! offering's stakeholders and a return to its investors look the same.
//!
//! An id is paid at most once. The backend chooses it (a hash of its own
//! record), a repeated id is refused, and a correction is a new
//! distribution with a new id.
//!
//! Three powers, each a stored address fixed at construction with no
//! setter, as on the token and the sale: `owner` (the treasury that pays)
//! distributes, pauses and unpauses; `pauser` (the platform multisig)
//! pauses, never unpauses; `upgrader` (the platform multisig) replaces the
//! code. A paused distributor blocks `distribute` and nothing else.
#![no_std]

mod errors;
mod events;
mod storage;
mod types;

use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, token, Address, BytesN, Env, Vec,
};
use stellar_contract_utils::{
    pausable::{self, Pausable},
    upgradeable::{self, Upgradeable},
};
use stellar_macros::when_not_paused;

pub use crate::errors::OfferDistributorError;
pub use crate::types::{Payout, PowerHolders};

// Contract metadata that goes on the compiled file.
contractmeta!(key = "binver", val = env!("CARGO_PKG_VERSION"));

/// Version of the storage layout this code writes and reads. It stays 1
/// until mainnet: testnet layouts change freely before then.
const SCHEMA_VERSION: u32 = 1;

/// Most payouts in one `distribute`, the same as the sale's payout. It is
/// a cap, not a promise: a transaction may emit 16,384 B of events, and a
/// full distribution of USDC to classic accounts emits 30 × (the asset's
/// `transfer` 244 B + `distributed` 260 B) + `distribution_completed`
/// 252 B = 15,372 B, 94% of that. An asset whose own event is larger than
/// 276 B does not fit 30; the call then fails whole in simulation, before
/// anyone signs, and the backend sends a shorter list. Adding a field to
/// `distributed` means lowering this in the same release.
pub const MAX_PAYOUTS: u32 = 30;

#[contract]
pub struct OfferDistributor;

#[contractimpl]
impl OfferDistributor {
    /// Sets the three powers. Emits `configured`.
    pub fn __constructor(e: &Env, power_holders: PowerHolders) {
        storage::set_powers(e, &power_holders);
        upgradeable::set_schema_version(e, SCHEMA_VERSION);
        storage::extend_instance_ttl(e);
        events::Configured {
            owner: power_holders.owner,
            pauser: power_holders.pauser,
            upgrader: power_holders.upgrader,
        }
        .publish(e);
    }

    /// Pays every `{recipient, amount}` listed, in `asset`, from the
    /// owner's own account, in this call or not at all. Owner only (6300).
    /// The list holds 1 to `MAX_PAYOUTS` payouts (6303, 6304), `id` was
    /// never paid before (6307), each amount is positive (6305), no payout
    /// goes to the distributor itself (6306) and the amounts fit an `i128`
    /// together (6308). The same recipient may be listed more than once.
    ///
    /// The owner's authorization must cover this call and each `transfer`
    /// the asset makes in its name; a recipient the asset refuses fails
    /// the whole call with the asset's own code. Emits `distributed` per
    /// payout, after each `transfer`, then `distribution_completed`.
    #[when_not_paused]
    pub fn distribute(
        e: &Env,
        id: BytesN<32>,
        asset: Address,
        payouts: Vec<Payout>,
        caller: Address,
    ) {
        require_owner(e, &caller);
        if payouts.is_empty() {
            panic_with_error!(e, OfferDistributorError::EmptyBatch);
        }
        if payouts.len() > MAX_PAYOUTS {
            panic_with_error!(e, OfferDistributorError::BatchTooLarge);
        }
        if storage::is_paid(e, &id) {
            panic_with_error!(e, OfferDistributorError::AlreadyPaid);
        }
        let me = e.current_contract_address();
        let mut total = 0i128;
        for payout in payouts.iter() {
            if payout.amount <= 0 {
                panic_with_error!(e, OfferDistributorError::NonPositiveAmount);
            }
            if payout.recipient == me {
                panic_with_error!(e, OfferDistributorError::InvalidRecipient);
            }
            total = match total.checked_add(payout.amount) {
                Some(sum) => sum,
                None => panic_with_error!(e, OfferDistributorError::AmountOverflow),
            };
        }
        // Recorded before the transfers. A failed transfer undoes the whole
        // call, this write included, so an id is marked only when paid.
        storage::mark_paid(e, &id);
        // The asset is whatever contract the owner named. It can do in the
        // owner's name only what the owner's authorization lists, which is
        // why that authorization should hold these transfers and nothing
        // else (the distributor document, Authorization).
        let payment = token::TokenClient::new(e, &asset);
        for payout in payouts.iter() {
            payment.transfer(&caller, &payout.recipient, &payout.amount);
            events::Distributed {
                id: id.clone(),
                recipient: payout.recipient,
                asset: asset.clone(),
                amount: payout.amount,
            }
            .publish(e);
        }
        storage::extend_instance_ttl(e);
        events::DistributionCompleted {
            id,
            asset,
            total,
            count: payouts.len(),
        }
        .publish(e);
    }

    /// Whether a distribution with this id was paid. The answer to "did my
    /// call go through?" when the response was lost.
    pub fn is_paid(e: &Env, id: BytesN<32>) -> bool {
        storage::is_paid(e, &id)
    }

    pub fn powers(e: &Env) -> PowerHolders {
        storage::powers(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

fn require_owner(e: &Env, caller: &Address) {
    storage::require_power(
        e,
        caller,
        &[storage::powers(e).owner],
        OfferDistributorError::NotOwner,
    );
}

#[contractimpl]
impl Pausable for OfferDistributor {
    fn paused(e: &Env) -> bool {
        pausable::paused(e)
    }

    /// `owner` or `pauser` may pause.
    fn pause(e: &Env, caller: Address) {
        let powers = storage::powers(e);
        storage::require_power(
            e,
            &caller,
            &[powers.owner, powers.pauser],
            OfferDistributorError::NotPauser,
        );
        pausable::pause(e);
        storage::extend_instance_ttl(e);
        events::PauseChanged {
            caller,
            paused: true,
        }
        .publish(e);
    }

    /// Only `owner` unpauses.
    fn unpause(e: &Env, caller: Address) {
        require_owner(e, &caller);
        pausable::unpause(e);
        storage::extend_instance_ttl(e);
        events::PauseChanged {
            caller,
            paused: false,
        }
        .publish(e);
    }
}

#[contractimpl]
impl Upgradeable for OfferDistributor {
    /// Replaces the code once this call succeeds. `operator` must be the
    /// stored `upgrader` and sign. The constructor does not run again;
    /// storage is kept as is. No `migrate` in v1.
    fn upgrade(e: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        storage::require_power(
            e,
            &operator,
            &[storage::powers(e).upgrader],
            OfferDistributorError::NotUpgrader,
        );
        storage::extend_instance_ttl(e);
        events::Upgraded {
            new_wasm_hash: new_wasm_hash.clone(),
            from_schema_version: upgradeable::get_schema_version(e),
        }
        .publish(e);
        upgradeable::upgrade(e, &new_wasm_hash);
    }
}

mod test;
