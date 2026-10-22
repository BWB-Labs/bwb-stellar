//! Tests drive the sale through its generated client, as the backend and
//! investors would, beside the real allowlist and token and a Stellar Asset
//! Contract standing in for USDC, with the ledger time pinned per test. The
//! `ttl` module reaches storage through `as_contract` to read lifetimes.
//! Modules follow the D2 vocabulary so the evidence index can cite them
//! directly.
#![cfg(test)]
extern crate std;

mod authorization;
mod boundary;
mod cooling_off;
mod events;
mod failure;
mod idempotency;
mod pause;
mod state;
mod ttl;
mod upgrade;

use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, InvokeError, String, Symbol, Vec,
};

use offer_allowlist::{AllocationEntry, OfferAllowlist, OfferAllowlistClient};
use offer_token::{OfferToken, OfferTokenClient, OfferingMetadata, PowerHolders as TokenPowers};

use crate::{Contracts, OfferSale, OfferSaleClient, Payout, PowerHolders, SaleConfig, SaleState};

/// Tokens for sale.
pub const SUPPLY: i128 = 1_000;
/// 10 USDC per token, in stroops.
pub const PRICE: i128 = 10_0000000;
pub const COOLING_OFF: u64 = 5 * 86_400;
pub const MIN_PERCENT: u32 = 75;
/// 2026-10-15 00:00 UTC: mid-month, so a purchase and its refund fall in
/// the same allowlist period unless a test moves the clock on purpose.
pub const T0: u64 = 1_792_108_800;

/// One deployed offering: the sale beside the real allowlist and token and
/// a SAC as the payment asset, every party a distinct account. Auths are
/// mocked; authorization tests replace the mocks per call.
pub struct Setup<'a> {
    pub e: Env,
    pub id: Address,
    pub client: OfferSaleClient<'a>,
    pub token: OfferTokenClient<'a>,
    pub allowlist: OfferAllowlistClient<'a>,
    pub usdc: TokenClient<'a>,
    pub usdc_admin: StellarAssetClient<'a>,
    /// The tokenizer's treasury.
    pub owner: Address,
    /// The platform multisig: pauser and upgrader of the sale, admin of the
    /// allowlist.
    pub platform: Address,
    /// The platform hot key: the allowlist's allocator.
    pub hot_key: Address,
    /// The token's admin and xfer_admin.
    pub token_admin: Address,
    pub alice: Address,
    pub bob: Address,
}

/// The production order (interface 5.1): the token mints to the sale's
/// address before the sale exists, so the address is fixed first and the
/// sale is registered at it afterwards. Left in Preparing.
pub fn setup_preparing<'a>() -> Setup<'a> {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_timestamp(T0);
    let owner = Address::generate(&e);
    let platform = Address::generate(&e);
    let hot_key = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let issuer = Address::generate(&e);
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    let usdc_id = e.register_stellar_asset_contract_v2(issuer).address();
    let allowlist_id = e.register(
        OfferAllowlist,
        (platform.clone(), vec![&e, hot_key.clone()]),
    );
    let sale_id = Address::generate(&e);
    let token_id = e.register(
        OfferToken,
        (
            TokenPowers {
                admin: token_admin.clone(),
                xfer_admin: token_admin.clone(),
                pauser: platform.clone(),
                upgrader: platform.clone(),
            },
            OfferingMetadata {
                name: String::from_str(&e, "Edificio Aurora"),
                symbol: String::from_str(&e, "AUR"),
                offer_uri: String::from_str(&e, "ipfs://offering-v1"),
            },
            SUPPLY,
            sale_id.clone(),
            Vec::<Address>::new(&e),
        ),
    );
    e.register_at(
        &sale_id,
        OfferSale,
        (
            PowerHolders {
                owner: owner.clone(),
                pauser: platform.clone(),
                upgrader: platform.clone(),
            },
            Contracts {
                token: token_id.clone(),
                payment_asset: usdc_id.clone(),
                allowlist: allowlist_id.clone(),
            },
            config(),
        ),
    );
    let allowlist = OfferAllowlistClient::new(&e, &allowlist_id);
    allowlist.grant_role(&sale_id, &Symbol::new(&e, "sale"), &hot_key);
    Setup {
        client: OfferSaleClient::new(&e, &sale_id),
        token: OfferTokenClient::new(&e, &token_id),
        allowlist,
        usdc: TokenClient::new(&e, &usdc_id),
        usdc_admin: StellarAssetClient::new(&e, &usdc_id),
        id: sale_id,
        e,
        owner,
        platform,
        hot_key,
        token_admin,
        alice,
        bob,
    }
}

/// Activated, with alice and bob allowed and funded for the whole supply.
pub fn setup<'a>() -> Setup<'a> {
    let s = setup_preparing();
    s.client.activate();
    for who in [&s.alice, &s.bob] {
        s.allow(who, SUPPLY * PRICE);
        s.fund(who, SUPPLY * PRICE);
    }
    s
}

pub fn config() -> SaleConfig {
    SaleConfig {
        price: PRICE,
        cooling_off_secs: COOLING_OFF,
        min_success_percent: MIN_PERCENT,
    }
}

