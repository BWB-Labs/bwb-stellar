//! Each entrypoint emits exactly the events the distributor document
//! lists, in the documented order.
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    xdr, Address, Env, Event as _, TryFromVal,
};

use super::{setup, USDC};
use crate::events::{Configured, Distributed, DistributionCompleted, PauseChanged};

extern crate std;
use std::vec::Vec;

/// The events the distributor emitted in the last invocation.
fn emitted(e: &Env, id: &Address) -> Vec<xdr::ContractEvent> {
    e.events().all().filter_by_contract(id).events().to_vec()
}

#[test]
fn construction_emits_configured() {
    let s = setup();
    // Registered here, not through the fixture, because the test
    // environment keeps the events of the latest invocation only.
    let id = s.e.register(crate::OfferDistributor, (s.client.powers(),));
    let configured = Configured {
        owner: s.owner.clone(),
        pauser: s.platform.clone(),
        upgrader: s.platform.clone(),
    };
    assert_eq!(emitted(&s.e, &id), [configured.to_xdr(&s.e, &id)]);
}

#[test]
fn a_distribution_emits_distributed_per_payout_then_the_summary() {
    let s = setup();
    s.distribute(7, &[(&s.alice, 3 * USDC), (&s.bob, 2 * USDC)]);
    let to_alice = Distributed {
        id: s.dist(7),
        recipient: s.alice.clone(),
        asset: s.usdc.address.clone(),
        amount: 3 * USDC,
    };
    let to_bob = Distributed {
        id: s.dist(7),
        recipient: s.bob.clone(),
        asset: s.usdc.address.clone(),
        amount: 2 * USDC,
    };
    let completed = DistributionCompleted {
        id: s.dist(7),
        asset: s.usdc.address.clone(),
        total: 5 * USDC,
        count: 2,
    };
    assert_eq!(
        emitted(&s.e, &s.id),
        [
            to_alice.to_xdr(&s.e, &s.id),
            to_bob.to_xdr(&s.e, &s.id),
            completed.to_xdr(&s.e, &s.id)
        ]
    );
}

/// The asset's own `transfer` comes before each `distributed`, so an
/// indexer reading the transaction in order sees the money move first.
#[test]
fn each_distributed_follows_the_assets_transfer() {
    let s = setup();
    s.distribute(7, &[(&s.alice, USDC), (&s.bob, USDC)]);
    let all = s.e.events().all();
    let emitters: Vec<Address> = all
        .events()
        .iter()
        .map(|ev| {
            let emitter = xdr::ScAddress::Contract(ev.contract_id.clone().unwrap());
            Address::try_from_val(&s.e, &emitter).unwrap()
        })
        .collect();
    let usdc = s.usdc.address.clone();
    let me = s.id.clone();
    assert_eq!(emitters, [usdc.clone(), me.clone(), usdc, me.clone(), me]);
}

#[test]
fn a_refused_distribution_emits_nothing() {
    let s = setup();
    assert!(s.try_distribute(7, &[(&s.alice, 0)]).is_err());
    assert!(emitted(&s.e, &s.id).is_empty());
}

#[test]
fn pause_and_unpause_emit_pause_changed_with_the_caller() {
    let s = setup();
    s.client.pause(&s.platform);
    let paused = PauseChanged {
        caller: s.platform.clone(),
        paused: true,
    };
    assert_eq!(
        emitted(&s.e, &s.id).last().unwrap(),
        &paused.to_xdr(&s.e, &s.id)
    );
    s.client.unpause(&s.owner);
    let resumed = PauseChanged {
        caller: s.owner.clone(),
        paused: false,
    };
    assert_eq!(
        emitted(&s.e, &s.id).last().unwrap(),
        &resumed.to_xdr(&s.e, &s.id)
    );
}

/// The XDR size of the per-payout event and of the summary, which the
/// batch arithmetic rests on. Asserted so the documents can't drift from
/// the code; `-- --nocapture` prints them. A classic account is the
/// largest recipient.
#[test]
fn event_sizes_match_the_distributor_document() {
    use soroban_sdk::xdr::{Limits, WriteXdr};
    let s = setup();
    let size = |ev: xdr::ContractEvent| ev.to_xdr(Limits::none()).unwrap().len();
    let account = s.create_account_with_trustline(1);
    let contract = Address::generate(&s.e);
    let to = |recipient: &Address| Distributed {
        id: s.dist(7),
        recipient: recipient.clone(),
        asset: s.usdc.address.clone(),
        amount: 1,
    };
    let completed = DistributionCompleted {
        id: s.dist(7),
        asset: s.usdc.address.clone(),
        total: 1,
        count: 1,
    };
    let sizes = (
        size(to(&account).to_xdr(&s.e, &s.id)),
        size(to(&contract).to_xdr(&s.e, &s.id)),
        size(completed.to_xdr(&s.e, &s.id)),
    );
    std::println!(
        "distributed {} B (account) / {} B (contract), distribution_completed {} B",
        sizes.0,
        sizes.1,
        sizes.2
    );
    assert_eq!(sizes, (260, 256, 252));
}
