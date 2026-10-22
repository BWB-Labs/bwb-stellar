//! Offer sale: the escrow and lifecycle of one offering. One instance per
//! offering. Soroban port of Base's `OfferTokenSale`, with the moment of
//! truth moved.
//!
//! The contract holds the offering's whole token supply (minted to it by
//! the token) and the investors' money. An investor buys with the payment
//! asset and gets a reservation: tokens held for them, kept purchase by
//! purchase under the allowlist's receipt number, each purchase with its
//! own cooling-off window. Buying stops when the owner closes or the last
//! token sells; the offering then waits in Closing until every window has
//! run out, and anyone finalizes it: Successful if the minimum held, Failed
//! otherwise. So nothing can be undone after success: release is immediate
//! and open to anyone, the owner withdraws freely, and the unsold tokens
//! are burned. After failure everyone is refunded in batches and the owner
//! takes the tokens back. A refund during Closing that drops the offering
//! below its minimum leaves the owner a choice: reopen, cancel, or let it
//! fail.
//!
//! Three powers, each a stored address fixed at construction with no
//! setter, as on the token: `owner` (the tokenizer's treasury) closes,
//! reopens, cancels, withdraws, pauses and unpauses; `pauser` (the platform
//! multisig) pauses, never unpauses; `upgrader` (the platform multisig)
//! replaces the code. A paused sale blocks what moves money toward the
//! tokenizer (`buy`, `release`, the withdrawals), never the investor's
//! exit (`cooling_off_refund`, `refund`) nor the decision (`finalize`,
//! `cancel`).
#![no_std]

mod allowlist;
mod errors;
mod events;
mod offer_token;
mod storage;
mod types;

use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, token, Address, BytesN, Env, Map,
    Symbol, Vec,
};
use stellar_contract_utils::{
    pausable::{self, Pausable},
    upgradeable::{self, Upgradeable},
};
use stellar_macros::when_not_paused;

use crate::allowlist::AllowlistClient;
pub use crate::errors::OfferSaleError;
use crate::events::{
    Activated, Cancelled, Closed, Configured, Finalized, FinalizedFailed, PauseChanged,
    PaymentWithdrawn, Purchased, Refunded, Released, Reopened, StateChanged, TokensWithdrawn,
    Upgraded,
};
use crate::offer_token::OfferTokenSupplyClient;
pub use crate::types::{
    Contracts, Payout, PowerHolders, Purchase, Reservation, SaleConfig, SaleState, Totals,
};

// Contract metadata that goes on the compiled file.
contractmeta!(key = "binver", val = env!("CARGO_PKG_VERSION"));

/// Version of the storage layout this code writes and reads.
const SCHEMA_VERSION: u32 = 1;

/// The allowlist role the sale contract must hold to consume room.
const SALE_ROLE: &str = "sale";

/// Shortest cooling-off window the constructor accepts: one day. Base's
/// factory enforced the same; its offerings use 5 days.
pub const MIN_COOLING_OFF_SECS: u64 = 86_400;

/// Most live purchases one investor may hold in one offering. A failed
/// offering refunds the investor whole, one `allocation_restored` (292 B)
/// per purchase, so this keeps one investor's refund far inside a
/// transaction's 16 KB of events, and it equals the allowlist's `restore`
/// limit, so a cooling-off refund never needs more than one call there.
pub const MAX_PURCHASES: u32 = 20;

/// Most investors in one `release`: 30 × (token `transfer` 168 B +
/// `released` 188 B) = 10,680 B of events, 62 write entries.
pub const MAX_RELEASE: u32 = 30;

/// Most investors in one `refund`, with `REFUND_UNITS` as the finer bound.
pub const MAX_REFUND: u32 = 16;

/// The event budget of one `refund`, in units of one `allocation_restored`
/// (292 B, the largest event in the call): an investor costs 2 (payment
/// `transfer` 244 B + `refunded` 296 B ≤ 2 × 292) and each live purchase
/// 1. At 48 units the call emits at most 14,016 B, 86% of the limit.
pub const REFUND_UNITS: u32 = 48;
const UNITS_PER_INVESTOR: u32 = 2;

/// Most legs in one `withdraw_payment`: 30 × (payment `transfer` 244 B +
/// `payment_withdrawn` 168 B) = 12,360 B of events.
pub const MAX_PAYOUTS: u32 = 30;

#[contract]
pub struct OfferSale;

