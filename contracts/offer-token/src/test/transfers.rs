//! The token's normal behaviour: metadata, the "from OR to" whitelist rule,
//! whitelist management, the offering URI and forced transfer.
use soroban_sdk::{testutils::Address as _, xdr, Address, Env, MuxedAddress, String, TryFromVal};

use super::{err, setup, SUPPLY, URI};
use crate::OfferTokenError;

#[test]
fn construction_sets_metadata_powers_and_supply() {
    let s = setup();
    let e = &s.e;
    assert_eq!(s.client.name(), String::from_str(e, "Edificio Aurora"));
    assert_eq!(s.client.symbol(), String::from_str(e, "AUR"));
    assert_eq!(s.client.decimals(), 0);
    assert_eq!(s.client.total_supply(), SUPPLY);
    assert_eq!(s.client.balance(&s.sale), SUPPLY - 200);
    assert_eq!(s.client.offer_uri(), String::from_str(e, URI));
    assert_eq!(s.client.admin(), s.admin);
    assert_eq!(s.client.xfer_admin(), s.xfer_admin);
    assert_eq!(s.client.pauser(), s.pauser);
    assert_eq!(s.client.upgrader(), s.upgrader);
    assert!(!s.client.paused());
    assert_eq!(s.client.schema_version(), 1);
}

#[test]
fn initial_holder_and_listed_accounts_are_whitelisted_and_nobody_else() {
    let s = setup();
    assert!(s.client.is_transfer_whitelisted(&s.sale));
    assert!(s.client.is_transfer_whitelisted(&s.listed));
    assert!(!s.client.is_transfer_whitelisted(&s.admin));
    assert!(!s.client.is_transfer_whitelisted(&s.alice));
}

#[test]
fn investor_can_return_tokens_to_a_whitelisted_account() {
    let s = setup();
    s.client.transfer(&s.alice, &s.listed, &40);
    assert_eq!(s.client.balance(&s.alice), 60);
    assert_eq!(s.client.balance(&s.listed), 40);
}

#[test]
fn investor_cannot_transfer_to_another_investor() {
    let s = setup();
    let r = s.client.try_transfer(&s.alice, &s.bob, &1);
    assert_eq!(err(r), OfferTokenError::TransferNotWhitelisted.into());
    assert_eq!(s.client.balance(&s.bob), 100);
}

#[test]
fn whitelisted_sender_can_send_to_any_account() {
    let s = setup();
    let carol = Address::generate(&s.e);
    s.client.transfer(&s.listed, &carol, &0);
    s.client.transfer(&s.sale, &carol, &5);
    assert_eq!(s.client.balance(&carol), 5);
}

#[test]
fn transfer_from_follows_the_same_rule() {
    let s = setup();
    let spender = Address::generate(&s.e);
    s.client.approve(&s.alice, &spender, &50, &1_000);
    let r = s.client.try_transfer_from(&spender, &s.alice, &s.bob, &10);
    assert_eq!(err(r), OfferTokenError::TransferNotWhitelisted.into());
    s.client.transfer_from(&spender, &s.alice, &s.listed, &10);
    assert_eq!(s.client.balance(&s.listed), 10);
    assert_eq!(s.client.allowance(&s.alice, &spender), 40);
}

/// A Stellar account (G...) built from a fixed key, and a muxed address
/// (M...) on top of it. Muxed addresses only exist for accounts.
fn account(e: &Env, key: u8) -> Address {
    let sc = xdr::ScAddress::Account(xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(
        xdr::Uint256([key; 32]),
    )));
    Address::try_from_val(e, &xdr::ScVal::Address(sc)).unwrap()
}

fn muxed(e: &Env, key: u8, id: u64) -> MuxedAddress {
    let sc = xdr::ScAddress::MuxedAccount(xdr::MuxedEd25519Account {
        id,
        ed25519: xdr::Uint256([key; 32]),
    });
    MuxedAddress::try_from_val(e, &xdr::ScVal::Address(sc)).unwrap()
}

#[test]
fn muxed_recipient_is_checked_on_its_underlying_account() {
    let s = setup();
    let r = s.client.try_transfer(&s.alice, muxed(&s.e, 7, 42), &1);
    assert_eq!(err(r), OfferTokenError::TransferNotWhitelisted.into());

    let exchange = account(&s.e, 9);
    s.client.set_transfer_whitelist(&exchange, &true, &s.admin);
    s.client.transfer(&s.alice, muxed(&s.e, 9, 42), &1);
    assert_eq!(s.client.balance(&exchange), 1);
}

#[test]
fn admin_adds_and_removes_whitelisted_accounts() {
    let s = setup();
    s.client.set_transfer_whitelist(&s.alice, &true, &s.admin);
    assert!(s.client.is_transfer_whitelisted(&s.alice));
    s.client.transfer(&s.alice, &s.bob, &10);
    assert_eq!(s.client.balance(&s.bob), 110);

    s.client.set_transfer_whitelist(&s.alice, &false, &s.admin);
    assert!(!s.client.is_transfer_whitelisted(&s.alice));
    let r = s.client.try_transfer(&s.alice, &s.bob, &10);
    assert_eq!(err(r), OfferTokenError::TransferNotWhitelisted.into());
}

#[test]
fn removing_the_sale_from_the_whitelist_blocks_its_transfers() {
    // Documented, not prevented: release fails until the sale is re-added.
    let s = setup();
    s.client.set_transfer_whitelist(&s.sale, &false, &s.admin);
    let r = s.client.try_transfer(&s.sale, &s.alice, &1);
    assert_eq!(err(r), OfferTokenError::TransferNotWhitelisted.into());
}

#[test]
fn admin_replaces_the_offering_uri() {
    let s = setup();
    let uri = String::from_str(&s.e, "ipfs://offering-v2");
    s.client.set_offer_uri(&uri, &s.admin);
    assert_eq!(s.client.offer_uri(), uri);
}

#[test]
fn forced_transfer_ignores_the_whitelist() {
    let s = setup();
    s.client
        .admin_transfer(&s.alice, &s.bob, &30, &s.xfer_admin);
    assert_eq!(s.client.balance(&s.alice), 70);
    assert_eq!(s.client.balance(&s.bob), 130);
}

#[test]
fn forced_transfer_can_move_tokens_out_of_the_sale() {
    let s = setup();
    s.client
        .admin_transfer(&s.sale, &s.admin, &50, &s.xfer_admin);
    assert_eq!(s.client.balance(&s.admin), 50);
}

#[test]
fn supply_never_changes() {
    let s = setup();
    s.client.transfer(&s.alice, &s.listed, &10);
    s.client
        .admin_transfer(&s.bob, &s.alice, &10, &s.xfer_admin);
    assert_eq!(s.client.total_supply(), SUPPLY);
}
