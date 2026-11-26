//! What the distributor reads from `offer-token`: a holder's balance, the
//! supply, and whether an account is on the transfer whitelist.
//! Hand-written, so this WASM carries none of the token's code.
use soroban_sdk::{contractclient, Address, Env};

// The trait exists for the client the attribute derives from it.
#[allow(dead_code)]
#[contractclient(name = "QuotaTokenClient")]
pub trait QuotaToken {
    fn balance(e: Env, account: Address) -> i128;

    fn total_supply(e: Env) -> i128;

    fn is_transfer_whitelisted(e: Env, account: Address) -> bool;
}
