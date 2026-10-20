//! Tests drive the allowlist through its generated client, as the backend
//! and the sale contract would, with the ledger time pinned per test. The
//! exceptions reach storage through `as_contract`: the `ttl` module reads
//! entry lifetimes, `boundary` checks a usage entry is gone, and `failure`
//! writes two states the contract never produces (a usage below its
//! receipt, a receipt counter at `u32::MAX`). Modules follow the D2
//! vocabulary so the evidence index can cite them directly.
#![cfg(test)]
extern crate std;

mod authorization;
mod boundary;
mod events;
mod failure;
mod idempotency;
mod period;
mod ttl;
mod upgrade;

use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, testutils::Ledger as _, vec, Address, Env,
    InvokeError, Symbol, Vec,
};

use crate::{AllocationEntry, OfferAllowlist, OfferAllowlistClient};

/// A test-only stand-in for the sale contract: it calls `consume` and
/// `restore` with its own address as `caller`, so the direct-caller
/// authorization is exercised for real, not through mocked auths. `buy`
/// returns the receipt number, which a real sale contract would store with
/// the purchase.
#[contract]
pub struct SaleStub;

#[contractimpl]
impl SaleStub {
    pub fn buy(e: &Env, allowlist: Address, investor: Address, amount: i128) -> u32 {
        OfferAllowlistClient::new(e, &allowlist).consume(
            &investor,
            &amount,
            &e.current_contract_address(),
        )
    }

    pub fn refund(e: &Env, allowlist: Address, investor: Address, receipts: Vec<u32>) {
        OfferAllowlistClient::new(e, &allowlist).restore(
            &investor,
            &receipts,
            &e.current_contract_address(),
        )
    }
}

/// One deployed allowlist with the platform multisig as admin and the
/// platform hot key as the only initial allocator, a sale stub already
/// granted `sale` by the hot key (the way an offering's deploy batch does),
/// and two investors with no allocation yet. Auths are mocked;
/// authorization tests replace the mocks per call.
pub struct Setup<'a> {
    pub e: Env,
    pub id: Address,
    pub client: OfferAllowlistClient<'a>,
    pub admin: Address,
    /// The platform hot key: the only allocator at construction.
    pub hot_key: Address,
    /// The sale stub's address, holding `sale`.
    pub sale: Address,
    pub alice: Address,
    pub bob: Address,
}

pub fn setup<'a>() -> Setup<'a> {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let hot_key = Address::generate(&e);
    let id = e.register(OfferAllowlist, (admin.clone(), vec![&e, hot_key.clone()]));
    let client = OfferAllowlistClient::new(&e, &id);
    let sale = e.register(SaleStub, ());
    client.grant_role(&sale, &sale_role(&e), &hot_key);
    at(&e, 2026, 10, 15);
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);
    Setup {
        e,
        id,
        client,
        admin,
        hot_key,
        sale,
        alice,
        bob,
    }
}

impl Setup<'_> {
    /// Writes an enabled allocation of `max` for `investor` as the hot key.
    pub fn allow(&self, investor: &Address, max: i128) {
        self.client
            .set_allocations(&vec![&self.e, entry(investor, max, true)], &self.hot_key);
    }

    /// The sale stub's client.
    pub fn sale(&self) -> SaleStubClient<'_> {
        SaleStubClient::new(&self.e, &self.sale)
    }

    /// A second sale stub, granted `sale` by the hot key: another offering.
    pub fn another_sale(&self) -> SaleStubClient<'_> {
        let sale = self.e.register(SaleStub, ());
        self.client
            .grant_role(&sale, &sale_role(&self.e), &self.hot_key);
        SaleStubClient::new(&self.e, &sale)
    }

    /// Consumes through the sale stub, the only kind of account that may,
    /// and returns the receipt number.
    pub fn consume(&self, investor: &Address, amount: &i128) -> u32 {
        self.sale().buy(&self.id, investor, amount)
    }

    pub fn try_consume(&self, investor: &Address, amount: &i128) -> TryResult<u32> {
        self.sale().try_buy(&self.id, investor, amount)
    }

    /// Restores an investor's receipts through the sale stub.
    pub fn restore(&self, investor: &Address, receipts: &[u32]) {
        self.sale().refund(&self.id, investor, &self.list(receipts))
    }

    pub fn try_restore(&self, investor: &Address, receipts: &[u32]) -> TryResult<()> {
        self.sale()
            .try_refund(&self.id, investor, &self.list(receipts))
    }

    /// The stub sale's receipt `receipt` for `investor`.
    pub fn receipt(&self, investor: &Address, receipt: u32) -> Option<crate::Receipt> {
        self.client.receipt(&self.sale, investor, &receipt)
    }

    /// Receipt numbers as the contract's `Vec`.
    pub fn list(&self, receipts: &[u32]) -> Vec<u32> {
        Vec::from_slice(&self.e, receipts)
    }
}

/// What a client's `try_` call returns.
pub type TryResult<T> =
    Result<Result<T, soroban_sdk::ConversionError>, Result<soroban_sdk::Error, InvokeError>>;

pub fn allocator(e: &Env) -> Symbol {
    Symbol::new(e, crate::ALLOCATOR)
}

pub fn sale_role(e: &Env) -> Symbol {
    Symbol::new(e, crate::SALE)
}

pub fn entry(investor: &Address, max: i128, enabled: bool) -> AllocationEntry {
    AllocationEntry {
        investor: investor.clone(),
        max,
        enabled,
    }
}

/// `year * 12 + (month - 1)`, the documented period of a UTC month.
pub fn period(year: u32, month: u32) -> u32 {
    year * 12 + (month - 1)
}

/// Unix timestamp of a UTC date and time (Hinnant's `days_from_civil`).
pub fn timestamp(year: u64, month: u64, day: u64, h: u64, m: u64, s: u64) -> u64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + h * 3_600 + m * 60 + s
}

/// Pins the ledger time to midnight UTC of a date.
pub fn at(e: &Env, year: u64, month: u64, day: u64) {
    at_time(e, year, month, day, 0, 0, 0);
}

pub fn at_time(e: &Env, year: u64, month: u64, day: u64, h: u64, m: u64, s: u64) {
    e.ledger()
        .set_timestamp(timestamp(year, month, day, h, m, s));
}

/// The contract error a failed call carries.
pub fn code(n: u32) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(n)
}

pub const UNAUTHORIZED: u32 = 2000;
pub const ROLE_NOT_HELD: u32 = 2007;

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
