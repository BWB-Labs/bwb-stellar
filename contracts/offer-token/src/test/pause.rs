//! Pause blocks balance movements by holders, never approvals or forced
//! transfers, and either pauser leaves a trace of who paused.
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

use super::{code, err, setup};

const ENFORCED_PAUSE: u32 = 1000;
const EXPECTED_PAUSE: u32 = 1001;

#[test]
fn paused_token_blocks_transfer_and_transfer_from() {
    let s = setup();
    let spender = Address::generate(&s.e);
    s.client.approve(&s.alice, &spender, &10, &1_000);
    s.client.pause(&s.admin);
    let r = s.client.try_transfer(&s.alice, &s.listed, &1);
    assert_eq!(err(r), code(ENFORCED_PAUSE));
    let r = s
        .client
        .try_transfer_from(&spender, &s.alice, &s.listed, &1);
    assert_eq!(err(r), code(ENFORCED_PAUSE));
}

#[test]
fn approve_works_while_paused() {
    let s = setup();
    let spender = Address::generate(&s.e);
    s.client.pause(&s.pauser);
    s.client.approve(&s.alice, &spender, &10, &1_000);
    assert_eq!(s.client.allowance(&s.alice, &spender), 10);
}

#[test]
fn forced_transfer_ignores_pause() {
    let s = setup();
    s.client.pause(&s.pauser);
    s.client
        .admin_transfer(&s.alice, &s.bob, &10, &s.xfer_admin);
    assert_eq!(s.client.balance(&s.bob), 110);
}

#[test]
fn admin_actions_still_work_while_paused() {
    let s = setup();
    s.client.pause(&s.admin);
    s.client.set_transfer_whitelist(&s.alice, &true, &s.admin);
    assert!(s.client.is_transfer_whitelisted(&s.alice));
}

#[test]
fn unpause_restores_transfers() {
    let s = setup();
    s.client.pause(&s.pauser);
    s.client.unpause(&s.admin);
    s.client.transfer(&s.alice, &s.listed, &1);
    assert_eq!(s.client.balance(&s.listed), 1);
}

#[test]
fn pausing_twice_or_unpausing_a_live_token_fails() {
    let s = setup();
    assert_eq!(err(s.client.try_unpause(&s.admin)), code(EXPECTED_PAUSE));
    s.client.pause(&s.admin);
    assert_eq!(err(s.client.try_pause(&s.pauser)), code(ENFORCED_PAUSE));
}
