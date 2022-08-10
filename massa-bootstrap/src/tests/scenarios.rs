// Copyright (c) 2022 MASSA LABS <info@massa.net>

use super::{
    mock_establisher,
    tools::{
        bridge_mock_streams, get_boot_state, get_peers, get_random_final_state_bootstrap,
        wait_consensus_command, wait_network_command,
    },
};
use crate::BootstrapSettings;
use crate::{
    get_state, start_bootstrap_server,
    tests::tools::{
        assert_eq_bootstrap_graph, assert_eq_thread_cycle_states, get_bootstrap_config,
    },
};
use massa_consensus_exports::{commands::ConsensusCommand, ConsensusCommandSender};
use massa_final_state::{test_exports::assert_eq_final_state, FinalState};
use massa_models::Version;
use massa_network_exports::{NetworkCommand, NetworkCommandSender};
use massa_signature::KeyPair;
use massa_time::MassaTime;
use parking_lot::RwLock;
use serial_test::serial;
use std::{str::FromStr, sync::Arc};
use tokio::sync::mpsc;

lazy_static::lazy_static! {
    pub static ref BOOTSTRAP_SETTINGS_KEYPAIR: (BootstrapSettings, KeyPair) = {
        let keypair = KeyPair::generate();
        (get_bootstrap_config(keypair.get_public_key()), keypair)
    };
}

#[tokio::test]
#[serial]
async fn test_bootstrap_server() {
    let (bootstrap_settings, keypair): &(BootstrapSettings, KeyPair) = &BOOTSTRAP_SETTINGS_KEYPAIR;

    let (consensus_cmd_tx, mut consensus_cmd_rx) = mpsc::channel::<ConsensusCommand>(5);
    let (network_cmd_tx, mut network_cmd_rx) = mpsc::channel::<NetworkCommand>(5);
    let final_state_bootstrap = get_random_final_state_bootstrap(2);
    let final_state = Arc::new(RwLock::new(final_state_bootstrap));

    let (bootstrap_establisher, bootstrap_interface) = mock_establisher::new();
    let bootstrap_manager = start_bootstrap_server(
        ConsensusCommandSender(consensus_cmd_tx),
        NetworkCommandSender(network_cmd_tx),
        final_state.clone(),
        bootstrap_settings,
        bootstrap_establisher,
        keypair.clone(),
        0,
        Version::from_str("TEST.1.2").unwrap(),
    )
    .await
    .unwrap()
    .unwrap();

    let final_state_client = Arc::new(RwLock::new(FinalState::default()));
    let final_state_client_thread = final_state_client.clone();

    // launch the get_state process
    let (remote_establisher, mut remote_interface) = mock_establisher::new();
    let get_state_h = tokio::spawn(async move {
        get_state(
            bootstrap_settings,
            final_state_client_thread,
            remote_establisher,
            Version::from_str("TEST.1.2").unwrap(),
            MassaTime::now().unwrap().saturating_sub(1000.into()),
            None,
        )
        .await
        .unwrap()
    });

    // accept connection attempt from remote
    let (remote_rw, conn_addr, resp) = tokio::time::timeout(
        std::time::Duration::from_millis(1000),
        remote_interface.wait_connection_attempt_from_controller(),
    )
    .await
    .expect("timeout waiting for connection attempt from remote")
    .expect("error receiving connection attempt from remote");
    let expect_conn_addr = bootstrap_settings.bootstrap_list[0].0;
    assert_eq!(
        conn_addr, expect_conn_addr,
        "client connected to wrong bootstrap ip"
    );
    resp.send(true)
        .expect("could not send connection accept to remote");

    // connect to bootstrap
    let remote_addr = std::net::SocketAddr::from_str("82.245.72.98:10000").unwrap(); // not checked
    let bootstrap_rw = tokio::time::timeout(
        std::time::Duration::from_millis(1000),
        bootstrap_interface.connect_to_controller(&remote_addr),
    )
    .await
    .expect("timeout while connecting to bootstrap")
    .expect("could not connect to bootstrap");

    // launch bridge
    let bridge = tokio::spawn(async move {
        bridge_mock_streams(remote_rw, bootstrap_rw).await;
    });

    // intercept peers being asked
    let wait_peers = async move || {
        // wait for bootstrap to ask network for peers, send them
        let response = match wait_network_command(&mut network_cmd_rx, 1000.into(), |cmd| match cmd
        {
            NetworkCommand::GetBootstrapPeers(resp) => Some(resp),
            _ => None,
        })
        .await
        {
            Some(resp) => resp,
            None => panic!("timeout waiting for get peers command"),
        };
        let sent_peers = get_peers();
        response.send(sent_peers.clone()).unwrap();
        sent_peers
    };

    // wait for peers
    let sent_peers = wait_peers().await;

    // here the ledger is queried directly. We don't intercept this

    // wait for bootstrap to ask consensus for bootstrap graph, send it
    let response = match wait_consensus_command(&mut consensus_cmd_rx, 1000.into(), |cmd| match cmd
    {
        ConsensusCommand::GetBootstrapState(resp) => Some(resp),
        _ => None,
    })
    .await
    {
        Some(resp) => resp,
        None => panic!("timeout waiting for get boot graph consensus command"),
    };
    let (sent_pos, sent_graph) = get_boot_state();
    response
        .send((sent_pos.clone(), sent_graph.clone()))
        .unwrap();

    // wait for get_state
    let bootstrap_res = get_state_h
        .await
        .expect("error while waiting for get_state to finish");

    // wait for bridge
    bridge.await.expect("bridge join failed");

    // check peers
    assert_eq!(
        sent_peers.0,
        bootstrap_res.peers.unwrap().0,
        "mismatch between sent and received peers"
    );

    // check final states
    assert_eq_final_state(&final_state.read(), &final_state_client.read());

    // check states
    assert_eq_thread_cycle_states(&sent_pos, &bootstrap_res.pos.unwrap());
    assert_eq_bootstrap_graph(&sent_graph, &bootstrap_res.graph.unwrap());

    // stop bootstrap server
    bootstrap_manager
        .stop()
        .await
        .expect("could not stop bootstrap server");
}
