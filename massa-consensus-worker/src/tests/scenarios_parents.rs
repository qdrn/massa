// Copyright (c) 2022 MASSA LABS <info@massa.net>
use super::tools::*;
use massa_consensus_exports::ConsensusConfig;

use massa_models::slot::Slot;
use massa_signature::KeyPair;
use serial_test::serial;

#[tokio::test]
#[serial]
#[ignore]
async fn test_parent_in_the_future() {
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
            let genesis_hashes = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            // Parent, in the future.
            let t0s1 = create_block(
                &cfg,
                Slot::new(4, 0),
                genesis_hashes.clone(),
                &staking_keys[0],
            );

            let _ = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(5, 0),
                vec![t0s1.id],
                false,
                false,
                &staking_keys[0],
            )
            .await;
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
async fn test_parents() {
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
            let genesis_hashes = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            // generate two normal blocks in each thread
            let hasht1s1 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 0),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            let _ = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 1),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            let _ = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(3, 0),
                vec![hasht1s1, genesis_hashes[0]],
                false,
                false,
                &staking_keys[0],
            )
            .await;
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
async fn test_parents_in_incompatible_cliques() {
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
            let genesis_hashes = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            let hasht0s1 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 0),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            let hasht0s2 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(2, 0),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // from that point we have two incompatible clique

            let _ = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 1),
                vec![hasht0s1, genesis_hashes[1]],
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // Block with incompatible parents.
            let _ = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(2, 1),
                vec![hasht0s1, hasht0s2],
                false,
                false,
                &staking_keys[0],
            )
            .await;
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
