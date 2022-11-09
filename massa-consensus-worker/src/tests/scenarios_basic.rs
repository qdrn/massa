// Copyright (c) 2022 MASSA LABS <info@massa.net>

use super::tools;
use crate::tests::block_factory::BlockFactory;
use massa_consensus_exports::ConsensusConfig;
use massa_hash::Hash;
use massa_models::{block::BlockId, slot::Slot};
use massa_signature::KeyPair;
use massa_storage::Storage;
use serial_test::serial;

#[tokio::test]
#[serial]
#[ignore]
async fn test_old_stale_not_propagated_and_discarded() {
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        t0: 32.into(),
        future_block_processing_max_periods: 50,
        ..ConsensusConfig::default()
    };

    tools::consensus_without_pool_test(
        cfg.clone(),
        async move |protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    selector_controller| {
            let parents: Vec<BlockId> = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .best_parents
                .iter()
                .map(|(b, _p)| *b)
                .collect();

            let mut block_factory =
                BlockFactory::start_block_factory(parents.clone(), protocol_controller);
            block_factory.creator_keypair = staking_keys[0].clone();
            block_factory.slot = Slot::new(1, 0);

            let block_1 = block_factory.create_and_receive_block(true).await;

            block_factory.slot = Slot::new(1, 1);
            block_factory.create_and_receive_block(true).await;

            block_factory.slot = Slot::new(1, 0);
            block_factory.best_parents = vec![block_1.id, parents[0]];
            let block_3 = block_factory.create_and_receive_block(false).await;

            // Old stale block was discarded.
            let status = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status");
            assert_eq!(status.discarded_blocks.len(), 1);
            assert!(status.discarded_blocks.get(&block_3.id).is_some());
            (
                block_factory.take_protocol_controller(),
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
async fn test_block_not_processed_multiple_times() {
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        t0: 500.into(),
        future_block_processing_max_periods: 50,
        ..ConsensusConfig::default()
    };
    let mut storage = Storage::create_root();

    tools::consensus_without_pool_test(
        cfg.clone(),
        async move |protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    selector_controller| {
            let parents: Vec<BlockId> = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .best_parents
                .iter()
                .map(|(b, _p)| *b)
                .collect();

            let mut block_factory =
                BlockFactory::start_block_factory(parents.clone(), protocol_controller);
            block_factory.creator_keypair = staking_keys[0].clone();
            block_factory.slot = Slot::new(1, 0);
            let block_1 = block_factory.create_and_receive_block(true).await;

            // Send it again, it should not be propagated.
            storage.store_block(block_1.clone());
            block_factory
                .receive_block(
                    false,
                    block_1.id,
                    block_1.content.header.content.slot,
                    storage.clone(),
                )
                .await;

            // Send it again, it should not be propagated.
            block_factory
                .receive_block(
                    false,
                    block_1.id,
                    block_1.content.header.content.slot,
                    storage.clone(),
                )
                .await;

            // Block was not discarded.
            let status = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status");
            assert_eq!(status.discarded_blocks.len(), 0);
            (
                block_factory.take_protocol_controller(),
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
async fn test_queuing() {
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        future_block_processing_max_periods: 50,
        t0: 1000.into(),
        ..ConsensusConfig::default()
    };

    tools::consensus_without_pool_test(
        cfg.clone(),
        async move |protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    selector_controller| {
            let parents: Vec<BlockId> = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .best_parents
                .iter()
                .map(|(b, _p)| *b)
                .collect();

            let mut block_factory =
                BlockFactory::start_block_factory(parents.clone(), protocol_controller);
            block_factory.creator_keypair = staking_keys[0].clone();
            block_factory.slot = Slot::new(3, 0);

            let block_1 = block_factory.create_and_receive_block(false).await;

            block_factory.slot = Slot::new(4, 0);
            block_factory.best_parents = vec![block_1.id, parents[1]];

            block_factory.create_and_receive_block(false).await;

            // Blocks were queued, not discarded.
            let status = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status");
            assert_eq!(status.discarded_blocks.len(), 0);
            (
                block_factory.take_protocol_controller(),
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
async fn test_double_staking_does_not_propagate() {
    let staking_keys: Vec<KeyPair> = (0..1).map(|_| KeyPair::generate()).collect();
    let cfg = ConsensusConfig {
        future_block_processing_max_periods: 50,
        t0: 1000.into(),
        ..ConsensusConfig::default()
    };

    let mut storage = Storage::create_root();

    tools::consensus_without_pool_test(
        cfg.clone(),
        async move |protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    selector_controller| {
            let parents: Vec<BlockId> = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .best_parents
                .iter()
                .map(|(b, _p)| *b)
                .collect();

            let mut block_factory =
                BlockFactory::start_block_factory(parents.clone(), protocol_controller);
            block_factory.creator_keypair = staking_keys[0].clone();
            block_factory.slot = Slot::new(1, 0);
            let mut block_1 = block_factory.create_and_receive_block(true).await;

            // Same creator, same slot, different block
            block_1.content.header.content.operation_merkle_root =
                Hash::compute_from("hello world".as_bytes());
            let block = block_factory.sign_header(block_1.content.header.content);

            // Note: currently does propagate, see #190.
            storage.store_block(block.clone());
            block_factory
                .receive_block(
                    true,
                    block.id,
                    block.content.header.content.slot,
                    storage.clone(),
                )
                .await;

            // Block was not discarded.
            let status = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status");
            assert_eq!(status.discarded_blocks.len(), 0);
            (
                block_factory.take_protocol_controller(),
                consensus_command_sender,
                consensus_event_receiver,
                selector_controller,
            )
        },
    )
    .await;
}
