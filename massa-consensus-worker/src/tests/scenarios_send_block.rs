// Copyright (c) 2022 MASSA LABS <info@massa.net>

// RUST_BACKTRACE=1 cargo test scenarios106 -- --nocapture

use super::tools::*;
use massa_consensus_exports::ConsensusConfig;

use massa_models::slot::Slot;
use massa_signature::KeyPair;
use massa_storage::Storage;
use serial_test::serial;

#[tokio::test]
#[serial]
#[ignore]
async fn test_consensus_sends_block_to_peer_who_asked_for_it() {
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        t0: 32.into(),
        future_block_processing_max_periods: 50,
        ..ConsensusConfig::default()
    };
    let mut storage = Storage::create_root();

    consensus_without_pool_test(
        cfg.clone(),
        async move |mut protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    selector_controller| {
            let start_slot = 3;
            let genesis_hashes = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            // create test blocks
            let slot = Slot::new(1 + start_slot, 0);
            let draw = selector_controller.get_selection(slot).unwrap().producer;
            let creator = get_creator_for_draw(&draw, &staking_keys.clone());
            let t0s1 = create_block(
                &cfg,
                Slot::new(1 + start_slot, 0),
                genesis_hashes.clone(),
                &creator,
            );

            let t0s1_id = t0s1.id;
            let t0s1_slot = t0s1.content.header.content.slot;
            storage.store_block(t0s1);

            // Send the actual block.
            protocol_controller
                .receive_block(t0s1_id, t0s1_slot, storage.clone())
                .await;

            // block t0s1 is propagated
            let hash_list = vec![t0s1_id];
            validate_propagate_block_in_list(
                &mut protocol_controller,
                &hash_list,
                3000 + start_slot * 1000,
            )
            .await;

            // Ask for the block to consensus.
            protocol_controller
                .receive_get_active_blocks(vec![t0s1_id])
                .await;

            // Consensus should respond with results including the block.
            validate_block_found(&mut protocol_controller, &t0s1_id, 100).await;
            (
                protocol_controller,
                consensus_command_sender,
                consensus_event_receiver,
                selector_controller,
            )
        },
    )
    .await;
}

#[tokio::test]
#[serial]
#[ignore]
async fn test_consensus_block_not_found() {
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        t0: 32.into(),
        future_block_processing_max_periods: 50,
        ..ConsensusConfig::default()
    };

    consensus_without_pool_test(
        cfg.clone(),
        async move |mut protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    selector_controller| {
            let start_slot = 3;
            let genesis_hashes = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            // create test blocks
            let t0s1 = create_block(
                &cfg,
                Slot::new(1 + start_slot, 0),
                genesis_hashes.clone(),
                &staking_keys[0],
            );

            // Ask for the block to consensus.
            protocol_controller
                .receive_get_active_blocks(vec![t0s1.id])
                .await;

            // Consensus should not have the block.
            validate_block_not_found(&mut protocol_controller, &t0s1.id, 100).await;
            (
                protocol_controller,
                consensus_command_sender,
                consensus_event_receiver,
                selector_controller,
            )
        },
    )
    .await;
}
