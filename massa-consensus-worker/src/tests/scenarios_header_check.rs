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
async fn test_consensus_asks_for_block() {
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        t0: 500.into(),
        future_block_processing_max_periods: 50,
        ..ConsensusConfig::default()
    };

    consensus_without_pool_test(
        cfg.clone(),
        async move |mut protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    selector_controller| {
            let genesis_hashes = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            // create test blocks
            let t0s1 = create_block(
                &cfg,
                Slot::new(1, 0),
                genesis_hashes.clone(),
                &staking_keys[0],
            );
            // send header for block t0s1
            protocol_controller
                .receive_header(t0s1.content.header.clone())
                .await;

            validate_ask_for_block(&mut protocol_controller, t0s1.id, 1000).await;
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
async fn test_consensus_does_not_ask_for_block() {
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
            let t0s1 = create_block(
                &cfg,
                Slot::new(1 + start_slot, 0),
                genesis_hashes.clone(),
                &staking_keys[0],
            );
            let header = t0s1.content.header.clone();
            let id = t0s1.id;
            // Send the actual block.
            storage.store_block(t0s1);
            protocol_controller
                .receive_block(header.id, header.content.slot, storage.clone())
                .await;

            // block t0s1 is propagated
            let hash_list = vec![id];
            validate_propagate_block_in_list(
                &mut protocol_controller,
                &hash_list,
                3000 + start_slot * 1000,
            )
            .await;

            // Send the hash
            protocol_controller.receive_header(header).await;

            // Consensus should not ask for the block, so the time-out should be hit.
            validate_does_not_ask_for_block(&mut protocol_controller, &id, 10).await;
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
