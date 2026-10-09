//! Tests drive the token only through its generated client, as the backend
//! and other contracts would. Modules follow the D2 vocabulary so the
//! evidence index can cite them directly.
#![cfg(test)]
extern crate std;

mod authorization;
mod boundary;
mod events;
mod failure;
mod idempotency;
mod pause;
mod transfers;
mod ttl;
mod upgrade;

use soroban_sdk::{testutils::Address as _, vec, Address, Env, InvokeError, String, Vec};

use crate::{OfferToken, OfferTokenClient};

pub const SUPPLY: i128 = 1_000;
pub const URI: &str = "ipfs://offering-v1";

/// One deployed token with every party as a distinct account.
pub struct Setup<'a> {
    pub e: Env,
    pub id: Address,
    pub client: OfferTokenClient<'a>,
    pub admin: Address,
    pub xfer_admin: Address,
    pub pauser: Address,
    pub upgrader: Address,
    /// The initial holder, standing in for the offering's sale contract.
    pub sale: Address,
    /// An operational account whitelisted at construction (e.g. a distributor).
    pub listed: Address,
    /// Two investors, never whitelisted.
    pub alice: Address,
    pub bob: Address,
}

pub fn ctor_args(
    e: &Env,
    s: &Parties,
    supply: i128,
    whitelist: Vec<Address>,
) -> (
    Address,
    Address,
    Address,
    Address,
    String,
    String,
    i128,
    Address,
    Vec<Address>,
    String,
) {
    (
        s.admin.clone(),
        s.xfer_admin.clone(),
        s.pauser.clone(),
        s.upgrader.clone(),
        String::from_str(e, "Edificio Aurora"),
        String::from_str(e, "AUR"),
        supply,
        s.sale.clone(),
        whitelist,
        String::from_str(e, URI),
    )
}

pub struct Parties {
    pub admin: Address,
    pub xfer_admin: Address,
    pub pauser: Address,
    pub upgrader: Address,
    pub sale: Address,
}

pub fn parties(e: &Env) -> Parties {
    Parties {
        admin: Address::generate(e),
        xfer_admin: Address::generate(e),
        pauser: Address::generate(e),
        upgrader: Address::generate(e),
        sale: Address::generate(e),
    }
}

/// Deploys a token and hands 100 tokens each to alice and bob through the
/// sale, the way release will. Auths are mocked; authorization tests replace
/// the mocks per call.
pub fn setup<'a>() -> Setup<'a> {
    let e = Env::default();
    e.mock_all_auths();
    let p = parties(&e);
    let listed = Address::generate(&e);
    let id = e.register(
        OfferToken,
        ctor_args(&e, &p, SUPPLY, vec![&e, listed.clone()]),
    );
    let client = OfferTokenClient::new(&e, &id);
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);
    client.transfer(&p.sale, &alice, &100);
    client.transfer(&p.sale, &bob, &100);
    Setup {
        e,
        id,
        client,
        admin: p.admin,
        xfer_admin: p.xfer_admin,
        pauser: p.pauser,
        upgrader: p.upgrader,
        sale: p.sale,
        listed,
        alice,
        bob,
    }
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
