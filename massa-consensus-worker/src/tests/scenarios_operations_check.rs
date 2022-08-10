// Copyright (c) 2022 MASSA LABS <info@massa.net>
use super::tools::*;
use massa_consensus_exports::ConsensusConfig;

use massa_models::ledger_models::LedgerData;
use massa_models::prehash::Set;
use massa_models::{Address, Amount, Slot};
use massa_signature::KeyPair;
use massa_time::MassaTime;
use serial_test::serial;
use std::collections::HashMap;
use std::str::FromStr;

#[tokio::test]
#[serial]
async fn test_operations_check() {
    // setup logging
    /*
    stderrlog::new()
        .verbosity(4)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
    */

    let thread_count = 2;

    let (address_1, keypair_1) = random_address_on_thread(0, thread_count).into();
    let (address_2, keypair_2) = random_address_on_thread(1, thread_count).into();

    assert_eq!(1, address_2.get_thread(thread_count));
    let mut ledger = HashMap::new();
    ledger.insert(address_1, LedgerData::new(Amount::from_str("5").unwrap()));

    let cfg = ConsensusConfig {
        t0: 1000.into(),
        future_block_processing_max_periods: 50,
        operation_validity_periods: 10,
        genesis_timestamp: MassaTime::now().unwrap().saturating_sub(10000.into()),
        ..ConsensusConfig::default_with_staking_keys_and_ledger(&[keypair_1.clone()], &ledger)
    };

    consensus_without_pool_test(
        cfg.clone(),
        async move |mut protocol_controller, consensus_command_sender, consensus_event_receiver| {
            let genesis_ids = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            // Valid block A sending 5 from addr1 to addr2 + reward 1 to addr1
            let operation_1 = create_transaction(&keypair_1, address_2, 5, 5, 1);
            let block_a = create_block_with_operations(
                &cfg,
                Slot::new(1, 0),
                &genesis_ids,
                &keypair_1,
                vec![operation_1.clone()],
            );
            propagate_block(&mut protocol_controller, block_a.clone(), true, 150).await;

            // assert address 1 has 1 coin at blocks (A, genesis_ids[1]) (see #269)
            let mut set = Set::<Address>::default();
            set.insert(address_1);
            let res = consensus_command_sender
                .get_addresses_info(set)
                .await
                .unwrap()
                .get(&address_1)
                .unwrap()
                .ledger_info
                .candidate_ledger_info;
            assert_eq!(res.balance, Amount::from_str("1").unwrap());

            // receive block b with invalid operation (not enough coins)
            let operation_2 = create_transaction(&keypair_2, address_1, 10, 8, 1);
            let block_2b = create_block_with_operations(
                &cfg,
                Slot::new(1, 1),
                &vec![block_a.id, genesis_ids[1]],
                &keypair_2,
                vec![operation_2],
            );
            propagate_block(&mut protocol_controller, block_2b, false, 1000).await;

            // receive empty block b
            let block_b = create_block_with_operations(
                &cfg,
                Slot::new(1, 1),
                &vec![block_a.id, genesis_ids[1]],
                &keypair_1,
                vec![],
            );
            propagate_block(&mut protocol_controller, block_b.clone(), true, 150).await;

            // assert address 2 has 5 coins at block B
            let mut set = Set::<Address>::default();
            set.insert(address_2);
            let res = consensus_command_sender
                .get_addresses_info(set)
                .await
                .unwrap()
                .get(&address_2)
                .unwrap()
                .ledger_info
                .candidate_ledger_info;
            assert_eq!(res.balance, Amount::from_str("5").unwrap());

            // receive block with reused operation
            let block_1c = create_block_with_operations(
                &cfg,
                Slot::new(1, 0),
                &vec![block_a.id, block_b.id],
                &keypair_1,
                vec![operation_1.clone()],
            );
            propagate_block(&mut protocol_controller, block_1c.clone(), false, 1000).await;

            (
                protocol_controller,
                consensus_command_sender,
                consensus_event_receiver,
            )
        },
    )
    .await;
}

#[tokio::test]
#[serial]
async fn test_execution_check() {
    let (address_1, keypair_1) = random_address().into();

    let mut ledger = HashMap::new();
    ledger.insert(address_1, LedgerData::new(Amount::from_str("5").unwrap()));

    let staking_keys: Vec<KeyPair> = vec![keypair_1.clone()];
    let cfg = ConsensusConfig {
        t0: 1000.into(),
        future_block_processing_max_periods: 50,
        operation_validity_periods: 10,
        genesis_key: keypair_1.clone(),
        genesis_timestamp: MassaTime::now().unwrap().saturating_sub(10000.into()),
        ..ConsensusConfig::default_with_staking_keys_and_ledger(&staking_keys, &ledger)
    };

    consensus_without_pool_test(
        cfg.clone(),
        async move |mut protocol_controller, consensus_command_sender, consensus_event_receiver| {
            let genesis_ids = consensus_command_sender
                .get_block_graph_status(None, None)
                .await
                .expect("could not get block graph status")
                .genesis_blocks;

            // Valid block A executing some bytecode and spending 2 coins.
            let operation_1 = create_executesc(&keypair_1, 5, 5, Default::default(), 1, 2, 1);
            let block_a = create_block_with_operations(
                &cfg,
                Slot::new(1, 0),
                &genesis_ids,
                &keypair_1,
                vec![operation_1.clone()],
            );
            propagate_block(&mut protocol_controller, block_a, true, 150).await;

            // assert the `coins` argument as been deducted from the balance of address 1.
            let mut set = Set::<Address>::default();
            set.insert(address_1);
            let res = consensus_command_sender
                .get_addresses_info(set)
                .await
                .unwrap()
                .get(&address_1)
                .unwrap()
                .ledger_info
                .candidate_ledger_info;
            assert_eq!(res.balance, Amount::from_str("3").unwrap());

            (
                protocol_controller,
                consensus_command_sender,
                consensus_event_receiver,
            )
        },
    )
    .await;
}
