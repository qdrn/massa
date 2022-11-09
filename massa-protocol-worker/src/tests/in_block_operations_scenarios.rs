// Copyright (c) 2022 MASSA LABS <info@massa.net>

use super::tools::protocol_test;
use massa_hash::Hash;
use massa_models::operation::OperationId;
use massa_models::wrapped::{Id, WrappedContent};
use massa_models::{
    address::Address,
    block::{Block, BlockHeader, BlockHeaderSerializer, BlockSerializer},
    slot::Slot,
};
use massa_network_exports::NetworkCommand;
use massa_protocol_exports::tests::tools;
use massa_protocol_exports::tests::tools::{
    create_and_connect_nodes, create_block_with_operations, create_operation_with_expire_period,
    send_and_propagate_block,
};
use massa_signature::KeyPair;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_protocol_does_propagate_operations_received_in_blocks() {
    let protocol_config = &tools::PROTOCOL_CONFIG;
    protocol_test(
        protocol_config,
        async move |mut network_controller,
                    mut protocol_event_receiver,
                    mut protocol_command_sender,
                    protocol_manager,
                    protocol_pool_event_receiver| {
            // Create 2 node.
            let mut nodes = create_and_connect_nodes(2, &mut network_controller).await;

            let creator_node = nodes.pop().expect("Failed to get node info.");

            let mut keypair = KeyPair::generate();
            let mut address = Address::from_public_key(&keypair.get_public_key());
            let mut thread = address.get_thread(2);

            while thread != 0 {
                keypair = KeyPair::generate();
                address = Address::from_public_key(&keypair.get_public_key());
                thread = address.get_thread(2);
            }

            // block with ok operation
            let op = create_operation_with_expire_period(&keypair, 5);
            let op_thread = op.creator_address.get_thread(protocol_config.thread_count);

            let block = create_block_with_operations(
                &creator_node.keypair,
                Slot::new(1, op_thread),
                vec![op.clone()],
            );
            send_and_propagate_block(
                &mut network_controller,
                block,
                true,
                creator_node.id,
                &mut protocol_event_receiver,
                &mut protocol_command_sender,
                vec![op.clone()],
            )
            .await;

            // Propagates the operation found in the block.
            if let Some(NetworkCommand::SendOperationAnnouncements { to_node, batch }) =
                network_controller
                    .wait_command(1000.into(), |cmd| match cmd {
                        cmd @ NetworkCommand::SendOperationAnnouncements { .. } => Some(cmd),
                        _ => None,
                    })
                    .await
            {
                if !batch.contains(&op.id.prefix())
                    && to_node == nodes.pop().expect("Failed to get node info.").id
                {
                    panic!("Operation in block not propagated.");
                }
            };
            (
                network_controller,
                protocol_event_receiver,
                protocol_command_sender,
                protocol_manager,
                protocol_pool_event_receiver,
            )
        },
    )
    .await;
}

#[tokio::test]
#[serial]
async fn test_protocol_sends_blocks_with_operations_to_consensus() {
    //         // setup logging
    // stderrlog::new()
    // .verbosity(4)
    // .timestamp(stderrlog::Timestamp::Millisecond)
    // .init()
    // .unwrap();
    let protocol_config = &tools::PROTOCOL_CONFIG;
    protocol_test(
        protocol_config,
        async move |mut network_controller,
                    mut protocol_event_receiver,
                    mut protocol_command_sender,
                    protocol_manager,
                    protocol_pool_event_receiver| {
            // Create 1 node.
            let mut nodes = create_and_connect_nodes(1, &mut network_controller).await;

            let creator_node = nodes.pop().expect("Failed to get node info.");

            let mut keypair = KeyPair::generate();
            let mut address = Address::from_public_key(&keypair.get_public_key());
            let mut thread = address.get_thread(2);

            while thread != 0 {
                keypair = KeyPair::generate();
                address = Address::from_public_key(&keypair.get_public_key());
                thread = address.get_thread(2);
            }

            // block with ok operation
            {
                let op = create_operation_with_expire_period(&keypair, 5);
                let op_thread = op.creator_address.get_thread(protocol_config.thread_count);

                let block = create_block_with_operations(
                    &creator_node.keypair,
                    Slot::new(1, op_thread),
                    vec![op.clone()],
                );
                send_and_propagate_block(
                    &mut network_controller,
                    block,
                    true,
                    creator_node.id,
                    &mut protocol_event_receiver,
                    &mut protocol_command_sender,
                    vec![op.clone()],
                )
                .await;
            }

            // block with wrong merkle root
            {
                let op = create_operation_with_expire_period(&keypair, 5);
                let op_thread = op.creator_address.get_thread(protocol_config.thread_count);
                let block = {
                    let operation_merkle_root = Hash::compute_from("merkle root".as_bytes());

                    let header = BlockHeader::new_wrapped(
                        BlockHeader {
                            slot: Slot::new(1, op_thread),
                            parents: Vec::new(),
                            operation_merkle_root,
                            endorsements: Vec::new(),
                        },
                        BlockHeaderSerializer::new(),
                        &creator_node.keypair,
                    )
                    .unwrap();

                    Block::new_wrapped(
                        Block {
                            header,
                            operations: vec![op.clone()].into_iter().map(|op| op.id).collect(),
                        },
                        BlockSerializer::new(),
                        &creator_node.keypair,
                    )
                    .unwrap()
                };

                send_and_propagate_block(
                    &mut network_controller,
                    block,
                    false,
                    creator_node.id,
                    &mut protocol_event_receiver,
                    &mut protocol_command_sender,
                    vec![op.clone()],
                )
                .await;
            }

            // block with operation with wrong signature
            {
                let mut op = create_operation_with_expire_period(&keypair, 5);
                let op_thread = op.creator_address.get_thread(protocol_config.thread_count);
                op.id = OperationId::new(Hash::compute_from("wrong signature".as_bytes()));
                let block = create_block_with_operations(
                    &creator_node.keypair,
                    Slot::new(1, op_thread),
                    vec![op.clone()],
                );

                send_and_propagate_block(
                    &mut network_controller,
                    block,
                    false,
                    creator_node.id,
                    &mut protocol_event_receiver,
                    &mut protocol_command_sender,
                    vec![op.clone()],
                )
                .await;
            }

            (
                network_controller,
                protocol_event_receiver,
                protocol_command_sender,
                protocol_manager,
                protocol_pool_event_receiver,
            )
        },
    )
    .await;
}
