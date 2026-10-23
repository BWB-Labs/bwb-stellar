//! Each entrypoint emits exactly the events the sale document lists, in
//! the documented order, with the totals after the change.
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    xdr, Address, Env, Event as _, Symbol, TryFromVal,
};

use super::{setup, setup_preparing, PRICE, SUPPLY, T0};
use crate::events::{
    Activated, Cancelled, Closed, Configured, Finalized, FinalizedFailed, PauseChanged,
    PaymentWithdrawn, Purchased, Refunded, Released, Reopened, RestoreSkipped, StateChanged,
    TokensWithdrawn,
};

extern crate std;
use std::vec::Vec;

/// The events the sale emitted in the last invocation.
fn emitted(e: &Env, id: &Address) -> Vec<xdr::ContractEvent> {
    e.events().all().filter_by_contract(id).events().to_vec()
}

/// The first topic of each event the sale emitted, i.e. the names.
fn names(e: &Env, id: &Address) -> Vec<Symbol> {
    emitted(e, id)
        .iter()
        .map(|ev| {
            let xdr::ContractEventBody::V0(body) = &ev.body;
            let topic = soroban_sdk::Val::try_from_val(e, &body.topics[0]).unwrap();
            Symbol::try_from_val(e, &topic).unwrap()
        })
        .collect()
}

fn syms(e: &Env, names: &[&str]) -> Vec<Symbol> {
    names.iter().map(|n| Symbol::new(e, n)).collect()
}

fn sym(e: &Env, s: &str) -> Symbol {
    Symbol::new(e, s)
}

fn changed(e: &Env, id: &Address, from: &str, to: &str) -> xdr::ContractEvent {
    StateChanged {
        from: sym(e, from),
        to: sym(e, to),
    }
    .to_xdr(e, id)
}

#[test]
fn construction_emits_configured() {
    let s = setup_preparing();
    // Registered here, not through the fixture, because the test
    // environment keeps the events of the latest invocation only.
    let contracts = s.client.contracts();
    let id = s.e.register(
        crate::OfferSale,
        (
            crate::PowerHolders {
                owner: s.owner.clone(),
                pauser: s.platform.clone(),
                upgrader: s.platform.clone(),
            },
            contracts.clone(),
            super::config(),
        ),
    );
    let ev = Configured {
        owner: s.owner.clone(),
        pauser: s.platform.clone(),
        upgrader: s.platform.clone(),
        token: contracts.token,
        payment_asset: contracts.payment_asset,
        allowlist: contracts.allowlist,
        price: PRICE,
        cooling_off_secs: super::COOLING_OFF,
        min_success_percent: super::MIN_PERCENT,
    };
    assert_eq!(emitted(&s.e, &id), [ev.to_xdr(&s.e, &id)]);
}

#[test]
fn activate_emits_the_transition_then_activated() {
    let s = setup_preparing();
    s.client.activate();
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            changed(&s.e, &s.id, "preparing", "active"),
            Activated { supply: SUPPLY }.to_xdr(&s.e, &s.id)
        ]
    );
}

#[test]
fn buy_emits_purchased_with_the_totals_after() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.client.buy(&s.alice, &(5 * PRICE));
    let ev = Purchased {
        investor: s.alice.clone(),
        paid: 5 * PRICE,
        tokens: 5,
        receipt: 1,
        at: T0,
        reserved_paid: 15 * PRICE,
        reserved_tokens: 15,
        raised: 15 * PRICE,
        sold_tokens: 15,
    };
    assert_eq!(emitted(&s.e, &s.id), [ev.to_xdr(&s.e, &s.id)]);
}

#[test]
fn the_purchase_that_sells_out_emits_purchased_closed_and_the_transition() {
    let s = setup();
    s.client.buy(&s.alice, &(SUPPLY * PRICE));
    assert_eq!(
        names(&s.e, &s.id),
        syms(&s.e, &["purchased", "closed", "state_changed"])
    );
    let evs = emitted(&s.e, &s.id);
    let closed = Closed {
        reason: sym(&s.e, "sold_out"),
        raised: SUPPLY * PRICE,
        sold_tokens: SUPPLY,
    };
    assert_eq!(evs[1], closed.to_xdr(&s.e, &s.id));
    assert_eq!(evs[2], changed(&s.e, &s.id, "active", "closing"));
}

