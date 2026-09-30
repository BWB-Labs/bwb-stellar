#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Env, String};

#[test]
fn constructor_sets_admin_and_hello_answers() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let id = e.register(OfferToken, (&admin,));
    let client = OfferTokenClient::new(&e, &id);

    assert_eq!(client.get_admin(), Some(admin));

    let words = client.hello(&String::from_str(&e, "Dev"));
    assert_eq!(
        words,
        vec![
            &e,
            String::from_str(&e, "Hello"),
            String::from_str(&e, "Dev")
        ]
    );
}
