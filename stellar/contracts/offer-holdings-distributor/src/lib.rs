//! Offer holdings distributor: pays an offering's returns to whoever holds
//! its quotas. One instance per offering. Soroban port of Base's
//! `TokenDistributorByHoldings`.
//!
//! The owner deposits money. A deposit touches no holder: it only raises
//! one counter, what a single quota has earned so far. A holder's share is
//! its quota balance times that counter, less what it was already paid,
//! and it leaves the contract in a claim. Anyone may send a claim for a
//! list of holders; the money goes to each holder, never to the sender.
//!
//! The balance is read when the claim runs and the "paid so far" record is
//! kept per account. Quotas that move after their holder claimed can
//! therefore be claimed again by the receiver, as on Base. Quotas move
//! only through the token's whitelist and its forced transfer, and an
//! account on the whitelist cannot claim; the distributor document tells
//! what is left of the risk and the rule that covers it.
//!
//! Three powers, each a stored address fixed at construction with no
//! setter, as on the other contracts: `owner` (the issuer's treasury)
//! deposits, withdraws, pauses and unpauses; `pauser` (the platform
//! multisig) pauses, never unpauses; `upgrader` (the platform multisig)
//! replaces the code. A paused distributor blocks `deposit` and `claim`.
//!
//! `withdraw` lets the owner take any balance into its own account, and
//! nowhere else: the dust divisions leave, the share of quotas nobody can
//! claim for, a token sent by mistake, and, as on Base, money holders have
//! not claimed yet.
#![no_std]

mod errors;
mod events;
mod offer_token;
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

pub use crate::errors::OfferHoldingsDistributorError;
pub use crate::types::{Contracts, PowerHolders};

use crate::offer_token::QuotaTokenClient;

// Contract metadata that goes on the compiled file.
contractmeta!(key = "binver", val = env!("CARGO_PKG_VERSION"));

/// Version of the storage layout this code writes and reads. It stays 1
/// until mainnet: testnet layouts change freely before then.
const SCHEMA_VERSION: u32 = 1;

/// The per-quota counter is kept times this, so a deposit smaller than the
/// supply still raises it. The same value as Base.
pub const MAGNITUDE: i128 = 1_000_000_000_000_000_000;

/// Most holders in one `claim`. It is a cap, not a promise: a transaction
/// may emit 16,384 B of events and each paid holder costs the payment
/// asset's own `transfer` event. A list that does not fit fails whole in
/// simulation and the sender splits it.
pub const MAX_HOLDERS: u32 = 50;

#[contract]
pub struct OfferHoldingsDistributor;

#[contractimpl]
impl OfferHoldingsDistributor {
    /// Sets the three powers and the two contracts. Emits `configured`.
    pub fn __constructor(e: &Env, power_holders: PowerHolders, contracts: Contracts) {
        storage::set_powers(e, &power_holders);
        storage::set_contracts(e, &contracts);
        upgradeable::set_schema_version(e, SCHEMA_VERSION);
        storage::extend_instance_ttl(e);
        events::Configured {
            owner: power_holders.owner,
            pauser: power_holders.pauser,
            upgrader: power_holders.upgrader,
            token: contracts.token,
            payment_asset: contracts.payment_asset,
        }
        .publish(e);
    }

    /// Takes `amount` of the payment asset from the owner's account and
    /// divides it among the quotas that exist now. Owner only (6400). The
    /// amount is positive (6405), the token has a supply (6406) and the
    /// per-quota counter can take the amount (6407).
    ///
    /// The owner's authorization must cover this call and the one
    /// `transfer` the asset makes in its name. No holder is touched; each
    /// one's share waits for a claim. Emits `deposited`.
    #[when_not_paused]
    pub fn deposit(e: &Env, amount: i128, caller: Address) {
        require_owner(e, &caller);
        if amount <= 0 {
            panic_with_error!(e, OfferHoldingsDistributorError::NonPositiveAmount);
        }
        let contracts = storage::contracts(e);
        let supply = QuotaTokenClient::new(e, &contracts.token).total_supply();
        if supply <= 0 {
            panic_with_error!(e, OfferHoldingsDistributorError::NoSupply);
        }
        // Rounds down: what the division leaves stays in the distributor.
        let per_quota = amount
            .checked_mul(MAGNITUDE)
            .map(|scaled| scaled / supply)
            .and_then(|increase| storage::per_quota(e).checked_add(increase));
        let Some(per_quota) = per_quota else {
            panic_with_error!(e, OfferHoldingsDistributorError::AmountOverflow);
        };
        storage::set_per_quota(e, per_quota);
        token::TokenClient::new(e, &contracts.payment_asset).transfer(
            &caller,
            e.current_contract_address(),
            &amount,
        );
        storage::extend_instance_ttl(e);
        events::Deposited { amount }.publish(e);
    }