#[test]
fn close_reopen_and_cancel_emit_their_event_and_the_transition() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.client.close(&s.owner);
    let closed = Closed {
        reason: sym(&s.e, "owner"),
        raised: 10 * PRICE,
        sold_tokens: 10,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            closed.to_xdr(&s.e, &s.id),
            changed(&s.e, &s.id, "active", "closing")
        ]
    );
    s.client.reopen(&s.owner);
    let reopened = Reopened {
        reason: sym(&s.e, "owner"),
        raised: 10 * PRICE,
        sold_tokens: 10,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            changed(&s.e, &s.id, "closing", "active"),
            reopened.to_xdr(&s.e, &s.id)
        ]
    );
    s.client.cancel(&s.owner);
    let cancelled = Cancelled {
        raised: 10 * PRICE,
        sold_tokens: 10,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            changed(&s.e, &s.id, "active", "failed"),
            cancelled.to_xdr(&s.e, &s.id)
        ]
    );
}

#[test]
fn a_refund_below_the_minimum_in_closing_emits_refunded_then_the_reopen() {
    let s = setup();
    s.buy(&s.alice, 700);
    s.buy(&s.bob, 60);
    s.client.close(&s.owner);
    s.client.cooling_off_refund(&s.bob);
    let refunded = Refunded {
        investor: s.bob.clone(),
        paid: 60 * PRICE,
        tokens: 60,
        reason: sym(&s.e, "cooling_off"),
        raised: 700 * PRICE,
        sold_tokens: 700,
    };
    let reopened = Reopened {
        reason: sym(&s.e, "below_minimum"),
        raised: 700 * PRICE,
        sold_tokens: 700,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            refunded.to_xdr(&s.e, &s.id),
            changed(&s.e, &s.id, "closing", "active"),
            reopened.to_xdr(&s.e, &s.id)
        ]
    );
}

#[test]
fn finalize_emits_finalized_with_the_burn_then_the_transition() {
    let s = setup();
    s.buy(&s.alice, 750);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    let finalized = Finalized {
        raised: 750 * PRICE,
        sold_tokens: 750,
        burned: 250,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            finalized.to_xdr(&s.e, &s.id),
            changed(&s.e, &s.id, "closing", "successful")
        ]
    );
}

#[test]
fn finalize_below_the_minimum_emits_finalized_failed_then_the_transition() {
    let s = setup();
    s.buy(&s.alice, 100);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    let failed = FinalizedFailed {
        raised: 100 * PRICE,
        sold_tokens: 100,
        min_success_percent: super::MIN_PERCENT,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            failed.to_xdr(&s.e, &s.id),
            changed(&s.e, &s.id, "closing", "failed")
        ]
    );
}

#[test]
fn release_emits_released_per_investor_served() {
    let s = setup();
    s.buy(&s.alice, 700);
    s.buy(&s.bob, 50);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    s.client
        .release(&s.investors(&[&s.alice, &Address::generate(&s.e), &s.bob]));
    let released = |investor: &Address, tokens: i128| {
        Released {
            investor: investor.clone(),
            paid: tokens * PRICE,
            tokens,
        }
        .to_xdr(&s.e, &s.id)
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [released(&s.alice, 700), released(&s.bob, 50)]
    );
}

#[test]
fn cooling_off_refund_emits_refunded_with_its_reason_and_the_totals_after() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.buy(&s.bob, 20);
    s.client.cooling_off_refund(&s.alice);
    let refunded = Refunded {
        investor: s.alice.clone(),
        paid: 10 * PRICE,
        tokens: 10,
        reason: sym(&s.e, "cooling_off"),
        raised: 20 * PRICE,
        sold_tokens: 20,
    };
    assert_eq!(emitted(&s.e, &s.id), [refunded.to_xdr(&s.e, &s.id)]);
}

#[test]
fn refund_emits_refunded_per_investor_with_the_totals_falling() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.buy(&s.bob, 20);
    s.client.cancel(&s.owner);
    s.client.refund(&s.investors(&[&s.alice, &s.bob]));
    let refunded = |investor: &Address, tokens: i128, raised_after: i128, sold_after: i128| {
        Refunded {
            investor: investor.clone(),
            paid: tokens * PRICE,
            tokens,
            reason: sym(&s.e, "failed"),
            raised: raised_after * PRICE,
            sold_tokens: sold_after,
        }
        .to_xdr(&s.e, &s.id)
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [refunded(&s.alice, 10, 20, 20), refunded(&s.bob, 20, 0, 0)]
    );
}

