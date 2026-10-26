//! The edges of every limit: the price multiple, the last token, the
//! purchases per investor, the batch sizes and the event budget they rest
//! on, and a payment asset that keeps a fee.
extern crate std;
use soroban_sdk::{testutils::Address as _, Address, Vec};

use super::{err, setup, setup_with_bonus_token, setup_with_fee_token, Setup, PRICE, SUPPLY};
use crate::{
    OfferSaleError, SaleState, MAX_PAYOUTS, MAX_PURCHASES, MAX_REFUND, MAX_RELEASE, REFUND_UNITS,
};

#[test]
fn a_purchase_must_be_a_whole_number_of_tokens() {
    let s = setup();
    for amount in [PRICE + 1, PRICE - 1, 2 * PRICE + 5] {
        assert_eq!(
            err(s.client.try_buy(&s.alice, &amount)),
            OfferSaleError::NotWholeTokens.into()
        );
    }
    for amount in [0, -1, -PRICE] {
        assert_eq!(
            err(s.client.try_buy(&s.alice, &amount)),
            OfferSaleError::NonPositiveAmount.into()
        );
    }
    assert!(s.client.reservation(&s.alice).is_none());
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
}

#[test]
fn preview_tokens_agrees_with_buy() {
    let s = setup();
    assert_eq!(s.client.preview_tokens(&(7 * PRICE)), 7);
    assert_eq!(
        err(s.client.try_preview_tokens(&(7 * PRICE + 1))),
        OfferSaleError::NotWholeTokens.into()
    );
}

#[test]
fn the_last_token_sells_and_one_more_does_not() {
    let s = setup();
    s.buy(&s.alice, SUPPLY - 1);
    assert_eq!(
        err(s.try_buy(&s.bob, 2)),
        OfferSaleError::ExceedsSupply.into()
    );
    s.buy(&s.bob, 1);
    assert_eq!(s.client.state(), SaleState::Closing);
}

#[test]
fn an_investor_holds_at_most_max_purchases_until_a_refund_frees_them() {
    let s = setup();
    assert_eq!(MAX_PURCHASES, 20);
    for _ in 0..MAX_PURCHASES {
        s.buy(&s.alice, 1);
    }
    assert_eq!(
        err(s.try_buy(&s.alice, 1)),
        OfferSaleError::TooManyPurchases.into()
    );
    assert_eq!(s.client.reservation(&s.alice).unwrap().purchases.len(), 20);
    s.client.cooling_off_refund(&s.alice);
    s.buy(&s.alice, 1);
    assert_eq!(s.client.reservation(&s.alice).unwrap().tokens, 1);
}

/// The test host enforces the limits its SDK ships as mainnet's (50 write
/// entries, 100 footprint entries); the interface document quotes 200 and
/// 400, read from testnet on 2026-10-20 (R7), and the batch arithmetic
/// rests on those. So the measuring tests lift the host's check and assert
/// the documented limits themselves.
const WRITE_ENTRIES: u32 = 200;
const FOOTPRINT_ENTRIES: u32 = 400;
const EVENT_BYTES: u32 = 16_384;

fn network_limits(s: &Setup) {
    s.e.cost_estimate().budget().reset_unlimited();
    s.e.cost_estimate().disable_resource_limits();
}

fn strangers(s: &Setup, n: u32) -> Vec<Address> {
    let mut v = Vec::new(&s.e);
    for _ in 0..n {
        v.push_back(Address::generate(&s.e));
    }
    v
}

#[test]
fn release_takes_thirty_investors_and_not_thirty_one() {
    let s = setup();
    s.successful();
    assert_eq!(MAX_RELEASE, 30);
    assert_eq!(
        err(s.client.try_release(&strangers(&s, 31))),
        OfferSaleError::BatchTooLarge.into()
    );
    s.client.release(&strangers(&s, 30));
}

#[test]
fn withdraw_payment_takes_thirty_legs_and_not_thirty_one() {
    let s = setup();
    s.successful();
    assert_eq!(MAX_PAYOUTS, 30);
    let mut legs = std::vec::Vec::new();
    let to = Address::generate(&s.e);
    for _ in 0..31 {
        legs.push((&to, 1));
    }
    assert_eq!(
        err(s.client.try_withdraw_payment(&s.payouts(&legs), &s.owner)),
        OfferSaleError::BatchTooLarge.into()
    );
    legs.pop();
    s.client.withdraw_payment(&s.payouts(&legs), &s.owner);
    assert_eq!(s.usdc.balance(&to), 30);
}

#[test]
fn refund_takes_sixteen_investors_and_not_seventeen() {
    let s = setup();
    s.failed();
    assert_eq!(MAX_REFUND, 16);
    assert_eq!(
        err(s.client.try_refund(&strangers(&s, 17))),
        OfferSaleError::BatchTooLarge.into()
    );
    s.client.refund(&strangers(&s, 16));
}