impl Setup<'_> {
    /// Writes an enabled allocation of `max` for `investor` as the hot key.
    pub fn allow(&self, investor: &Address, max: i128) {
        self.allowlist.set_allocations(
            &vec![
                &self.e,
                AllocationEntry {
                    investor: investor.clone(),
                    max,
                    enabled: true,
                },
            ],
            &self.hot_key,
        );
    }

    /// Mints payment asset to an account.
    pub fn fund(&self, who: &Address, amount: i128) {
        self.usdc_admin.mint(who, &amount);
    }

    /// Buys `tokens` tokens' worth and returns the allowlist receipt number
    /// of the purchase (read back from the reservation).
    pub fn buy(&self, who: &Address, tokens: i128) -> u32 {
        self.client.buy(who, &(tokens * PRICE));
        let reservation = self.client.reservation(who).unwrap();
        reservation.purchases.keys().last().unwrap()
    }

    pub fn try_buy(&self, who: &Address, tokens: i128) -> TryResult<()> {
        self.client.try_buy(who, &(tokens * PRICE))
    }

    /// Moves the ledger time forward.
    pub fn advance(&self, secs: u64) {
        self.e.ledger().with_mut(|l| l.timestamp += secs);
    }

    /// Past every cooling-off window that could be open.
    pub fn past_cooling_off(&self) {
        self.advance(COOLING_OFF + 1);
    }

    /// Alice buys the minimum, the owner closes, the windows run out and
    /// the offering is finalized Successful.
    pub fn successful(&self) {
        self.buy(&self.alice, 750);
        self.client.close(&self.owner);
        self.past_cooling_off();
        self.client.finalize();
        assert_eq!(self.client.state(), SaleState::Successful);
    }

    /// Alice buys, the owner cancels: Failed with one reservation open.
    pub fn failed(&self) {
        self.buy(&self.alice, 100);
        self.client.cancel(&self.owner);
        assert_eq!(self.client.state(), SaleState::Failed);
    }

    pub fn investors(&self, who: &[&Address]) -> Vec<Address> {
        let mut v = Vec::new(&self.e);
        for a in who {
            v.push_back((*a).clone());
        }
        v
    }

    pub fn payouts(&self, legs: &[(&Address, i128)]) -> Vec<Payout> {
        let mut v = Vec::new(&self.e);
        for (recipient, amount) in legs {
            v.push_back(Payout {
                recipient: (*recipient).clone(),
                amount: *amount,
            });
        }
        v
    }
}

/// A payment asset that keeps 1 stroop of every transfer: a SAC never
/// does, so the measurement in `buy` is tested against a contract that
/// does. Only what the sale calls is implemented.
#[contract]
pub struct FeeToken;

#[contracttype]
pub enum FeeKey {
    Balance(Address),
}

fn fee_balance(e: &Env, who: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&FeeKey::Balance(who.clone()))
        .unwrap_or(0)
}

fn set_fee_balance(e: &Env, who: &Address, amount: i128) {
    e.storage()
        .persistent()
        .set(&FeeKey::Balance(who.clone()), &amount);
}

#[contractimpl]
impl FeeToken {
    pub fn mint(e: &Env, to: Address, amount: i128) {
        set_fee_balance(e, &to, fee_balance(e, &to) + amount);
    }

    pub fn balance(e: &Env, id: Address) -> i128 {
        fee_balance(e, &id)
    }

    pub fn transfer(e: &Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        set_fee_balance(e, &from, fee_balance(e, &from) - amount);
        set_fee_balance(e, &to, fee_balance(e, &to) + amount - 1);
    }
}

/// An offering paid in `FeeToken`, activated, alice allowed and funded.
pub fn setup_with_fee_token<'a>() -> (Setup<'a>, FeeTokenClient<'a>) {
    let s = setup_preparing();
    let fee_id = s.e.register(FeeToken, ());
    let fee = FeeTokenClient::new(&s.e, &fee_id);
    // The sale's payment asset is fixed at construction, so the sale is
    // re-registered at the same address with the fee token in its place.
    let contracts = s.client.contracts();
    s.e.register_at(
        &s.id,
        OfferSale,
        (
            s.client.powers(),
            Contracts {
                token: contracts.token,
                payment_asset: fee_id.clone(),
                allowlist: contracts.allowlist,
            },
            config(),
        ),
    );
    s.client.activate();
    s.allow(&s.alice, SUPPLY * PRICE);
    fee.mint(&s.alice, &(SUPPLY * PRICE));
    (s, fee)
}

/// What a client's `try_` call returns.
pub type TryResult<T> =
    Result<Result<T, soroban_sdk::ConversionError>, Result<soroban_sdk::Error, InvokeError>>;

/// The contract error a failed call carries.
pub fn code(n: u32) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(n)
}

/// Unwraps the error side of a `try_` call into the host error it carried.
pub fn err<T, C: core::fmt::Debug>(
    r: Result<Result<T, C>, Result<soroban_sdk::Error, InvokeError>>,
) -> soroban_sdk::Error {
    match r {
        Err(Ok(e)) => e,
        Err(Err(InvokeError::Contract(n))) => soroban_sdk::Error::from_contract_error(n),
        Err(Err(InvokeError::Abort)) => panic!("call aborted without an error code"),
        Ok(_) => panic!("call succeeded but was expected to fail"),
    }
}