#[contractimpl]
impl OfferSale {
    /// Sets the three powers, the contracts and the terms. `price` must be
    /// positive, `cooling_off_secs` at least one day and
    /// `min_success_percent` between 10 and 100 (6216). Starts in
    /// Preparing. Emits `configured`.
    pub fn __constructor(
        e: &Env,
        power_holders: PowerHolders,
        contracts: Contracts,
        config: SaleConfig,
    ) {
        if config.price <= 0
            || config.cooling_off_secs < MIN_COOLING_OFF_SECS
            || !(10..=100).contains(&config.min_success_percent)
        {
            panic_with_error!(e, OfferSaleError::InvalidConfig);
        }
        storage::set_powers(e, &power_holders);
        storage::set_contracts(e, &contracts);
        storage::set_config(e, &config);
        storage::set_state(e, SaleState::Preparing);
        storage::set_totals(
            e,
            &Totals {
                supply: 0,
                sold_tokens: 0,
                raised: 0,
                released_tokens: 0,
                withdrawn: 0,
            },
        );
        upgradeable::set_schema_version(e, SCHEMA_VERSION);
        storage::extend_instance_ttl(e);
        Configured {
            owner: power_holders.owner,
            pauser: power_holders.pauser,
            upgrader: power_holders.upgrader,
            token: contracts.token,
            payment_asset: contracts.payment_asset,
            allowlist: contracts.allowlist,
            price: config.price,
            cooling_off_secs: config.cooling_off_secs,
            min_success_percent: config.min_success_percent,
        }
        .publish(e);
    }

    /// Opens buying. Anyone may call it: it only checks facts. The sale
    /// contract must hold the token's whole supply (6204) and `sale` on the
    /// allowlist (6219), so it runs after the token's deploy and the role
    /// grant. The supply is kept as the tokens for sale. Emits
    /// `state_changed`, then `activated`.
    pub fn activate(e: &Env) {
        assert_state(e, SaleState::Preparing, OfferSaleError::NotPreparing);
        let contracts = storage::contracts(e);
        let me = e.current_contract_address();
        let token = token::TokenClient::new(e, &contracts.token);
        let supply = OfferTokenSupplyClient::new(e, &contracts.token).total_supply();
        if supply <= 0 || token.balance(&me) != supply {
            panic_with_error!(e, OfferSaleError::InventoryMissing);
        }
        let allowlist = AllowlistClient::new(e, &contracts.allowlist);
        if allowlist
            .has_role(&me, &Symbol::new(e, SALE_ROLE))
            .is_none()
        {
            panic_with_error!(e, OfferSaleError::NotSale);
        }
        let mut totals = storage::totals(e);
        totals.supply = supply;
        storage::set_totals(e, &totals);
        transition(e, SaleState::Preparing, SaleState::Active);
        Activated { supply }.publish(e);
    }

