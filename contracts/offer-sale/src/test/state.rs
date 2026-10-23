//! Every transition of the lifecycle, and every call that a state refuses.
use soroban_sdk::{testutils::Address as _, Address};

use super::{err, sale_role, setup, setup_preparing, PRICE, SUCCESS_TOKENS, SUPPLY};
use crate::{OfferSaleError, SaleState};

#[test]
fn activate_moves_preparing_to_active_with_the_supply() {
    let s = setup_preparing();
    assert_eq!(s.client.state(), SaleState::Preparing);
    assert_eq!(s.client.totals().supply, 0);
    s.client.activate();
    assert_eq!(s.client.state(), SaleState::Active);
    assert_eq!(s.client.totals().supply, SUPPLY);
    assert_eq!(
        err(s.client.try_activate()),
        OfferSaleError::NotPreparing.into()
    );
}

#[test]
fn activate_without_the_whole_supply_fails_with_inventory_missing() {
    let s = setup_preparing();
    s.token
        .admin_transfer(&s.id, &s.token_admin, &1, &s.token_admin);
    assert_eq!(
        err(s.client.try_activate()),
        OfferSaleError::InventoryMissing.into()
    );
    assert_eq!(s.client.state(), SaleState::Preparing);
}

#[test]
fn activate_without_the_sale_role_fails_with_not_sale() {
    let s = setup_preparing();
    s.allowlist.revoke_role(&s.id, &sale_role(&s.e), &s.hot_key);
    assert_eq!(err(s.client.try_activate()), OfferSaleError::NotSale.into());
}

#[test]
fn buying_before_activation_fails_with_not_active() {
    let s = setup_preparing();
    s.allow(&s.alice, PRICE);
    s.fund(&s.alice, PRICE);
    assert_eq!(
        err(s.try_buy(&s.alice, 1)),
        OfferSaleError::NotActive.into()
    );
}

#[test]
fn the_owner_closes_and_buying_stops() {
    let s = setup();
    s.buy(&s.alice, 100);
    s.client.close(&s.owner);
    assert_eq!(s.client.state(), SaleState::Closing);
    assert_eq!(err(s.try_buy(&s.bob, 1)), OfferSaleError::NotActive.into());
    assert_eq!(
        err(s.client.try_close(&s.owner)),
        OfferSaleError::NotActive.into()
    );
}

#[test]
fn selling_out_closes_by_itself() {
    let s = setup();
    s.buy(&s.alice, SUPPLY - 1);
    assert_eq!(s.client.state(), SaleState::Active);
    s.buy(&s.bob, 1);
    assert_eq!(s.client.state(), SaleState::Closing);
    assert_eq!(s.client.totals().sold_tokens, SUPPLY);
}

#[test]
fn reopen_returns_to_active_and_buying_resumes() {
    let s = setup();
    assert_eq!(
        err(s.client.try_reopen(&s.owner)),
        OfferSaleError::NotClosing.into()
    );
    s.client.close(&s.owner);
    s.client.reopen(&s.owner);
    assert_eq!(s.client.state(), SaleState::Active);
    s.buy(&s.alice, 10);
    assert_eq!(s.client.totals().sold_tokens, 10);
}

#[test]
fn cancel_fails_the_offering_from_active() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.client.cancel(&s.owner);
    assert_eq!(s.client.state(), SaleState::Failed);
    // The totals are what the refunds will undo.
    assert_eq!(s.client.totals().sold_tokens, 10);
}

#[test]
fn cancel_fails_the_offering_from_closing() {
    let s = setup();
    s.buy(&s.alice, 800);
    s.client.close(&s.owner);
    s.client.cancel(&s.owner);
    assert_eq!(s.client.state(), SaleState::Failed);
}

#[test]
fn cancel_once_decided_fails_with_not_open() {
    let s = setup();
    s.successful();
    assert_eq!(
        err(s.client.try_cancel(&s.owner)),
        OfferSaleError::NotOpen.into()
    );
    let s = setup();
    s.failed();
    assert_eq!(
        err(s.client.try_cancel(&s.owner)),
        OfferSaleError::NotOpen.into()
    );
    let s = setup_preparing();
    assert_eq!(
        err(s.client.try_cancel(&s.owner)),
        OfferSaleError::NotOpen.into()
    );
}

/// The window is inclusive of its last second, so finalize passes only
/// strictly after `last purchase + cooling-off`.
#[test]
fn finalize_waits_for_the_last_window_to_run_out() {
    let s = setup();
    s.buy(&s.alice, 750);
    s.client.close(&s.owner);
    assert_eq!(
        err(s.client.try_finalize()),
        OfferSaleError::CoolingOffOpen.into()
    );
    s.advance(super::COOLING_OFF);
    assert_eq!(
        err(s.client.try_finalize()),
        OfferSaleError::CoolingOffOpen.into()
    );
    s.advance(1);
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Successful);
}

#[test]
fn finalize_at_exactly_the_minimum_succeeds() {
    let s = setup();
    s.buy(&s.alice, 750);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Successful);
}

