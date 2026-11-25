//! A paused distributor blocks `distribute` and nothing else: the reads
//! keep answering, and the powers keep working.
use super::{code, err, setup, USDC};

const ENFORCED_PAUSE: u32 = 1000;
const EXPECTED_PAUSE: u32 = 1001;

#[test]
fn paused_blocks_distribute_until_the_owner_unpauses() {
    let s = setup();
    s.client.pause(&s.pauser);
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, USDC)])),
        code(ENFORCED_PAUSE)
    );
    assert_eq!(s.usdc.balance(&s.alice), 0);
    assert!(!s.client.is_paid(&s.dist(1)));
    s.client.unpause(&s.owner);
    s.distribute(1, &[(&s.alice, USDC)]);
    assert_eq!(s.usdc.balance(&s.alice), USDC);
}

#[test]
fn the_owner_can_pause_its_own_distributor() {
    let s = setup();
    s.client.pause(&s.owner);
    assert_eq!(
        err(s.try_distribute(1, &[(&s.alice, USDC)])),
        code(ENFORCED_PAUSE)
    );
}

#[test]
fn the_reads_answer_while_paused() {
    let s = setup();
    s.distribute(1, &[(&s.alice, USDC)]);
    s.client.pause(&s.pauser);
    assert!(s.client.is_paid(&s.dist(1)));
    assert!(!s.client.is_paid(&s.dist(2)));
    assert_eq!(s.client.powers().owner, s.owner);
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