    /// Reserves `amount / price` tokens for `investor` against `amount` of
    /// the payment asset, which the investor authorizes. `amount` must be
    /// positive (6205), the tokens must be left (6207), and the investor
    /// may hold at most `MAX_PURCHASES` live purchases here (6220). The
    /// contract measures what actually arrived, as Base does, and reserves
    /// against that: it must be positive (6209) and an exact multiple of
    /// the price (6206), so a payment asset that takes a fee can only buy
    /// whole tokens by sending the fee on top. Then it uses that much
    /// of the investor's allowlist room (6100, 6101, 6107 surface) and
    /// keeps the purchase under the receipt number. The purchase that sells
    /// the last token closes buying. Emits `purchased`, after the
    /// allowlist's `allocation_consumed` and the payment's `transfer`.
    #[when_not_paused]
    pub fn buy(e: &Env, investor: Address, amount: i128) {
        investor.require_auth();
        assert_state(e, SaleState::Active, OfferSaleError::NotActive);
        let config = storage::config(e);
        if amount <= 0 {
            panic_with_error!(e, OfferSaleError::NonPositiveAmount);
        }
        let mut totals = storage::totals(e);
        if amount / config.price > totals.supply - totals.sold_tokens {
            panic_with_error!(e, OfferSaleError::ExceedsSupply);
        }
        let mut reservation = storage::reservation(e, &investor).unwrap_or(Reservation {
            schema: storage::RESERVATION_SCHEMA,
            tokens: 0,
            paid: 0,
            purchases: Map::new(e),
        });
        if reservation.purchases.len() >= MAX_PURCHASES {
            panic_with_error!(e, OfferSaleError::TooManyPurchases);
        }

        let contracts = storage::contracts(e);
        let me = e.current_contract_address();
        let payment = token::TokenClient::new(e, &contracts.payment_asset);
        let before = payment.balance(&me);
        payment.transfer(&investor, &me, &amount);
        let received = payment.balance(&me) - before;
        if received <= 0 {
            panic_with_error!(e, OfferSaleError::NothingReceived);
        }
        if received % config.price != 0 {
            panic_with_error!(e, OfferSaleError::NotWholeTokens);
        }
        let tokens = received / config.price;

        let receipt =
            AllowlistClient::new(e, &contracts.allowlist).consume(&investor, &received, &me);
        let at = e.ledger().timestamp();
        reservation.purchases.set(
            receipt,
            Purchase {
                tokens,
                paid: received,
                at,
            },
        );
        reservation.tokens += tokens;
        reservation.paid += received;
        storage::set_reservation(e, &investor, &reservation);
        totals.sold_tokens += tokens;
        totals.raised += received;
        storage::set_totals(e, &totals);
        storage::set_last_purchase_at(e, at);
        storage::extend_instance_ttl(e);
        Purchased {
            investor,
            paid: received,
            tokens,
            receipt,
            at,
            reserved_paid: reservation.paid,
            reserved_tokens: reservation.tokens,
            raised: totals.raised,
            sold_tokens: totals.sold_tokens,
        }
        .publish(e);
        if totals.sold_tokens == totals.supply {
            close_with(e, &totals, "sold_out");
        }
    }

    /// Stops buying: the offering waits in Closing for the last cooling-off
    /// window. Owner only. Emits `closed` and `state_changed`.
    pub fn close(e: &Env, caller: Address) {
        require_owner(e, &caller);
        assert_state(e, SaleState::Active, OfferSaleError::NotActive);
        close_with(e, &storage::totals(e), "owner");
    }

    /// Opens buying again from Closing, so a refund that dropped the
    /// offering below its minimum need not fail it. Owner only. Emits
    /// `state_changed`, then `reopened`.
    pub fn reopen(e: &Env, caller: Address) {
        require_owner(e, &caller);
        assert_state(e, SaleState::Closing, OfferSaleError::NotClosing);
        let totals = storage::totals(e);
        transition(e, SaleState::Closing, SaleState::Active);
        Reopened {
            raised: totals.raised,
            sold_tokens: totals.sold_tokens,
        }
        .publish(e);
    }

    /// Fails the offering at the owner's decision, while Active or Closing
    /// (6225 once decided): everyone is then refunded. Emits
    /// `state_changed`, then `cancelled`.
    pub fn cancel(e: &Env, caller: Address) {
        require_owner(e, &caller);
        let from = require_open(e);
        let totals = storage::totals(e);
        transition(e, from, SaleState::Failed);
        Cancelled {
            raised: totals.raised,
            sold_tokens: totals.sold_tokens,
        }
        .publish(e);
    }

    /// Decides the offering once no purchase can be undone: in Closing
    /// (6221), after the latest purchase's window has run out (6214).
    /// Anyone may call it. Successful when `sold × 100 ≥ supply × min`:
    /// the contract must still hold every sold token (6217; `xfer_admin`
    /// can move them) and burns the rest. Failed otherwise. Emits
    /// `finalized` or `finalized_failed`, then `state_changed`.
    pub fn finalize(e: &Env) {
        assert_state(e, SaleState::Closing, OfferSaleError::NotClosing);
        let config = storage::config(e);
        let last = storage::last_purchase_at(e);
        if e.ledger().timestamp() <= last + config.cooling_off_secs {
            panic_with_error!(e, OfferSaleError::CoolingOffOpen);
        }
        let totals = storage::totals(e);
        let met =
            totals.sold_tokens * 100 >= totals.supply * i128::from(config.min_success_percent);
        if !met {
            FinalizedFailed {
                raised: totals.raised,
                sold_tokens: totals.sold_tokens,
                min_success_percent: config.min_success_percent,
            }
            .publish(e);
            transition(e, SaleState::Closing, SaleState::Failed);
            return;
        }
        let me = e.current_contract_address();
        let token = token::TokenClient::new(e, &storage::contracts(e).token);
        let held = token.balance(&me);
        if held < totals.sold_tokens {
            panic_with_error!(e, OfferSaleError::TokensMissing);
        }
        let burned = held - totals.sold_tokens;
        if burned > 0 {
            token.burn(&me, &burned);
        }
        Finalized {
            raised: totals.raised,
            sold_tokens: totals.sold_tokens,
            burned,
        }
        .publish(e);
        transition(e, SaleState::Closing, SaleState::Successful);
    }

