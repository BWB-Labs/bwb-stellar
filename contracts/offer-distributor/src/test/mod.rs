//! Tests drive the distributor through its generated client, as the backend
//! would, beside a Stellar Asset Contract standing in for USDC. The owner
//! is funded and pays from its own account, as in production. The `ttl`
//! module reaches storage through `as_contract` to read lifetimes. Modules
//! follow the D2 vocabulary so the evidence index can cite them directly.
#![cfg(test)]
extern crate std;

mod authorization;
mod boundary;
mod events;
mod failure;
mod idempotency;
mod pause;
mod ttl;
mod upgrade;

use std::rc::Rc;

use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, IssuerFlags, StellarAssetContract},
    token::{StellarAssetClient, TokenClient},
    xdr, Address, BytesN, Env, IntoVal, InvokeError, Symbol, TryFromVal, Vec,
};

use crate::{OfferDistributor, OfferDistributorClient, Payout, PowerHolders};

/// 1 USDC, in stroops.
pub const USDC: i128 = 10_000_000;
/// What the owner holds at the start of every test.
pub const FUNDS: i128 = 1_000_000 * USDC;

/// One deployed distributor beside a SAC as the payment asset, every party
/// a distinct account. Auths are mocked; authorization tests replace the
/// mocks per call.
pub struct Setup<'a> {
    pub e: Env,
    pub id: Address,
    pub client: OfferDistributorClient<'a>,
    pub sac: StellarAssetContract,
    pub usdc: TokenClient<'a>,
    pub usdc_admin: StellarAssetClient<'a>,
    /// The treasury that pays.
    pub owner: Address,
    /// The platform's pauser. A different account from the upgrader, so a
    /// test can tell the two powers apart.
    pub pauser: Address,
    /// The platform's upgrader.
    pub upgrader: Address,
    /// An account with no power here, funded like the owner.
    pub stranger: Address,
    pub alice: Address,
    pub bob: Address,
}

pub fn setup<'a>() -> Setup<'a> {
    let e = Env::default();
    e.mock_all_auths();
    let owner = Address::generate(&e);
    let pauser = Address::generate(&e);
    let upgrader = Address::generate(&e);
    let stranger = Address::generate(&e);
    let issuer = Address::generate(&e);
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    // Revocable, like Circle's USDC: the issuer can freeze a balance.
    let sac = e.register_stellar_asset_contract_v2(issuer);
    sac.issuer().set_flag(IssuerFlags::RevocableFlag);
    let usdc_id = sac.address();
    let id = e.register(
        OfferDistributor,
        (PowerHolders {
            owner: owner.clone(),
            pauser: pauser.clone(),
            upgrader: upgrader.clone(),
        },),
    );
    let usdc_admin = StellarAssetClient::new(&e, &usdc_id);
    usdc_admin.mint(&owner, &FUNDS);
    usdc_admin.mint(&stranger, &FUNDS);
    Setup {
        client: OfferDistributorClient::new(&e, &id),
        usdc: TokenClient::new(&e, &usdc_id),
        usdc_admin,
        sac,
        id,
        e,
        owner,
        pauser,
        upgrader,
        stranger,
        alice,
        bob,
    }
}

impl Setup<'_> {
    /// A distribution id: 32 bytes of `n`.
    pub fn dist(&self, n: u8) -> BytesN<32> {
        BytesN::from_array(&self.e, &[n; 32])
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

    /// The owner pays `legs` in USDC under distribution `n`.
    pub fn distribute(&self, n: u8, legs: &[(&Address, i128)]) {
        self.client.distribute(
            &self.dist(n),
            &self.usdc.address,
            &self.payouts(legs),
            &self.owner,
        );
    }

    pub fn try_distribute(&self, n: u8, legs: &[(&Address, i128)]) -> TryResult<()> {
        self.client.try_distribute(
            &self.dist(n),
            &self.usdc.address,
            &self.payouts(legs),
            &self.owner,
        )
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

    /// Creates classic account number `i` with an authorized USDC trustline
    /// and returns its address: what a real recipient looks like.
    pub fn create_account_with_trustline(&self, i: u32) -> Address {
        let address = self.create_account(i);
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
        address
    }
}

const TRUSTLINE_AUTHORIZED: u32 = 1;

fn account_id(i: u32) -> xdr::AccountId {
    let mut bytes = [7u8; 32];
    bytes[..4].copy_from_slice(&i.to_be_bytes());
    xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(bytes)))
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

/// An "asset" whose `transfer` does nothing at all: no signature asked, no
/// balance moved. The distributor cannot tell it from a real one.
#[contract]
pub struct InertToken;

#[contractimpl]
impl InertToken {
    pub fn transfer(_e: &Env, _from: Address, _to: Address, _amount: i128) {}
}

/// An "asset" whose `transfer` calls back into the distributor that is
/// calling it.
#[contract]
pub struct ReentrantToken;

#[contracttype]
pub enum HostileKey {
    Distributor,
    Usdc,
    Thief,
}

#[contractimpl]
impl ReentrantToken {
    pub fn __constructor(e: &Env, distributor: Address) {
        e.storage()
            .instance()
            .set(&HostileKey::Distributor, &distributor);
    }

    pub fn transfer(e: &Env, _from: Address, _to: Address, _amount: i128) {
        let distributor: Address = e
            .storage()
            .instance()
            .get(&HostileKey::Distributor)
            .unwrap();
        let id = BytesN::from_array(e, &[0; 32]);
        let _: bool =
            e.invoke_contract(&distributor, &Symbol::new(e, "is_paid"), (id,).into_val(e));
    }
}

/// An "asset" whose `transfer` also moves real USDC from the sender to a
/// thief: what a wrong asset address could try with the owner's signature.
#[contract]
pub struct GreedyToken;

#[contractimpl]
impl GreedyToken {
    pub fn __constructor(e: &Env, usdc: Address, thief: Address) {
        e.storage().instance().set(&HostileKey::Usdc, &usdc);
        e.storage().instance().set(&HostileKey::Thief, &thief);
    }

    pub fn transfer(e: &Env, from: Address, _to: Address, amount: i128) {
        let usdc: Address = e.storage().instance().get(&HostileKey::Usdc).unwrap();
        let thief: Address = e.storage().instance().get(&HostileKey::Thief).unwrap();
        TokenClient::new(e, &usdc).transfer(&from, &thief, &amount);
    }
}

/// What a `try_` call carries when a call the distributor makes fails
/// without a contract code: a refused authorization inside the asset, a
/// re-entry, an address with no contract.
pub fn trapped() -> soroban_sdk::Error {
    soroban_sdk::Error::from_type_and_code(
        xdr::ScErrorType::Context,
        xdr::ScErrorCode::InvalidAction,
    )
}