/// The finer bound: an investor costs 2 units, each live purchase 1, and
/// a call may spend `REFUND_UNITS`. Checked before any transfer, so an
/// over-budget batch moves nothing.
#[test]
fn refund_spends_at_most_the_event_budget() {
    let s = setup();
    assert_eq!(REFUND_UNITS, 48);
    let carol = Address::generate(&s.e);
    let dave = Address::generate(&s.e);
    for who in [&carol, &dave] {
        s.allow(who, PRICE);
        s.fund(who, PRICE);
    }
    for _ in 0..20 {
        s.buy(&s.alice, 1);
        s.buy(&s.bob, 1);
    }
    s.buy(&carol, 1);
    s.buy(&dave, 1);
    s.client.cancel(&s.owner);
    // 22 + 22 + 3 + 3 = 50 units: over.
    let r = s
        .client
        .try_refund(&s.investors(&[&s.alice, &s.bob, &carol, &dave]));
    assert_eq!(err(r), OfferSaleError::BatchTooLarge.into());
    assert_eq!(s.usdc.balance(&s.alice), (SUPPLY - 20) * PRICE);
    // 22 + 22 + 3 = 47 units: fits.
    s.client.refund(&s.investors(&[&s.alice, &s.bob, &carol]));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    s.client.refund(&s.investors(&[&dave]));
    assert_eq!(s.usdc.balance(&dave), PRICE);
}

/// `MAX_REFUND` one-purchase investors in one call: the events stay under
/// the budget the constants promise. The payment asset here is a test SAC
/// whose `transfer` encodes 4 B shorter than USDC's (its asset code is
/// shorter), so the real figure is 16 × 4 B higher. `-- --nocapture`
/// prints the numbers.
#[test]
fn a_full_refund_stays_under_the_event_budget() {
    let s = setup();
    let mut investors = std::vec::Vec::new();
    for _ in 0..MAX_REFUND {
        let who = Address::generate(&s.e);
        s.allow(&who, PRICE);
        s.fund(&who, PRICE);
        s.buy(&who, 1);
        investors.push(who);
    }
    s.client.cancel(&s.owner);
    let list: std::vec::Vec<&Address> = investors.iter().collect();
    let list = s.investors(&list);
    network_limits(&s);
    s.client.refund(&list);
    let used = s.e.cost_estimate().resources();
    std::println!(
        "refund x{MAX_REFUND}: {} write entries, {} B of events",
        used.write_entries,
        used.contract_events_size_bytes
    );
    assert!(used.contract_events_size_bytes + 16 * 4 <= REFUND_UNITS * 292);
    assert!(used.write_entries <= WRITE_ENTRIES);
    assert!(used.memory_read_entries <= FOOTPRINT_ENTRIES);
}

#[test]
fn a_full_release_stays_under_the_event_limit() {
    let s = setup();
    let mut investors = std::vec::Vec::new();
    for _ in 0..MAX_RELEASE {
        let who = Address::generate(&s.e);
        s.allow(&who, PRICE);
        s.fund(&who, PRICE);
        s.buy(&who, 1);
        investors.push(who);
    }
    s.buy(&s.alice, 720);
    s.client.close(&s.owner);
    s.past_cooling_off();
    s.client.finalize();
    let list: std::vec::Vec<&Address> = investors.iter().collect();
    let list = s.investors(&list);
    network_limits(&s);
    s.client.release(&list);
    let used = s.e.cost_estimate().resources();
    std::println!(
        "release x{MAX_RELEASE}: {} write entries, {} B of events",
        used.write_entries,
        used.contract_events_size_bytes
    );
    assert_eq!(used.contract_events_size_bytes, 30 * (168 + 188));
    assert!(used.write_entries <= WRITE_ENTRIES);
    assert!(used.memory_read_entries <= FOOTPRINT_ENTRIES);
}

#[test]
fn a_full_payout_stays_under_the_event_limit() {
    let s = setup();
    s.successful();
    let mut legs = std::vec::Vec::new();
    let mut tos = std::vec::Vec::new();
    for _ in 0..MAX_PAYOUTS {
        tos.push(Address::generate(&s.e));
    }
    for to in &tos {
        legs.push((to, PRICE));
    }
    network_limits(&s);
    s.client.withdraw_payment(&s.payouts(&legs), &s.owner);
    let used = s.e.cost_estimate().resources();
    std::println!(
        "withdraw_payment x{MAX_PAYOUTS}: {} write entries, {} B of events",
        used.write_entries,
        used.contract_events_size_bytes
    );
    assert!(used.contract_events_size_bytes + 30 * 4 <= EVENT_BYTES);
    assert!(used.write_entries <= WRITE_ENTRIES);
}