    /// Gives the investor back every purchase still inside its cooling-off
    /// window, whole purchases only, while Active or Closing (6225). No
    /// reservation (6213) or no purchase still open (6212) fails. The
    /// receipts go back to the allowlist in one call, the payment goes
    /// back, and the totals fall; the reservation ends when nothing is
    /// left. The investor authorizes. Emits `refunded` with reason
    /// `cooling_off`, after the allowlist's `allocation_restored` events
    /// and the payment's `transfer`.
    pub fn cooling_off_refund(e: &Env, investor: Address) {
        require_open(e);
        investor.require_auth();
        let Some(mut reservation) = storage::reservation(e, &investor) else {
            panic_with_error!(e, OfferSaleError::NothingReserved);
        };
        let window = storage::config(e).cooling_off_secs;
        let now = e.ledger().timestamp();
        let mut receipts: Vec<u32> = Vec::new(e);
        let (mut tokens, mut paid) = (0i128, 0i128);
        for number in reservation.purchases.keys().iter() {
            let purchase = reservation.purchases.get_unchecked(number);
            if now <= purchase.at + window {
                receipts.push_back(number);
                tokens += purchase.tokens;
                paid += purchase.paid;
            }
        }
        if receipts.is_empty() {
            panic_with_error!(e, OfferSaleError::CoolingOffClosed);
        }
        for number in receipts.iter() {
            reservation.purchases.remove(number);
        }
        reservation.tokens -= tokens;
        reservation.paid -= paid;
        let contracts = storage::contracts(e);
        let me = e.current_contract_address();
        AllowlistClient::new(e, &contracts.allowlist).restore(&investor, &receipts, &me);
        token::TokenClient::new(e, &contracts.payment_asset).transfer(&me, &investor, &paid);
        storage::set_reservation(e, &investor, &reservation);
        let totals = lower_totals(e, tokens, paid);
        storage::extend_instance_ttl(e);
        Refunded {
            investor,
            paid,
            tokens,
            reason: Symbol::new(e, "cooling_off"),
            raised: totals.raised,
            sold_tokens: totals.sold_tokens,
        }
        .publish(e);
    }

    /// Delivers the reserved tokens of each investor listed, at most
    /// `MAX_RELEASE` (6215), in Successful (6202). Anyone may call it: the
    /// reservation says who gets what. An investor with nothing reserved,
    /// or listed twice, is skipped. Emits `released` per investor served,
    /// after the token's `transfer`.
    #[when_not_paused]
    pub fn release(e: &Env, investors: Vec<Address>) {
        assert_state(e, SaleState::Successful, OfferSaleError::NotSuccessful);
        if investors.len() > MAX_RELEASE {
            panic_with_error!(e, OfferSaleError::BatchTooLarge);
        }
        let token = token::TokenClient::new(e, &storage::contracts(e).token);
        let me = e.current_contract_address();
        let mut totals = storage::totals(e);
        for investor in investors.iter() {
            let Some(reservation) = storage::reservation(e, &investor) else {
                continue;
            };
            token.transfer(&me, &investor, &reservation.tokens);
            storage::remove_reservation(e, &investor);
            totals.released_tokens += reservation.tokens;
            Released {
                investor,
                paid: reservation.paid,
                tokens: reservation.tokens,
            }
            .publish(e);
        }
        storage::set_totals(e, &totals);
        storage::extend_instance_ttl(e);
    }

