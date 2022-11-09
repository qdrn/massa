// Copyright (c) 2022 MASSA LABS <info@massa.net>

use super::tools::*;
use massa_consensus_exports::ConsensusConfig;

use massa_hash::Hash;
use massa_models::slot::Slot;
use massa_signature::KeyPair;
use massa_storage::Storage;
use massa_time::MassaTime;
use serial_test::serial;

#[tokio::test]
#[serial]
#[ignore]
async fn test_queueing() {
    // setup logging
    // stderrlog::new()
    //     .verbosity(3)
    //     .timestamp(stderrlog::Timestamp::Millisecond)
    //     .init()
    //     .unwrap();
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        future_block_processing_max_periods: 50,
        // to avoid timing problems for blocks in the future
        genesis_timestamp: MassaTime::now(0)
            .unwrap()
            .saturating_sub(MassaTime::from_millis(32000).checked_mul(1000).unwrap()),
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

            // * create 30 normal blocks in each thread: in slot 1 they have genesis parents, in slot 2 they have slot 1 parents
            // create a valid block for slot 1
            let mut valid_hasht0 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 0),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // create a valid block on the other thread.
            let mut valid_hasht1 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 1),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // and loop for the 29 other blocks
            for i in 0..29 {
                valid_hasht0 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 2, 0),
                    vec![valid_hasht0, valid_hasht1],
                    true,
                    false,
                    &staking_keys[0],
                )
                .await;

                // create a valid block on the other thread.
                valid_hasht1 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 2, 1),
                    vec![valid_hasht0, valid_hasht1],
                    true,
                    false,
                    &staking_keys[0],
                )
                .await;
            }

            let missed_block = create_block(
                &cfg,
                Slot::new(32, 0),
                vec![valid_hasht0, valid_hasht1],
                &staking_keys[0],
            );

            // create 1 block in thread 0 slot 33 with missed block as parent
            valid_hasht0 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(33, 0),
                vec![missed_block.id, valid_hasht1],
                false,
                false,
                &staking_keys[0],
            )
            .await;

            // and loop again for the 99 other blocks
            for i in 0..30 {
                valid_hasht0 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 34, 0),
                    vec![valid_hasht0, valid_hasht1],
                    false,
                    false,
                    &staking_keys[0],
                )
                .await;

                // create a valid block on the other thread.
                valid_hasht1 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 34, 1),
                    vec![valid_hasht0, valid_hasht1],
                    false,
                    false,
                    &staking_keys[0],
                )
                .await;
            }
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
async fn test_doubles() {
    // setup logging
    // stderrlog::new()
    //     .verbosity(3)
    //     .timestamp(stderrlog::Timestamp::Millisecond)
    //     .init()
    //     .unwrap();
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        future_block_processing_max_periods: 50,
        // to avoid timing problems for blocks in the future
        genesis_timestamp: MassaTime::now(0)
            .unwrap()
            .saturating_sub(MassaTime::from_millis(32000).checked_mul(1000).unwrap()),
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

            // * create 40 normal blocks in each thread: in slot 1 they have genesis parents, in slot 2 they have slot 1 parents
            // create a valid block for slot 1
            let mut valid_hasht0 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 0),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // create a valid block on the other thread.
            let mut valid_hasht1 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 1),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // and loop for the 39 other blocks
            for i in 0..39 {
                valid_hasht0 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 2, 0),
                    vec![valid_hasht0, valid_hasht1],
                    true,
                    false,
                    &staking_keys[0],
                )
                .await;

                // create a valid block on the other thread.
                valid_hasht1 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 2, 1),
                    vec![valid_hasht0, valid_hasht1],
                    true,
                    false,
                    &staking_keys[0],
                )
                .await;
            }

            // create 1 block in thread 0 slot 41 with missed block as parent
            create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(41, 0),
                vec![valid_hasht0, valid_hasht1],
                true,
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
async fn test_double_staking() {
    // setup logging
    // stderrlog::new()
    //     .verbosity(3)
    //     .timestamp(stderrlog::Timestamp::Millisecond)
    //     .init()
    //     .unwrap();

    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        future_block_processing_max_periods: 50,
        // to avoid timing problems for blocks in the future
        genesis_timestamp: MassaTime::now(0)
            .unwrap()
            .saturating_sub(MassaTime::from_millis(32000).checked_mul(1000).unwrap()),
        ..ConsensusConfig::default()
    };
    let mut storage = Storage::create_root();

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

            // * create 40 normal blocks in each thread: in slot 1 they have genesis parents, in slot 2 they have slot 1 parents
            // create a valid block for slot 1
            let mut valid_hasht0 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 0),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // create a valid block on the other thread.
            let mut valid_hasht1 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 1),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // and loop for the 39 other blocks
            for i in 0..39 {
                valid_hasht0 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 2, 0),
                    vec![valid_hasht0, valid_hasht1],
                    true,
                    false,
                    &staking_keys[0],
                )
                .await;

                // create a valid block on the other thread.
                valid_hasht1 = create_and_test_block(
                    &mut protocol_controller,
                    &cfg,
                    Slot::new(i + 2, 1),
                    vec![valid_hasht0, valid_hasht1],
                    true,
                    false,
                    &staking_keys[0],
                )
                .await;
            }

            // same creator same slot, different block
            let operation_merkle_root = Hash::compute_from("42".as_bytes());
            let block_1 = create_block_with_merkle_root(
                &cfg,
                operation_merkle_root,
                Slot::new(41, 0),
                vec![valid_hasht0, valid_hasht1],
                &staking_keys[0],
            );
            storage.store_block(block_1.clone());
            propagate_block(
                &mut protocol_controller,
                block_1.id,
                block_1.content.header.content.slot,
                storage.clone(),
                true,
                150,
            )
            .await;

            let operation_merkle_root =
                Hash::compute_from("so long and thanks for all the fish".as_bytes());
            let block_2 = create_block_with_merkle_root(
                &cfg,
                operation_merkle_root,
                Slot::new(41, 0),
                vec![valid_hasht0, valid_hasht1],
                &staking_keys[0],
            );
            storage.store_block(block_2.clone());
            propagate_block(
                &mut protocol_controller,
                block_2.id,
                block_2.content.header.content.slot,
                storage.clone(),
                true,
                150,
            )
            .await;

            let graph = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .unwrap();
            let cliques_1 = get_cliques(&graph, block_1.id);
            let cliques_2 = get_cliques(&graph, block_2.id);
            assert!(cliques_1.is_disjoint(&cliques_2));
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
async fn test_test_parents() {
    // // setup logging
    // stderrlog::new()
    //     .verbosity(4)
    //     .timestamp(stderrlog::Timestamp::Millisecond)
    //     .init()
    //     .unwrap();

    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        future_block_processing_max_periods: 50,
        // to avoid timing problems for blocks in the future
        genesis_timestamp: MassaTime::now(0)
            .unwrap()
            .saturating_sub(MassaTime::from_millis(32000).checked_mul(1000).unwrap()),
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

            // * create 2 normal blocks in each thread: in slot 1 they have genesis parents, in slot 2 they have slot 1 parents
            // create a valid block for slot 1
            let valid_hasht0s1 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 0),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // create a valid block on the other thread.
            let valid_hasht1s1 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(1, 1),
                genesis_hashes.clone(),
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // create a valid block for slot 2
            let valid_hasht0s2 = create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(2, 0),
                vec![valid_hasht0s1, valid_hasht1s1],
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // create a valid block on the other thread.
            create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(2, 1),
                vec![valid_hasht0s1, valid_hasht1s1],
                true,
                false,
                &staking_keys[0],
            )
            .await;

            // * create 1 block in t0s3 with parents (t0s2, t1s0)
            // create a valid block for slot 2
            create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(3, 0),
                vec![valid_hasht0s2, genesis_hashes[1usize]],
                false,
                false,
                &staking_keys[0],
            )
            .await;

            // * create 1 block in t1s3 with parents (t0s0, t0s0)
            create_and_test_block(
                &mut protocol_controller,
                &cfg,
                Slot::new(3, 1),
                vec![genesis_hashes[0usize], genesis_hashes[0usize]],
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
