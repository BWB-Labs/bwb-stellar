//! Repeating an administrative call converges to the same state.
use soroban_sdk::String;

use super::setup;

#[test]
fn adding_twice_keeps_the_operational_account() {
    let s = setup();
    s.client.set_operational_account(&s.alice, &true, &s.admin);
    s.client.set_operational_account(&s.alice, &true, &s.admin);
    assert!(s.client.is_operational_account(&s.alice));
}

#[test]
fn removing_an_absent_account_is_a_no_op() {
    let s = setup();
    s.client.set_operational_account(&s.alice, &false, &s.admin);
    s.client.set_operational_account(&s.alice, &false, &s.admin);
    assert!(!s.client.is_operational_account(&s.alice));
}

#[test]
fn setting_the_same_uri_twice_succeeds() {
    let s = setup();
    let uri = String::from_str(&s.e, "ipfs://same");
    s.client.set_offer_uri(&uri, &s.admin);
    s.client.set_offer_uri(&uri, &s.admin);
    assert_eq!(s.client.offer_uri(), uri);
}
