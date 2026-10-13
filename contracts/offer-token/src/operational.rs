//! The transfer rule: an ordinary transfer passes only if the sender or the
//! recipient is an operational account (the sale contract, distributors;
//! never an investor). Base's `OfferERC20` has the same rule, under the name
//! `transferWhitelist`; OpenZeppelin's `AllowList` requires both sides, so the
//! token supplies its own contract type in the same shape.
//!
//! Investors are never operational accounts, so they can return quotas but
//! not trade among themselves. Not to be confused with the investor allowlist.
use soroban_sdk::{panic_with_error, Address, Env, MuxedAddress};
use stellar_tokens::fungible::{Base, ContractOverrides};

use crate::errors::OfferTokenError;
use crate::storage;

pub struct OperationalRule;

fn require_either(e: &Env, from: &Address, to: &Address) {
    if !storage::is_operational_account(e, from) && !storage::is_operational_account(e, to) {
        panic_with_error!(e, OfferTokenError::NoOperationalParty);
    }
}

impl ContractOverrides for OperationalRule {
    fn transfer(e: &Env, from: &Address, to: &MuxedAddress, amount: i128) {
        // A muxed recipient is checked on its underlying account.
        let to_account = to.address();
        require_either(e, from, &to_account);
        Base::transfer(e, from, to, amount);
        storage::extend_balance(e, from);
        storage::extend_balance(e, &to_account);
    }

    fn transfer_from(e: &Env, spender: &Address, from: &Address, to: &Address, amount: i128) {
        require_either(e, from, to);
        Base::transfer_from(e, spender, from, to, amount);
        storage::extend_balance(e, from);
        storage::extend_balance(e, to);
    }
}
