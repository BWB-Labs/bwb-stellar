//! Tests drive the distributor through its generated client, as the backend
//! would, beside the real `offer-token` and a Stellar Asset Contract
//! standing in for USDC. The whole supply starts on `desk`, an operational
//! account on the token's whitelist, which hands quotas to holders the way
//! the sale's release does. The `ttl` module reaches storage through
//! `as_contract` to read lifetimes. Modules follow the D2 vocabulary so the
//! evidence index can cite them directly.
#![cfg(test)]
extern crate std;

mod authorization;
mod claim;
mod deposit;
mod events;
mod pause;
mod skip;
mod ttl;
mod upgrade;
mod withdraw;

use std::rc::Rc;

use offer_token::{OfferToken, OfferTokenClient, OfferingMetadata, PowerHolders as TokenPowers};
use soroban_sdk::{
    testutils::{Address as _, Events as _, IssuerFlags, StellarAssetContract},
    token::{StellarAssetClient, TokenClient},
    xdr, Address, Env, InvokeError, String, TryFromVal, Vec,
};

use crate::{Contracts, OfferHoldingsDistributor, OfferHoldingsDistributorClient, PowerHolders};

/// 1 USDC, in stroops.
pub const USDC: i128 = 10_000_000;
/// What the owner holds at the start of every test.
pub const FUNDS: i128 = 1_000_000 * USDC;
/// The quotas of the offering in `setup`. A deposit of `SUPPLY` USDC is
/// 1 USDC per quota.
pub const SUPPLY: i128 = 1_000;

/// One deployed distributor beside its token and a SAC as the payment
/// asset, every party a distinct account. Auths are mocked; authorization
/// tests replace the mocks per call.
pub struct Setup<'a> {
    pub e: Env,
    pub id: Address,
    pub client: OfferHoldingsDistributorClient<'a>,
    pub token: OfferTokenClient<'a>,
    pub sac: StellarAssetContract,
    pub usdc: TokenClient<'a>,
    pub usdc_admin: StellarAssetClient<'a>,
    /// The issuer's treasury: it deposits and withdraws.
    pub owner: Address,
    /// The platform's pauser. A different account from the upgrader, so a
    /// test can tell the two powers apart.
    pub pauser: Address,
    /// The platform's upgrader.
    pub upgrader: Address,
    /// The token's admin and transfer admin.
    pub token_admin: Address,
    /// The token's initial holder: whitelisted, holds every quota not yet
    /// handed out.
    pub desk: Address,
    /// An account with no power here, funded like the owner.
    pub stranger: Address,
    pub alice: Address,
    pub bob: Address,
}

pub fn setup<'a>() -> Setup<'a> {
    setup_with_supply(SUPPLY)
}

pub fn setup_with_supply<'a>(supply: i128) -> Setup<'a> {
    let e = Env::default();
    e.mock_all_auths();
    let owner = Address::generate(&e);
    let pauser = Address::generate(&e);
    let upgrader = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let desk = Address::generate(&e);
    let stranger = Address::generate(&e);
    let issuer = Address::generate(&e);
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    // Revocable, like Circle's USDC: the issuer can freeze a balance.
    let sac = e.register_stellar_asset_contract_v2(issuer);
    sac.issuer().set_flag(IssuerFlags::RevocableFlag);
    let usdc_id = sac.address();
    let token_id = e.register(
        OfferToken,
        (
            TokenPowers {
                admin: token_admin.clone(),
                xfer_admin: token_admin.clone(),
                pauser: pauser.clone(),
                upgrader: upgrader.clone(),
            },
            OfferingMetadata {
                name: String::from_str(&e, "Edificio Aurora"),
                symbol: String::from_str(&e, "AUR"),
                offer_uri: String::from_str(&e, "ipfs://offering-v1"),
            },
            supply,
            desk.clone(),
            Vec::<Address>::new(&e),
        ),
    );
    let id = e.register(
        OfferHoldingsDistributor,
        (
            PowerHolders {
                owner: owner.clone(),
                pauser: pauser.clone(),
                upgrader: upgrader.clone(),
            },
            Contracts {
                token: token_id.clone(),
                payment_asset: usdc_id.clone(),
            },
        ),
    );
    let usdc_admin = StellarAssetClient::new(&e, &usdc_id);
    usdc_admin.mint(&owner, &FUNDS);
    usdc_admin.mint(&stranger, &FUNDS);
    Setup {
        client: OfferHoldingsDistributorClient::new(&e, &id),
        token: OfferTokenClient::new(&e, &token_id),
        usdc: TokenClient::new(&e, &usdc_id),
        usdc_admin,
        sac,
        id,
        e,
        owner,
        pauser,
        upgrader,
        token_admin,
        desk,
        stranger,
        alice,
        bob,
    }
}