    /// Returns the payment of each investor listed and gives all their
    /// receipts back to the allowlist, in Failed (6203). Anyone may call
    /// it. At most `MAX_REFUND` investors and `REFUND_UNITS` event units
    /// (2 per investor served plus 1 per live purchase), both checked
    /// before any transfer (6215). An investor with nothing reserved is
    /// skipped. A payment transfer that fails (a frozen balance, a dropped
    /// trustline) fails the whole call. Emits `refunded` with reason
    /// `failed` per investor served, after the allowlist's
    /// `allocation_restored` events and the payment's `transfer`.
    pub fn refund(e: &Env, investors: Vec<Address>) {
        assert_state(e, SaleState::Failed, OfferSaleError::NotFailed);
        if investors.len() > MAX_REFUND {
            panic_with_error!(e, OfferSaleError::BatchTooLarge);
        }
        let mut units = 0u32;
        for investor in investors.iter() {
            if let Some(reservation) = storage::reservation(e, &investor) {
                units += UNITS_PER_INVESTOR + reservation.purchases.len();
            }
        }
        if units > REFUND_UNITS {
            panic_with_error!(e, OfferSaleError::BatchTooLarge);
        }
        let contracts = storage::contracts(e);
        let me = e.current_contract_address();
        let allowlist = AllowlistClient::new(e, &contracts.allowlist);
        let payment = token::TokenClient::new(e, &contracts.payment_asset);
        let mut totals = storage::totals(e);
        for investor in investors.iter() {
            let Some(reservation) = storage::reservation(e, &investor) else {
                continue;
            };
            allowlist.restore_all(&investor, &me);
            payment.transfer(&me, &investor, &reservation.paid);
            storage::remove_reservation(e, &investor);
            totals.sold_tokens -= reservation.tokens;
            totals.raised -= reservation.paid;
            Refunded {
                investor,
                paid: reservation.paid,
                tokens: reservation.tokens,
                reason: Symbol::new(e, "failed"),
                raised: totals.raised,
                sold_tokens: totals.sold_tokens,
            }
            .publish(e);
        }
        storage::set_totals(e, &totals);
        storage::extend_instance_ttl(e);
    }

    /// Pays every `{recipient, amount}` listed, at most `MAX_PAYOUTS`
    /// (6215), in Successful (6202). Owner only. Each amount must be
    /// positive (6205) and the total at most `raised − withdrawn` (6218).
    /// Emits `payment_withdrawn` per leg, after each payment `transfer`.
    #[when_not_paused]
    pub fn withdraw_payment(e: &Env, payouts: Vec<Payout>, caller: Address) {
        require_owner(e, &caller);
        assert_state(e, SaleState::Successful, OfferSaleError::NotSuccessful);
        if payouts.len() > MAX_PAYOUTS {
            panic_with_error!(e, OfferSaleError::BatchTooLarge);
        }
        let mut total = 0i128;
        for payout in payouts.iter() {
            if payout.amount <= 0 {
                panic_with_error!(e, OfferSaleError::NonPositiveAmount);
            }
            total += payout.amount;
        }
        let mut totals = storage::totals(e);
        if total > totals.raised - totals.withdrawn {
            panic_with_error!(e, OfferSaleError::ExceedsWithdrawable);
        }
        let payment = token::TokenClient::new(e, &storage::contracts(e).payment_asset);
        let me = e.current_contract_address();
        for payout in payouts.iter() {
            payment.transfer(&me, &payout.recipient, &payout.amount);
            PaymentWithdrawn {
                recipient: payout.recipient,
                amount: payout.amount,
            }
            .publish(e);
        }
        totals.withdrawn += total;
        storage::set_totals(e, &totals);
        storage::extend_instance_ttl(e);
    }

    /// Sends `amount` of the offering's tokens to `to`, in Failed (6203).
    /// Owner only; the amount must be positive (6205). Emits
    /// `tokens_withdrawn` after the token's `transfer`.
    #[when_not_paused]
    pub fn withdraw_tokens(e: &Env, to: Address, amount: i128, caller: Address) {
        require_owner(e, &caller);
        assert_state(e, SaleState::Failed, OfferSaleError::NotFailed);
        if amount <= 0 {
            panic_with_error!(e, OfferSaleError::NonPositiveAmount);
        }
        let token = token::TokenClient::new(e, &storage::contracts(e).token);
        token.transfer(&e.current_contract_address(), &to, &amount);
        storage::extend_instance_ttl(e);
        TokensWithdrawn { to, amount }.publish(e);
    }

    pub fn state(e: &Env) -> SaleState {
        storage::state(e)
    }

    pub fn config(e: &Env) -> SaleConfig {
        storage::config(e)
    }

    pub fn powers(e: &Env) -> PowerHolders {
        storage::powers(e)
    }

    pub fn contracts(e: &Env) -> Contracts {
        storage::contracts(e)
    }

    pub fn totals(e: &Env) -> Totals {
        storage::totals(e)
    }