#[test]
fn finalize_below_the_minimum_fails_the_offering() {
    let s = setup();
    s.buy(&s.alice, 749);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Failed);
    // Nothing burned, nothing moved: the refunds come next.
    assert_eq!(s.token.total_supply(), SUPPLY);
    assert_eq!(s.client.totals().sold_tokens, 749);
}

#[test]
fn finalize_burns_the_unsold_tokens() {
    let s = setup();
    s.successful();
    assert_eq!(s.token.total_supply(), SUCCESS_TOKENS);
    assert_eq!(s.token.balance(&s.id), SUCCESS_TOKENS);
}

#[test]
fn finalize_with_everything_sold_burns_nothing() {
    let s = setup();
    s.buy(&s.alice, SUPPLY);
    s.past_cooling_off();
    s.client.finalize();
    assert_eq!(s.token.total_supply(), SUPPLY);
}

#[test]
fn finalize_outside_closing_fails_with_not_closing() {
    let s = setup();
    assert_eq!(
        err(s.client.try_finalize()),
        OfferSaleError::NotClosing.into()
    );
    s.successful();
    assert_eq!(
        err(s.client.try_finalize()),
        OfferSaleError::NotClosing.into()
    );
}

/// `xfer_admin` can move the escrowed tokens; a success that cannot
/// deliver them is refused rather than declared.
#[test]
fn finalize_with_sold_tokens_missing_fails() {
    let s = setup();
    s.buy(&s.alice, 750);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.token
        .admin_transfer(&s.id, &s.token_admin, &251, &s.token_admin);
    assert_eq!(
        err(s.client.try_finalize()),
        OfferSaleError::TokensMissing.into()
    );
    s.token
        .admin_transfer(&s.token_admin, &s.id, &1, &s.token_admin);
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Successful);
    assert_eq!(s.token.balance(&s.id), 750);
}

#[test]
fn release_delivers_the_tokens_and_ends_the_reservation() {
    let s = setup();
    s.successful();
    s.client.release(&s.investors(&[&s.alice]));
    assert_eq!(s.token.balance(&s.alice), SUCCESS_TOKENS);
    assert_eq!(s.token.balance(&s.id), 0);
    assert!(s.client.reservation(&s.alice).is_none());
    assert_eq!(s.client.totals().released_tokens, SUCCESS_TOKENS);
}

#[test]
fn release_outside_successful_fails_with_not_successful() {
    let s = setup();
    s.buy(&s.alice, 10);
    let list = s.investors(&[&s.alice]);
    assert_eq!(
        err(s.client.try_release(&list)),
        OfferSaleError::NotSuccessful.into()
    );
    s.client.cancel(&s.owner);
    assert_eq!(
        err(s.client.try_release(&list)),
        OfferSaleError::NotSuccessful.into()
    );
}

#[test]
fn refund_returns_the_money_and_restores_every_receipt() {
    let s = setup();
    s.buy(&s.alice, 60);
    s.buy(&s.alice, 40);
    s.client.cancel(&s.owner);
    s.client.refund(&s.investors(&[&s.alice]));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    assert_eq!(s.usdc.balance(&s.id), 0);
    assert!(s.client.reservation(&s.alice).is_none());
    let book = s.allowlist.receipt_book(&s.id, &s.alice).unwrap();
    assert_eq!((book.next, book.receipts.len()), (2, 0));
    assert_eq!(s.allowlist.remaining(&s.alice), SUPPLY * PRICE);
    let totals = s.client.totals();
    assert_eq!((totals.sold_tokens, totals.raised), (0, 0));
}

#[test]
fn refund_outside_failed_fails_with_not_failed() {
    let s = setup();
    s.buy(&s.alice, 10);
    let list = s.investors(&[&s.alice]);
    assert_eq!(
        err(s.client.try_refund(&list)),
        OfferSaleError::NotFailed.into()
    );
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Failed);
    s.client.refund(&list);
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
}

#[test]
fn withdraw_payment_pays_each_leg_up_to_the_money_raised() {
    let s = setup();
    s.successful();
    let raised = SUCCESS_TOKENS * PRICE;
    assert_eq!(s.client.withdrawable(), raised);
    let issuer = Address::generate(&s.e);
    s.client.withdraw_payment(
        &s.payouts(&[(&issuer, 600 * PRICE), (&s.owner, 100 * PRICE)]),
        &s.owner,
    );
    assert_eq!(s.usdc.balance(&issuer), 600 * PRICE);
    assert_eq!(s.usdc.balance(&s.owner), 100 * PRICE);
    assert_eq!(s.client.withdrawable(), 50 * PRICE);
    let over = s.payouts(&[(&s.owner, 50 * PRICE + 1)]);
    assert_eq!(
        err(s.client.try_withdraw_payment(&over, &s.owner)),
        OfferSaleError::ExceedsWithdrawable.into()
    );
    s.client
        .withdraw_payment(&s.payouts(&[(&s.owner, 50 * PRICE)]), &s.owner);
    assert_eq!(s.client.withdrawable(), 0);
    assert_eq!(s.client.totals().withdrawn, raised);
}

