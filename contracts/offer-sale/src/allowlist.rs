//! The slice of `offer-allowlist` the sale calls, as a hand-written client
//! trait. Depending on the allowlist crate would export its functions into
//! this WASM; importing its binary would tie every build to it. The tests
//! register the real allowlist, so any drift between this trait and the
//! contract fails there.
use soroban_sdk::{contractclient, Address, Env, Symbol, Vec};

// The trait exists for the client the attribute derives from it.
#[allow(dead_code)]
#[contractclient(name = "AllowlistClient")]
pub trait Allowlist {
    /// Uses room and returns the receipt number of the purchase.
    fn consume(e: Env, investor: Address, amount: i128, caller: Address) -> u32;

    /// Gives back the named receipts, at most 20.
    fn restore(e: Env, investor: Address, receipts: Vec<u32>, caller: Address);

    /// OpenZeppelin's role check: `Some` when held.
    fn has_role(e: Env, account: Address, role: Symbol) -> Option<u32>;
}