    /// The investor's reservation, or void.
    pub fn reservation(e: &Env, investor: Address) -> Option<Reservation> {
        storage::reservation(e, &investor)
    }

    /// The ledger time from which `finalize` passes its window check: one
    /// second after the latest purchase's window. 0 before any purchase.
    pub fn finalizable_at(e: &Env) -> u64 {
        let last = storage::last_purchase_at(e);
        if last == 0 {
            return 0;
        }
        last + storage::config(e).cooling_off_secs + 1
    }

    /// `raised − withdrawn` in Successful, else 0.
    pub fn withdrawable(e: &Env) -> i128 {
        if storage::state(e) != SaleState::Successful {
            return 0;
        }
        let totals = storage::totals(e);
        totals.raised - totals.withdrawn
    }

    /// Tokens `amount` of the payment asset buys; fails (6206) when it is
    /// not an exact multiple of the price.
    pub fn preview_tokens(e: &Env, amount: i128) -> i128 {
        let price = storage::config(e).price;
        if amount % price != 0 {
            panic_with_error!(e, OfferSaleError::NotWholeTokens);
        }
        amount / price
    }

    pub fn schema_version(e: &Env) -> u32 {
        upgradeable::get_schema_version(e)
    }
}

/// Fails with `err` unless the sale is in `expected`.
fn assert_state(e: &Env, expected: SaleState, err: OfferSaleError) {
    if storage::state(e) != expected {
        panic_with_error!(e, err);
    }
}

/// The current state if it is Active or Closing, where purchases can still
/// be undone; 6225 otherwise.
fn require_open(e: &Env) -> SaleState {
    match storage::state(e) {
        state @ (SaleState::Active | SaleState::Closing) => state,
        _ => panic_with_error!(e, OfferSaleError::NotOpen),
    }
}

fn require_owner(e: &Env, caller: &Address) {
    storage::require_power(
        e,
        caller,
        &[storage::powers(e).owner],
        OfferSaleError::NotOwner,
    );
}

fn state_symbol(e: &Env, state: SaleState) -> Symbol {
    Symbol::new(
        e,
        match state {
            SaleState::Preparing => "preparing",
            SaleState::Active => "active",
            SaleState::Closing => "closing",
            SaleState::Successful => "successful",
            SaleState::Failed => "failed",
        },
    )
}

/// Writes the new state, keeps the instance alive and emits
/// `state_changed`.
fn transition(e: &Env, from: SaleState, to: SaleState) {
    storage::set_state(e, to);
    storage::extend_instance_ttl(e);
    StateChanged {
        from: state_symbol(e, from),
        to: state_symbol(e, to),
    }
    .publish(e);
}

/// Active → Closing with the reason recorded.
fn close_with(e: &Env, totals: &Totals, reason: &str) {
    Closed {
        reason: Symbol::new(e, reason),
        raised: totals.raised,
        sold_tokens: totals.sold_tokens,
    }
    .publish(e);
    transition(e, SaleState::Active, SaleState::Closing);
}

/// Takes a refund out of the totals and returns them.
fn lower_totals(e: &Env, tokens: i128, paid: i128) -> Totals {
    let mut totals = storage::totals(e);
    totals.sold_tokens -= tokens;
    totals.raised -= paid;
    storage::set_totals(e, &totals);
    totals
}

#[contractimpl]
impl Pausable for OfferSale {
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
            OfferSaleError::NotPauser,
        );
        pausable::pause(e);
        storage::extend_instance_ttl(e);
        PauseChanged {
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
        PauseChanged {
            caller,
            paused: false,
        }
        .publish(e);
    }
}

#[contractimpl]
impl Upgradeable for OfferSale {
    /// Replaces the code once this call succeeds. `operator` must be the
    /// stored `upgrader` and sign. The constructor does not run again;
    /// storage is kept as is. No `migrate` in v1.
    fn upgrade(e: &Env, new_wasm_hash: BytesN<32>, operator: Address) {
        storage::require_power(
            e,
            &operator,
            &[storage::powers(e).upgrader],
            OfferSaleError::NotUpgrader,
        );
        storage::extend_instance_ttl(e);
        Upgraded {
            new_wasm_hash: new_wasm_hash.clone(),
            from_schema_version: upgradeable::get_schema_version(e),
        }
        .publish(e);
        upgradeable::upgrade(e, &new_wasm_hash);
    }
}

mod test;