#[test]
fn withdrawals_and_pause_record_their_leg_and_caller() {
    let s = setup();
    s.successful();
    let issuer = Address::generate(&s.e);
    s.client
        .withdraw_payment(&s.payouts(&[(&issuer, 7), (&s.owner, 3)]), &s.owner);
    let leg = |recipient: &Address, amount: i128| {
        PaymentWithdrawn {
            recipient: recipient.clone(),
            amount,
        }
        .to_xdr(&s.e, &s.id)
    };
    assert_eq!(emitted(&s.e, &s.id), [leg(&issuer, 7), leg(&s.owner, 3)]);
    s.client.pause(&s.platform);
    let paused = PauseChanged {
        caller: s.platform.clone(),
        paused: true,
    };
    assert_eq!(names(&s.e, &s.id), syms(&s.e, &["paused", "pause_changed"]));
    assert_eq!(emitted(&s.e, &s.id)[1], paused.to_xdr(&s.e, &s.id));

    let s = setup();
    s.failed();
    s.client.withdraw_tokens(&s.owner, &5, &s.owner);
    let taken = TokensWithdrawn {
        to: s.owner.clone(),
        amount: 5,
    };
    assert_eq!(emitted(&s.e, &s.id), [taken.to_xdr(&s.e, &s.id)]);
}

/// The XDR size of each per-investor event, which the batch arithmetic
/// rests on. Asserted so the documents can't drift from the code;
/// `-- --nocapture` prints them.
#[test]
fn event_sizes_match_the_interface_document() {
    use soroban_sdk::xdr::{Limits, WriteXdr};
    let s = setup();
    let size = |ev: xdr::ContractEvent| ev.to_xdr(Limits::none()).unwrap().len();
    let released = Released {
        investor: s.alice.clone(),
        paid: 1,
        tokens: 1,
    };
    let refunded = Refunded {
        investor: s.alice.clone(),
        paid: 1,
        tokens: 1,
        reason: sym(&s.e, "cooling_off"),
        raised: 1,
        sold_tokens: 1,
    };
    let leg = PaymentWithdrawn {
        recipient: s.alice.clone(),
        amount: 1,
    };
    let purchased = Purchased {
        investor: s.alice.clone(),
        paid: 1,
        tokens: 1,
        receipt: 1,
        at: 1,
        reserved_paid: 1,
        reserved_tokens: 1,
        raised: 1,
        sold_tokens: 1,
    };
    let failed = Refunded {
        investor: s.alice.clone(),
        paid: 1,
        tokens: 1,
        reason: sym(&s.e, "failed"),
        raised: 1,
        sold_tokens: 1,
    };
    let sizes = (
        size(released.to_xdr(&s.e, &s.id)),
        size(refunded.to_xdr(&s.e, &s.id)),
        size(failed.to_xdr(&s.e, &s.id)),
        size(leg.to_xdr(&s.e, &s.id)),
        size(purchased.to_xdr(&s.e, &s.id)),
    );
    std::println!(
        "released {} B, refunded {} B (cooling_off) / {} B (failed), payment_withdrawn {} B, purchased {} B",
        sizes.0,
        sizes.1,
        sizes.2,
        sizes.3,
        sizes.4
    );
    assert_eq!(sizes, (188, 300, 296, 168, 404));
}

/// When the allowlist refuses the restore, the refund still pays, and
/// `restore_skipped` tells the backend which room to repair.
#[test]
fn a_refused_restore_emits_restore_skipped_before_refunded() {
    let s = setup();
    s.buy(&s.alice, 10);
    s.buy(&s.alice, 5);
    s.client.cancel(&s.owner);
    s.allowlist.restore_all(&s.alice, &s.id);
    s.client.refund(&s.investors(&[&s.alice]));
    let skipped = RestoreSkipped {
        investor: s.alice.clone(),
        purchases: 2,
    };
    let refunded = Refunded {
        investor: s.alice.clone(),
        paid: 15 * PRICE,
        tokens: 15,
        reason: sym(&s.e, "failed"),
        raised: 0,
        sold_tokens: 0,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [skipped.to_xdr(&s.e, &s.id), refunded.to_xdr(&s.e, &s.id)]
    );
}
