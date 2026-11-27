//! A paused distributor blocks `deposit` and `claim` and nothing else: the
//! reads keep answering, the owner can still withdraw, and the powers keep
//! working.
use super::{code, err, setup, SUPPLY, USDC};

const ENFORCED_PAUSE: u32 = 1000;
const EXPECTED_PAUSE: u32 = 1001;

#[test]
fn paused_blocks_deposit_and_claim_until_the_owner_unpauses() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.client.pause(&s.pauser);
    assert_eq!(
        err(s.client.try_deposit(&USDC, &s.owner)),
        code(ENFORCED_PAUSE)
    );
    assert_eq!(
        err(s.client.try_claim(&s.holders(&[&s.alice]))),
        code(ENFORCED_PAUSE)
    );
    assert_eq!(s.usdc.balance(&s.alice), 0);
    s.client.unpause(&s.owner);
    s.claim(&[&s.alice]);
    assert_eq!(s.usdc.balance(&s.alice), 100 * USDC);
    s.deposit(USDC);
}

#[test]
fn the_owner_can_pause_its_own_distributor() {
    let s = setup();
    s.client.pause(&s.owner);
    assert_eq!(
        err(s.client.try_deposit(&USDC, &s.owner)),
        code(ENFORCED_PAUSE)
    );
}

#[test]
fn the_reads_answer_while_paused() {
    let s = setup();
    s.give(&s.alice, 100);
    s.deposit(SUPPLY * USDC);
    s.client.pause(&s.pauser);
    assert_eq!(s.client.claimable(&s.alice), 100 * USDC);
    assert_eq!(s.client.credited(&s.alice), 0);
    assert_eq!(s.client.powers().owner, s.owner);
    assert_eq!(s.client.contracts().token, s.token.address);
    assert_eq!(s.client.schema_version(), 1);
    assert!(s.client.paused());
}

#[test]
fn pause_twice_and_unpause_a_live_distributor_fail_with_open_zeppelins_codes() {
    let s = setup();
    assert_eq!(err(s.client.try_unpause(&s.owner)), code(EXPECTED_PAUSE));
    s.client.pause(&s.pauser);
    assert_eq!(err(s.client.try_pause(&s.owner)), code(ENFORCED_PAUSE));
    assert!(s.client.paused());
}
