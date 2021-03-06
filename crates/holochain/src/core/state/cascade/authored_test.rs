use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use holo_hash::EntryHash;
use holochain_types::Entry;
use holochain_wasm_test_utils::TestWasm;

use crate::{
    core::state::element_buf::ElementBuf,
    test_utils::{host_fn_api::*, wait_for_integration},
};
use crate::{
    core::state::source_chain::SourceChain, test_utils::conductor_setup::ConductorTestData,
};

/// - Alice commits an entry and it is in their authored store
/// - Bob doesn't have the entry in their authored store
/// - Bob does have the entry in their integrated store
/// - Bob commits the entry and it is now in their authored store
#[tokio::test(threaded_scheduler)]
async fn authored_test() {
    observability::test_run().ok();
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let zomes = vec![TestWasm::Create];
    let conductor_test = ConductorTestData::new(zomes, true).await;
    let ConductorTestData {
        __tmpdir,
        handle,
        alice_call_data,
        bob_call_data,
        ..
    } = conductor_test;
    let bob_call_data = bob_call_data.unwrap();

    let entry = Post("Hi there".into());
    let entry_hash = EntryHash::with_data_sync(&Entry::try_from(entry.clone()).unwrap());
    // 3
    commit_entry(
        &alice_call_data.env,
        alice_call_data.call_data(TestWasm::Create),
        entry.clone().try_into().unwrap(),
        POST_ID,
    )
    .await;

    // Produce and publish these commits
    let mut triggers = handle
        .get_cell_triggers(&alice_call_data.cell_id)
        .await
        .unwrap();
    triggers.produce_dht_ops.trigger();

    // Alice commits the entry
    let alice_source_chain = SourceChain::new(alice_call_data.env.clone().into()).unwrap();
    let alice_authored = alice_source_chain.elements();
    alice_authored
        .get_entry(&entry_hash)
        .unwrap()
        .expect("Alice should have the entry in their authored because they just committed");

    // Integration should have 3 ops in it.
    // Plus another 14 for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 3 + 14;

    wait_for_integration(
        &bob_call_data.env,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    let bob_source_chain = SourceChain::new(bob_call_data.env.clone().into()).unwrap();
    let bob_authored = bob_source_chain.elements();

    // Bob Should not have the entry in their authored table
    assert_eq!(bob_authored.get_entry(&entry_hash).unwrap(), None);

    let bob_integrated_store = ElementBuf::vault(bob_call_data.env.clone().into(), true).unwrap();
    bob_integrated_store
        .get_entry(&entry_hash)
        .unwrap()
        .expect("Bob should have the entry in their integrated store because they received gossip");

    // Now bob commits the entry
    commit_entry(
        &bob_call_data.env,
        bob_call_data.call_data(TestWasm::Create),
        entry.clone().try_into().unwrap(),
        POST_ID,
    )
    .await;

    // Produce and publish these commits
    let mut triggers = handle
        .get_cell_triggers(&bob_call_data.cell_id)
        .await
        .unwrap();
    triggers.produce_dht_ops.trigger();

    let bob_source_chain = SourceChain::new(bob_call_data.env.clone().into()).unwrap();
    let bob_authored = bob_source_chain.elements();
    bob_authored
        .get_entry(&entry_hash)
        .unwrap()
        .expect("Bob should now have the entry in their authored because they committed it");

    ConductorTestData::shutdown_conductor(handle).await;
}
