//! A paused sale blocks what moves money toward the tokenizer and never
//! the investor's exit or the decision; a paused token blocks the sale's
//! own token movements.
use super::{code, err, setup, PRICE, SUPPLY};
use crate::SaleState;

const ENFORCED_PAUSE: u32 = 1000;
const EXPECTED_PAUSE: u32 = 1001;

#[test]
fn paused_blocks_buy_but_not_cooling_off() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.client.pause(&s.platform);
    assert_eq!(err(s.try_buy(&s.bob, 1)), code(ENFORCED_PAUSE));
    s.client.cooling_off_refund(&s.alice);
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    s.client.unpause(&s.owner);
    s.buy(&s.bob, 1);
}

#[test]
fn paused_blocks_release_and_payout_in_successful() {
    let s = setup();
    s.successful();
    s.client.pause(&s.owner);
    let list = s.investors(&[&s.alice]);
    assert_eq!(err(s.client.try_release(&list)), code(ENFORCED_PAUSE));
    let legs = s.payouts(&[(&s.owner, 1)]);
    assert_eq!(
        err(s.client.try_withdraw_payment(&legs, &s.owner)),
        code(ENFORCED_PAUSE)
    );
    s.client.unpause(&s.owner);
    s.client.release(&list);
    s.client.withdraw_payment(&legs, &s.owner);
}

#[test]
fn paused_blocks_token_withdrawal_but_not_refunds_in_failed() {
    let s = setup();
    s.failed();
    s.client.pause(&s.platform);
    assert_eq!(
        err(s.client.try_withdraw_tokens(&s.owner, &1, &s.owner)),
        code(ENFORCED_PAUSE)
    );
    s.client.refund(&s.investors(&[&s.alice]));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
}

#[test]
fn close_cancel_and_finalize_work_while_paused() {
    let s = setup();
    s.buy(&s.alice, 750);
    s.client.pause(&s.platform);
    s.client.close(&s.owner);
    s.client.reopen(&s.owner);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Successful);
    let s = setup();
    s.client.pause(&s.platform);
    s.client.cancel(&s.owner);
    assert_eq!(s.client.state(), SaleState::Failed);
}

/// The token's own pause stops the sale's transfers and its burn: the
/// finalize waits until the token's admin unpauses.
#[test]
fn a_paused_token_blocks_release_and_the_burn_in_finalize() {
    let s = setup();
    s.buy(&s.alice, 750);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.token.pause(&s.platform);
    assert_eq!(err(s.client.try_finalize()), code(ENFORCED_PAUSE));
    assert_eq!(s.client.state(), SaleState::Closing);
    s.token.unpause(&s.token_admin);
    s.client.finalize();
    s.token.pause(&s.platform);
    let list = s.investors(&[&s.alice]);
    assert_eq!(err(s.client.try_release(&list)), code(ENFORCED_PAUSE));
}

#[test]
fn pause_twice_and_unpause_a_live_sale_fail_with_open_zeppelins_codes() {
    let s = setup();
    assert_eq!(err(s.client.try_unpause(&s.owner)), code(EXPECTED_PAUSE));
    s.client.pause(&s.platform);
    assert_eq!(err(s.client.try_pause(&s.owner)), code(ENFORCED_PAUSE));
    assert!(s.client.paused());
}
