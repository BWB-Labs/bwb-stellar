//! Each entrypoint emits exactly the events the distributor document
//! lists, in the documented order.
use soroban_sdk::Event as _;

use stellar_contract_utils::pausable::{Paused, Unpaused};

use super::{emitted, emitters, setup, SUPPLY, USDC};
use crate::events::{ClaimSkipped, Claimed, Configured, Deposited, PauseChanged};

extern crate std;

#[test]
fn construction_emits_configured() {
    let s = setup();
    // Registered here, not through the fixture, because the test
    // environment keeps the events of the latest invocation only.
    let id = s.e.register(
        crate::OfferHoldingsDistributor,
        (s.client.powers(), s.client.contracts()),
    );
    let configured = Configured {
        owner: s.owner.clone(),
        pauser: s.pauser.clone(),
        upgrader: s.upgrader.clone(),
        token: s.token.address.clone(),
        payment_asset: s.usdc.address.clone(),
    };
    assert_eq!(emitted(&s.e, &id), [configured.to_xdr(&s.e, &id)]);
}

/// The asset's `transfer` comes first, so an indexer reading the
/// transaction in order sees the money arrive before `deposited`.
#[test]
fn a_deposit_emits_the_transfer_then_deposited() {
    let s = setup();
    s.deposit(SUPPLY * USDC);
    let deposited = Deposited {
        amount: SUPPLY * USDC,
    };
    assert_eq!(emitted(&s.e, &s.id), [deposited.to_xdr(&s.e, &s.id)]);
    assert_eq!(emitters(&s.e), [s.usdc.address.clone(), s.id.clone()]);
}

/// A paid holder has no event of the distributor's: the asset's `transfer`
/// is its record. A skipped holder has `claim_skipped`, in list order, and
/// `claimed` closes the call.
#[test]
fn a_claim_emits_transfers_and_skips_in_list_order_then_claimed() {
    let s = setup();
    s.give(&s.alice, 100);
    s.give(&s.bob, 300);
    s.deposit(SUPPLY * USDC);
    s.claim(&[&s.alice, &s.desk, &s.bob, &s.stranger]);
    let skipped = ClaimSkipped {
        holder: s.desk.clone(),
    };
    let claimed = Claimed {
        count: 2,
        total: 400 * USDC,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [skipped.to_xdr(&s.e, &s.id), claimed.to_xdr(&s.e, &s.id)]
    );
    assert_eq!(
        emitters(&s.e),
        [
            s.usdc.address.clone(),
            s.id.clone(),
            s.usdc.address.clone(),
            s.id.clone()
        ]
    );
}

#[test]
fn a_claim_that_pays_nobody_still_emits_claimed() {
    let s = setup();
    s.claim(&[&s.alice]);
    let claimed = Claimed { count: 0, total: 0 };
    assert_eq!(emitted(&s.e, &s.id), [claimed.to_xdr(&s.e, &s.id)]);
}

#[test]
fn pause_and_unpause_emit_open_zeppelins_event_then_pause_changed() {
    let s = setup();
    s.client.pause(&s.pauser);
    let changed = PauseChanged {
        caller: s.pauser.clone(),
        paused: true,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [Paused {}.to_xdr(&s.e, &s.id), changed.to_xdr(&s.e, &s.id)]
    );
    s.client.unpause(&s.owner);
    let changed = PauseChanged {
        caller: s.owner.clone(),
        paused: false,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [Unpaused {}.to_xdr(&s.e, &s.id), changed.to_xdr(&s.e, &s.id)]
    );
}