impl Setup<'_> {
    /// Hands `quotas` from the desk to `holder`, as a release would.
    pub fn give(&self, holder: &Address, quotas: i128) {
        TokenClient::new(&self.e, &self.token.address).transfer(&self.desk, holder, &quotas);
    }

    /// The owner deposits `amount`.
    pub fn deposit(&self, amount: i128) {
        self.client.deposit(&amount, &self.owner);
    }

    pub fn holders(&self, list: &[&Address]) -> Vec<Address> {
        let mut v = Vec::new(&self.e);
        for holder in list {
            v.push_back((*holder).clone());
        }
        v
    }

    /// A claim for `list`, sent by nobody in particular.
    pub fn claim(&self, list: &[&Address]) {
        self.client.claim(&self.holders(list));
    }

    /// What the distributor holds in USDC.
    pub fn pot(&self) -> i128 {
        self.usdc.balance(&self.id)
    }

    /// The address of classic account number `i`. Nothing is created.
    pub fn account(&self, i: u32) -> Address {
        Address::try_from_val(&self.e, &xdr::ScAddress::Account(account_id(i))).unwrap()
    }

    /// Creates classic account number `i` with no trustline and returns its
    /// address.
    pub fn create_account(&self, i: u32) -> Address {
        let id = account_id(i);
        let key = Rc::new(xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
            account_id: id.clone(),
        }));
        let entry = Rc::new(xdr::LedgerEntry {
            data: xdr::LedgerEntryData::Account(xdr::AccountEntry {
                account_id: id,
                balance: 100_000_000,
                flags: 0,
                home_domain: Default::default(),
                inflation_dest: None,
                num_sub_entries: 1,
                seq_num: xdr::SequenceNumber(0),
                thresholds: xdr::Thresholds([1, 0, 0, 0]),
                signers: xdr::VecM::default(),
                ext: xdr::AccountEntryExt::V0,
            }),
            last_modified_ledger_seq: 0,
            ext: xdr::LedgerEntryExt::V0,
        });
        self.e.host().add_ledger_entry(&key, &entry, None).unwrap();
        self.account(i)
    }

    /// Gives classic account number `i` an authorized USDC trustline.
    pub fn add_trustline(&self, i: u32) {
        let id = account_id(i);
        let asset = match self.sac.asset() {
            xdr::Asset::CreditAlphanum4(a) => xdr::TrustLineAsset::CreditAlphanum4(a),
            xdr::Asset::CreditAlphanum12(a) => xdr::TrustLineAsset::CreditAlphanum12(a),
            xdr::Asset::Native => unreachable!(),
        };
        let key = Rc::new(xdr::LedgerKey::Trustline(xdr::LedgerKeyTrustLine {
            account_id: id.clone(),
            asset: asset.clone(),
        }));
        let entry = Rc::new(xdr::LedgerEntry {
            data: xdr::LedgerEntryData::Trustline(xdr::TrustLineEntry {
                account_id: id,
                asset,
                balance: 0,
                limit: i64::MAX,
                flags: TRUSTLINE_AUTHORIZED,
                ext: xdr::TrustLineEntryExt::V0,
            }),
            last_modified_ledger_seq: 0,
            ext: xdr::LedgerEntryExt::V0,
        });
        self.e.host().add_ledger_entry(&key, &entry, None).unwrap();
    }

    /// Creates classic account number `i` with an authorized USDC trustline
    /// and returns its address: what a real holder looks like.
    pub fn create_account_with_trustline(&self, i: u32) -> Address {
        let address = self.create_account(i);
        self.add_trustline(i);
        address
    }
}

const TRUSTLINE_AUTHORIZED: u32 = 1;

fn account_id(i: u32) -> xdr::AccountId {
    let mut bytes = [7u8; 32];
    bytes[..4].copy_from_slice(&i.to_be_bytes());
    xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(bytes)))
}

/// The events `id` emitted in the last invocation.
pub fn emitted(e: &Env, id: &Address) -> std::vec::Vec<xdr::ContractEvent> {
    e.events().all().filter_by_contract(id).events().to_vec()
}

/// The contract behind each event of the last invocation, in order.
pub fn emitters(e: &Env) -> std::vec::Vec<Address> {
    e.events()
        .all()
        .events()
        .iter()
        .map(|ev| {
            let emitter = xdr::ScAddress::Contract(ev.contract_id.clone().unwrap());
            Address::try_from_val(e, &emitter).unwrap()
        })
        .collect()
}

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
