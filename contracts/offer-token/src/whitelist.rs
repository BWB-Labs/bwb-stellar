//! The transfer rule: an ordinary transfer passes only if the sender or the
//! recipient is whitelisted. Base's `OfferERC20` has the same rule;
//! OpenZeppelin's `AllowList` requires both sides, so the token supplies its
//! own contract type in the same shape.
use soroban_sdk::{panic_with_error, Address, Env, MuxedAddress};
use stellar_tokens::fungible::{Base, ContractOverrides};

use crate::errors::OfferTokenError;
use crate::storage;

pub struct Whitelisted;

fn require_either(e: &Env, from: &Address, to: &Address) {
    if !storage::is_whitelisted(e, from) && !storage::is_whitelisted(e, to) {
        panic_with_error!(e, OfferTokenError::TransferNotWhitelisted);
    }
}

impl ContractOverrides for Whitelisted {
    fn transfer(e: &Env, from: &Address, to: &MuxedAddress, amount: i128) {
        // A muxed recipient is checked on its underlying account.
        let to_account = to.address();
        require_either(e, from, &to_account);
        Base::transfer(e, from, to, amount);
        storage::extend_balances(e, from, &to_account);
    }

    fn transfer_from(e: &Env, spender: &Address, from: &Address, to: &Address, amount: i128) {
        require_either(e, from, to);
        Base::transfer_from(e, spender, from, to, amount);
        storage::extend_balances(e, from, to);
    }
}