#[test]
fn withdraw_payment_outside_successful_fails() {
    let s = setup();
    s.buy(&s.alice, 10);
    let legs = s.payouts(&[(&s.owner, 1)]);
    assert_eq!(
        err(s.client.try_withdraw_payment(&legs, &s.owner)),
        OfferSaleError::NotSuccessful.into()
    );
    assert_eq!(s.client.withdrawable(), 0);
}

#[test]
fn withdraw_tokens_only_in_failed() {
    let s = setup();
    assert_eq!(
        err(s.client.try_withdraw_tokens(&s.owner, &1, &s.owner)),
        OfferSaleError::NotFailed.into()
    );
    s.failed();
    s.client.withdraw_tokens(&s.owner, &SUPPLY, &s.owner);
    assert_eq!(s.token.balance(&s.owner), SUPPLY);
    let s = setup();
    s.successful();
    assert_eq!(
        err(s.client.try_withdraw_tokens(&s.owner, &1, &s.owner)),
        OfferSaleError::NotFailed.into()
    );
}

#[test]
fn finalizable_at_reports_the_end_of_the_latest_window() {
    let s = setup();
    assert_eq!(s.client.finalizable_at(), 0);
    s.buy(&s.alice, 1);
    assert_eq!(
        s.client.finalizable_at(),
        super::T0 + super::COOLING_OFF + 1
    );
    s.advance(10);
    s.buy(&s.bob, 1);
    assert_eq!(
        s.client.finalizable_at(),
        super::T0 + 10 + super::COOLING_OFF + 1
    );
}

/// Agreed with the team on 2026-10-22: a cooling-off refund during Closing
/// that drops the offering below its minimum puts it back to Active, since
/// there is still time to sell; nobody can fail it in the meantime.
#[test]
fn a_refund_below_the_minimum_during_closing_reopens_buying() {
    let s = setup();
    s.buy(&s.alice, 700);
    s.buy(&s.bob, 60);
    s.client.close(&s.owner);
    s.client.cooling_off_refund(&s.bob);
    assert_eq!(s.client.state(), SaleState::Active);
    assert_eq!(s.client.totals().sold_tokens, 700);
    // Buying is open again, and a later close starts a new wait.
    s.buy(&s.bob, 50);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    assert_eq!(s.client.state(), SaleState::Successful);
}

/// The window's last second is the case the audit raised: before, the next
/// ledger's `finalize` by anyone would fail the offering.
#[test]
fn a_refund_below_the_minimum_at_the_last_second_cannot_be_raced_into_failed() {
    let s = setup();
    s.buy(&s.alice, 700);
    s.buy(&s.bob, 60);
    s.client.close(&s.owner);
    s.advance(super::COOLING_OFF);
    s.client.cooling_off_refund(&s.bob);
    assert_eq!(s.client.state(), SaleState::Active);
    s.advance(1);
    assert_eq!(
        err(s.client.try_finalize()),
        OfferSaleError::NotClosing.into()
    );
}

#[test]
fn a_refund_that_keeps_the_minimum_stays_closing() {
    let s = setup();
    s.buy(&s.alice, 760);
    s.buy(&s.bob, 20);
    s.client.close(&s.owner);
    s.client.cooling_off_refund(&s.bob);
    assert_eq!(s.client.state(), SaleState::Closing);
    assert_eq!(err(s.try_buy(&s.bob, 1)), OfferSaleError::NotActive.into());
}

/// After a sell-out, a refund frees supply; above the minimum the offering
/// stays closed, and the freed tokens are burned at finalize.
#[test]
fn a_refund_after_a_sell_out_above_the_minimum_stays_closing() {
    let s = setup();
    s.buy(&s.alice, 900);
    s.buy(&s.bob, 100);
    assert_eq!(s.client.state(), SaleState::Closing);
    s.client.cooling_off_refund(&s.bob);
    assert_eq!(s.client.state(), SaleState::Closing);
    s.past_cooling_off();
    s.client.finalize();
    assert_eq!(s.token.total_supply(), 900);
}

#[test]
fn a_refund_in_active_never_changes_the_state() {
    let s = setup();
    s.buy(&s.alice, 760);
    s.client.cooling_off_refund(&s.alice);
    assert_eq!(s.client.state(), SaleState::Active);
}

/// A token that names another account as its initial holder could never
/// burn for this sale, so `finalize` would fail with unsold tokens left:
/// the pairing is refused at activation, before anyone pays.
#[test]
fn activate_against_a_token_that_names_another_holder_fails_with_not_initial_holder() {
    let s = setup_preparing();
    let other = Address::generate(&s.e);
    s.e.register_at(
        &other,
        crate::OfferSale,
        (s.client.powers(), s.client.contracts(), super::config()),
    );
    s.token
        .admin_transfer(&s.id, &other, &SUPPLY, &s.token_admin);
    s.allowlist.grant_role(&other, &sale_role(&s.e), &s.hot_key);
    let client = crate::OfferSaleClient::new(&s.e, &other);
    assert_eq!(
        err(client.try_activate()),
        OfferSaleError::NotInitialHolder.into()
    );
}
