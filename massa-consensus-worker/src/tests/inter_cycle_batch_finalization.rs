//! Copyright (c) 2022 MASSA LABS <info@massa.net>

use super::tools::*;
use massa_consensus_exports::ConsensusConfig;
use massa_models::{block::BlockId, slot::Slot};
use massa_signature::KeyPair;
use massa_time::MassaTime;
use serial_test::serial;
use std::{collections::HashSet, str::FromStr};

/// # Context
///
/// Regression test for `https://github.com/massalabs/massa/pull/2433`
///
/// When we have the following block sequence
/// ```
/// 1 thread, periods_per_cycle = 2, delta_f0 = 1, 1 endorsement per block
///
/// cycle 0 | cycle 1 | cycle 2
///  G - B1 - B2 - B3 - B4
/// where G is the genesis block
/// and B4 contains a roll sell operation
/// ```
///
/// And the block `B1` is received AFTER `B4`, blocks will be processed recursively:
/// ```
/// * B1 is received and included
/// * B2 is processed
/// * B1 becomes final in the graph
/// * B3 is processed
/// * B2 becomes final in the graph
/// * B4 is processed
/// * B3 becomes final in the graph
/// * PoS is told about all finalized blocks
/// ```
///
/// The problem we had is that in order to check rolls to verify `B4`'s roll sell,
/// the final roll registry was assumed to be attached to the last final block known by the graph,
/// but that was inaccurate because PoS was the one holding the final roll registry,
/// and PoS was not yet aware of the blocks that finalized during recursion,
/// so it was actually still attached to G when `B4` was checked.
///
/// The correction involved taking the point of view of PoS on where the final roll registry is attached.
/// This test ensures non-regression by making sure `B4` is propagated when `B1` is received.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial]
async fn test_inter_cycle_batch_finalization() {
    let t0: MassaTime = 1000.into();
    let staking_key =
        KeyPair::from_str("S1UxdCJv5ckDK8z87E5Jq5fEfSVLi2cTHgtpfZy7iURs3KpPns8").unwrap();
    let warmup_time: MassaTime = 1000.into();
    let margin_time: MassaTime = 300.into();
    let cfg = ConsensusConfig {
        periods_per_cycle: 2,
        delta_f0: 1,
        thread_count: 1,
        endorsement_count: 1,
        max_future_processing_blocks: 10,
        max_dependency_blocks: 10,
        future_block_processing_max_periods: 10,
        t0,
        genesis_timestamp: MassaTime::now(0).unwrap().saturating_add(warmup_time),
        ..ConsensusConfig::default()
    };

    consensus_pool_test_with_storage(
        cfg.clone(),
        None,
        async move |pool_controller,
                    mut protocol_controller,
                    consensus_command_sender,
                    consensus_event_receiver,
                    mut storage,
                    selector_controller| {
            // wait for consensus warmup time
            tokio::time::sleep(warmup_time.to_duration()).await;

            let genesis_blocks: Vec<BlockId> = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .best_parents
                .iter()
                .map(|(b, _p)| *b)
                .collect();

            // create B1 but DO NOT SEND IT
            tokio::time::sleep(t0.to_duration()).await;
            let b1_block =
                create_block(&cfg, Slot::new(1, 0), genesis_blocks.clone(), &staking_key);

            // create and send B2
            tokio::time::sleep(t0.to_duration()).await;
            let b2_block = create_block_with_operations_and_endorsements(
                &cfg,
                Slot::new(2, 0),
                &vec![b1_block.id],
                &staking_key,
                vec![],
                vec![create_endorsement(
                    &staking_key,
                    Slot::new(1, 0),
                    b1_block.id,
                    0,
                )],
            );
            let b2_block_id = b2_block.id;
            let b2_block_slot = b2_block.content.header.content.slot;
            storage.store_block(b2_block);
            protocol_controller
                .receive_block(b2_block_id, b2_block_slot, storage.clone())
                .await;

            // create and send B3
            tokio::time::sleep(t0.to_duration()).await;
            let b3_block = create_block_with_operations_and_endorsements(
                &cfg,
                Slot::new(3, 0),
                &vec![b2_block_id],
                &staking_key,
                vec![],
                vec![create_endorsement(
                    &staking_key,
                    Slot::new(2, 0),
                    b2_block_id,
                    0,
                )],
            );
            let b3_block_id = b3_block.id;
            let b3_block_slot = b3_block.content.header.content.slot;
            storage.store_block(b3_block);
            protocol_controller
                .receive_block(b3_block_id, b3_block_slot, storage.clone())
                .await;

            // create and send B4
            tokio::time::sleep(t0.to_duration()).await;
            let roll_sell = create_roll_sell(&staking_key, 1, 4, 0);
            storage.store_operations(vec![roll_sell.clone()]);
            let b4_block = create_block_with_operations_and_endorsements(
                &cfg,
                Slot::new(4, 0),
                &vec![b3_block_id],
                &staking_key,
                vec![roll_sell],
                vec![create_endorsement(
                    &staking_key,
                    Slot::new(3, 0),
                    b3_block_id,
                    0,
                )],
            );
            let b4_block_id = b4_block.id;
            let b4_block_slot = b4_block.content.header.content.slot;
            storage.store_block(b4_block);
            protocol_controller
                .receive_block(b4_block_id, b4_block_slot, storage.clone())
                .await;

            // wait for the slot after B4
            tokio::time::sleep(t0.saturating_mul(5).to_duration()).await;

            // send B1
            let b1_block_id = b1_block.id;
            let b1_block_slot = b1_block.content.header.content.slot;
            storage.store_block(b1_block);
            protocol_controller
                .receive_block(b1_block_id, b1_block_slot, storage.clone())
                .await;

            approve_producer_and_selector_for_staker(&staking_key, &selector_controller);

            // wait for the propagation of B1, B2, B3 and B4 (unordered)
            let mut to_propagate: HashSet<_> =
                vec![b1_block_id, b2_block_id, b3_block_id, b4_block_id]
                    .into_iter()
                    .collect();
            for _ in 0u8..4 {
                to_propagate.remove(
                    &validate_propagate_block_in_list(
                        &mut protocol_controller,
                        &to_propagate.clone().into_iter().collect(),
                        margin_time.to_millis(),
                    )
                    .await,
                );
            }

            (
                pool_controller,
                protocol_controller,
                consensus_command_sender,
                consensus_event_receiver,
                selector_controller,
            )
        },
    )
    .await;
}