/// The sale credits what arrived, not what was asked, as Base does. A SAC
/// never keeps a fee; a payment asset that does can only buy whole tokens
/// by sending the fee on top.
#[test]
fn fee_on_transfer_credits_what_arrived() {
    let (s, fee) = setup_with_fee_token();
    assert_eq!(
        err(s.client.try_buy(&s.alice, &(10 * PRICE))),
        OfferSaleError::NotWholeTokens.into()
    );
    assert_eq!(fee.balance(&s.alice), SUPPLY * PRICE);
    s.client.buy(&s.alice, &(10 * PRICE + 1));
    let reservation = s.client.reservation(&s.alice).unwrap();
    assert_eq!((reservation.tokens, reservation.paid), (10, 10 * PRICE));
    assert_eq!(fee.balance(&s.alice), SUPPLY * PRICE - 10 * PRICE - 1);
    assert_eq!(fee.balance(&s.id), 10 * PRICE);
    assert_eq!(s.client.totals().raised, 10 * PRICE);
}

#[test]
fn a_transfer_that_delivers_nothing_fails_with_nothing_received() {
    let (s, _fee) = setup_with_fee_token();
    assert_eq!(
        err(s.client.try_buy(&s.alice, &1)),
        OfferSaleError::NothingReceived.into()
    );
}

/// The supply is checked against what arrived, not what was asked: an
/// asset that credits more than it took can't reserve tokens that don't
/// exist.
#[test]
fn an_asset_that_over_credits_cannot_reserve_past_the_supply() {
    let (s, _bonus) = setup_with_bonus_token();
    // 600 tokens asked, 1,200 tokens' worth arrives: over the 1,000 left.
    assert_eq!(
        err(s.client.try_buy(&s.alice, &(600 * PRICE))),
        OfferSaleError::ExceedsSupply.into()
    );
    assert_eq!(s.client.totals().sold_tokens, 0);
    // 400 asked, 800 arrive: fits, and is what gets reserved.
    s.client.buy(&s.alice, &(400 * PRICE));
    assert_eq!(s.client.totals().sold_tokens, 800);
}

/// A cooling-off refund restores all of an investor's live receipts in
/// one allowlist call, so the sale's cap must fit the allowlist's limit.
#[test]
fn the_purchase_cap_fits_one_allowlist_restore() {
    const { assert!(MAX_PURCHASES <= offer_allowlist::MAX_RESTORE) };
}

/// An investor listed twice is served once, so they are counted once
/// against the event budget too.
#[test]
fn a_duplicate_in_a_refund_list_is_not_charged_twice() {
    let s = setup();
    for _ in 0..20 {
        s.buy(&s.alice, 1);
        s.buy(&s.bob, 1);
    }
    s.client.cancel(&s.owner);
    // 22 + 22 = 44 units; counting alice twice would make it 66.
    s.client.refund(&s.investors(&[&s.alice, &s.bob, &s.alice]));
    assert_eq!(s.usdc.balance(&s.alice), SUPPLY * PRICE);
    assert_eq!(s.usdc.balance(&s.bob), SUPPLY * PRICE);
}

#[test]
fn preview_tokens_refuses_a_non_positive_amount() {
    let s = setup();
    for amount in [0, -PRICE] {
        assert_eq!(
            err(s.client.try_preview_tokens(&amount)),
            OfferSaleError::NonPositiveAmount.into()
        );
    }
}

/// A refund gives back the receipts the reservation holds, by number, and
/// nothing else: a receipt live in the allowlist that the reservation no
/// longer holds (left behind by a refused restore) is neither restored twice
/// nor charged against the event budget.
#[test]
fn a_refund_restores_only_the_reservations_own_receipts() {
    let s = setup();
    let bought = s.buy(&s.alice, 10);
    // A receipt the reservation does not hold, as a refused cooling-off
    // restore would leave behind.
    let stray = s.allowlist.consume(&s.alice, &PRICE, &s.id);
    s.client.cancel(&s.owner);
    s.client.refund(&s.investors(&[&s.alice]));
    assert!(s.allowlist.receipt(&s.id, &s.alice, &bought).is_none());
    assert!(s.allowlist.receipt(&s.id, &s.alice, &stray).is_some());
}

/// The minimum check multiplies the supply by a percentage, so `activate`
/// refuses a supply that could overflow it.
#[test]
fn a_supply_over_max_supply_is_refused_at_activation() {
    let s = super::setup_preparing();
    let huge = crate::MAX_SUPPLY + 1;
    s.e.register_at(
        &s.token.address,
        offer_token::OfferToken,
        (
            offer_token::PowerHolders {
                admin: s.token_admin.clone(),
                xfer_admin: s.token_admin.clone(),
                pauser: s.platform.clone(),
                upgrader: s.platform.clone(),
            },
            offer_token::OfferingMetadata {
                name: soroban_sdk::String::from_str(&s.e, "Edificio Aurora"),
                symbol: soroban_sdk::String::from_str(&s.e, "AUR"),
                offer_uri: soroban_sdk::String::from_str(&s.e, "ipfs://offering-v1"),
            },
            huge,
            s.id.clone(),
            Vec::<Address>::new(&s.e),
        ),
    );
    assert!(s.token.total_supply() > crate::MAX_SUPPLY);
    assert_eq!(
        err(s.client.try_activate()),
        OfferSaleError::InvalidConfig.into()
    );
}