    /// Pays each listed holder what its quotas have earned and it has not
    /// received yet. Anyone may call; nobody signs, and the money goes to
    /// the holder. The list holds 1 to `MAX_HOLDERS` holders (6403, 6404).
    ///
    /// A holder with nothing to receive is passed over. A holder on the
    /// token's whitelist, or one whose payment the asset refuses, is passed
    /// over with `claim_skipped`: its share stays here and its record does
    /// not change, so one holder never fails the call. So is a holder
    /// whose share does not fit an `i128`. Emits `claimed` once, after the
    /// payments.
    #[when_not_paused]
    pub fn claim(e: &Env, holders: Vec<Address>) {
        if holders.is_empty() {
            panic_with_error!(e, OfferHoldingsDistributorError::EmptyBatch);
        }
        if holders.len() > MAX_HOLDERS {
            panic_with_error!(e, OfferHoldingsDistributorError::BatchTooLarge);
        }
        let contracts = storage::contracts(e);
        let quota_token = QuotaTokenClient::new(e, &contracts.token);
        let payment = token::TokenClient::new(e, &contracts.payment_asset);
        let me = e.current_contract_address();
        let per_quota = storage::per_quota(e);
        let mut count = 0u32;
        let mut total = 0i128;
        for holder in holders.iter() {
            let Some((earned, owed)) = share(e, &quota_token, &holder, per_quota) else {
                events::ClaimSkipped { holder }.publish(e);
                continue;
            };
            if owed <= 0 {
                continue;
            }
            // The payment asset is the one fixed at construction, and the
            // host refuses a call back into a contract that is running, so
            // paying before recording cannot be turned into a second
            // payment. A refused payment undoes only itself.
            if quota_token.is_transfer_whitelisted(&holder)
                || !matches!(payment.try_transfer(&me, &holder, &owed), Ok(Ok(())))
            {
                events::ClaimSkipped { holder }.publish(e);
                continue;
            }
            // Paid so far and what was just paid add up to what the
            // holder's quotas have earned.
            storage::set_credited(e, &holder, earned);
            count += 1;
            // Every `owed` counted here left the distributor's balance,
            // itself an `i128`, so the sum fits.
            total += owed;
        }
        storage::extend_instance_ttl(e);
        events::Claimed { count, total }.publish(e);
    }

    /// Sends `amount` of `asset` out of the distributor's balance to the
    /// owner. Owner only (6400). The amount is positive (6405); more than
    /// the distributor holds fails with the asset's own code. `asset` is
    /// free, so a token sent here by mistake can leave too. On the payment
    /// asset this can take money holders have not claimed yet; their
    /// records do not change, so later claims are skipped until the money is
    /// back.
    /// Works while paused. Emits `withdrawn`.
    pub fn withdraw(e: &Env, asset: Address, amount: i128, caller: Address) {
        require_owner(e, &caller);
        if amount <= 0 {
            panic_with_error!(e, OfferHoldingsDistributorError::NonPositiveAmount);
        }
        // The distributor is the direct caller of this `transfer`, which is
        // all the authorization it gives: the asset cannot spend another
        // asset's balance in the distributor's name.
        token::TokenClient::new(e, &asset).transfer(
            &e.current_contract_address(),
            &caller,
            &amount,
        );
        storage::extend_instance_ttl(e);
        events::Withdrawn { asset, amount }.publish(e);
    }

    /// What a claim would try to pay `holder` now: zero for an account on
    /// the token's whitelist. Fails (6407) for a share that does not fit
    /// an `i128`, which a claim skips.
    pub fn claimable(e: &Env, holder: Address) -> i128 {
        let quota_token = QuotaTokenClient::new(e, &storage::contracts(e).token);
        let Some((_, owed)) = share(e, &quota_token, &holder, storage::per_quota(e)) else {
            panic_with_error!(e, OfferHoldingsDistributorError::AmountOverflow);
        };
        if owed <= 0 || quota_token.is_transfer_whitelisted(&holder) {
            return 0;
        }
        owed
    }

    /// What `holder` was paid so far.
    pub fn credited(e: &Env, holder: Address) -> i128 {
        storage::credited(e, &holder)
    }

    /// What one quota has earned from every deposit so far, times
    /// `MAGNITUDE`.
    pub fn cumulative_per_quota(e: &Env) -> i128 {
        storage::per_quota(e)
    }

    pub fn contracts(e: &Env) -> Contracts {
        storage::contracts(e)
    }

    pub fn powers(e: &Env) -> PowerHolders {
        storage::powers(e)
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

/// What the quotas `holder` has now earned from every deposit, rounded
/// down, and how much of it is still to be paid, which is negative when
/// the holder was paid for quotas it no longer has. Nothing when either
/// does not fit an `i128`.
fn share(
    e: &Env,
    quota_token: &QuotaTokenClient,
    holder: &Address,
    per_quota: i128,
) -> Option<(i128, i128)> {
    let earned = quota_token.balance(holder).checked_mul(per_quota)? / MAGNITUDE;
    let owed = earned.checked_sub(storage::credited(e, holder))?;
    Some((earned, owed))
}

fn require_owner(e: &Env, caller: &Address) {
    storage::require_power(
        e,
        caller,
        &[storage::powers(e).owner],
        OfferHoldingsDistributorError::NotOwner,
    );
}

#[contractimpl]
impl Pausable for OfferHoldingsDistributor {
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
            OfferHoldingsDistributorError::NotPauser,
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
impl Upgradeable for OfferHoldingsDistributor {
    /// Replaces the code once this call succeeds. `operator` must be the
    /// stored `upgrader` and sign. The constructor does not run again;
    /// storage is kept as is. No `migrate` in v1.
    fn upgrade(e: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        storage::require_power(
            e,
            &operator,
            &[storage::powers(e).upgrader],
            OfferHoldingsDistributorError::NotUpgrader,
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
