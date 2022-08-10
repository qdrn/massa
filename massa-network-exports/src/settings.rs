// Copyright (c) 2022 MASSA LABS <info@massa.net>

use enum_map::EnumMap;
use massa_time::MassaTime;
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};

use crate::peers::PeerType;

/// Network configuration
#[derive(Debug, Deserialize, Clone)]
pub struct NetworkSettings {
    /// Where to listen for communications.
    pub bind: SocketAddr,
    /// Our own IP if it is routable, else None.
    pub routable_ip: Option<IpAddr>,
    /// Protocol port
    pub protocol_port: u16,
    /// Time interval spent waiting for a response from a peer.
    /// In milliseconds
    pub connect_timeout: MassaTime,
    /// `Network_worker` will try to connect to available peers every `wakeup_interval`.
    /// In milliseconds
    pub wakeup_interval: MassaTime,
    /// Path to the file containing initial peers.
    pub initial_peers_file: std::path::PathBuf,
    /// Path to the file containing known peers.
    pub peers_file: std::path::PathBuf,
    /// Path to the file containing our keypair
    pub keypair_file: std::path::PathBuf,
    /// Configuration for `PeerType` connections
    pub peer_types_config: EnumMap<PeerType, PeerTypeConnectionConfig>,
    /// Limit on the number of in connections per ip.
    pub max_in_connections_per_ip: usize,
    /// Limit on the number of idle peers we remember.
    pub max_idle_peers: usize,
    /// Limit on the number of banned peers we remember.
    pub max_banned_peers: usize,
    /// Peer database is dumped every `peers_file_dump_interval` in milliseconds
    pub peers_file_dump_interval: MassaTime,
    /// After `message_timeout` milliseconds we are no longer waiting on handshake message
    pub message_timeout: MassaTime,
    /// Every `ask_peer_list_interval` in milliseconds we ask every one for its advertisable peers list.
    pub ask_peer_list_interval: MassaTime,
    /// Max wait time for sending a Network or Node event.
    pub max_send_wait: MassaTime,
    /// Time after which we forget a node
    pub ban_timeout: MassaTime,
    /// Timeout Duration when we send a `PeerList` in handshake
    pub peer_list_send_timeout: MassaTime,
    /// Max number of in connection overflowed managed by the handshake that send a list of peers
    pub max_in_connection_overflow: usize,
    /// Max operations per message in the network to avoid sending to big data packet.
    pub max_operations_per_message: u32,
    /// Read limitation for a connection in bytes per seconds
    pub max_bytes_read: f64,
    /// Write limitation for a connection in bytes per seconds
    pub max_bytes_write: f64,
}

/// Connection configuration for a peer type
/// Limit the current connections for a given peer type as a whole
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PeerTypeConnectionConfig {
    /// max number of incoming connection
    pub max_in_connections: usize,
    /// target number of outgoing connections
    pub target_out_connections: usize,
    /// max number of on going outgoing connection attempt
    pub max_out_attempts: usize,
}

/// setting tests
#[cfg(feature = "testing")]
pub mod tests {
    use crate::NetworkSettings;
    use crate::{test_exports::tools::get_temp_keypair_file, PeerType};
    use enum_map::enum_map;
    use massa_models::constants::{BASE_NETWORK_CONTROLLER_IP, MAX_OPERATIONS_PER_MESSAGE};
    use massa_time::MassaTime;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::PeerTypeConnectionConfig;

    impl Default for NetworkSettings {
        fn default() -> Self {
            let peer_types_config = enum_map! {
                PeerType::Bootstrap => PeerTypeConnectionConfig {
                    target_out_connections: 1,
                    max_out_attempts: 1,
                    max_in_connections: 1,
                },
                PeerType::WhiteListed => PeerTypeConnectionConfig {
                    target_out_connections: 2,
                    max_out_attempts: 2,
                    max_in_connections: 3,
                },
                PeerType::Standard => PeerTypeConnectionConfig {
                    target_out_connections: 10,
                    max_out_attempts: 15,
                    max_in_connections: 5,
                }
            };
            NetworkSettings {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                routable_ip: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                protocol_port: 0,
                connect_timeout: MassaTime::from(180_000),
                wakeup_interval: MassaTime::from(10_000),
                peers_file: std::path::PathBuf::new(),
                max_in_connections_per_ip: 2,
                max_idle_peers: 3,
                max_banned_peers: 3,
                peers_file_dump_interval: MassaTime::from(10_000),
                message_timeout: MassaTime::from(5000u64),
                ask_peer_list_interval: MassaTime::from(50000u64),
                keypair_file: std::path::PathBuf::new(),
                max_send_wait: MassaTime::from(100),
                ban_timeout: MassaTime::from(100_000_000),
                initial_peers_file: std::path::PathBuf::new(),
                peer_list_send_timeout: MassaTime::from(500),
                max_in_connection_overflow: 2,
                peer_types_config,
                max_operations_per_message: MAX_OPERATIONS_PER_MESSAGE,
                max_bytes_read: std::f64::INFINITY,
                max_bytes_write: std::f64::INFINITY,
            }
        }
    }

    impl NetworkSettings {
        /// default network settings from port and peer file path
        pub fn scenarios_default(port: u16, peers_file: &std::path::Path) -> Self {
            // Init the serialization context with a default,
            // can be overwritten with a more specific one in the test.
            massa_models::init_serialization_context(massa_models::SerializationContext {
                max_advertise_length: 128,
                max_bootstrap_blocks: 100,
                max_bootstrap_cliques: 100,
                max_bootstrap_deps: 100,
                max_bootstrap_children: 100,
                max_ask_blocks_per_message: 10,
                endorsement_count: 8,
                ..massa_models::SerializationContext::default()
            });
            let peer_types_config = enum_map! {
                PeerType::Bootstrap => PeerTypeConnectionConfig {
                    target_out_connections: 1,
                    max_out_attempts: 1,
                    max_in_connections: 1,
                },
                PeerType::WhiteListed => PeerTypeConnectionConfig {
                    target_out_connections: 2,
                    max_out_attempts: 2,
                    max_in_connections: 3,
                },
                PeerType::Standard => PeerTypeConnectionConfig {
                    target_out_connections: 10,
                    max_out_attempts: 15,
                    max_in_connections: 5,
                }
            };
            Self {
                bind: format!("0.0.0.0:{}", port).parse().unwrap(),
                routable_ip: Some(BASE_NETWORK_CONTROLLER_IP),
                protocol_port: port,
                connect_timeout: MassaTime::from(3000),
                peers_file: peers_file.to_path_buf(),
                wakeup_interval: MassaTime::from(3000),
                max_in_connections_per_ip: 100,
                max_idle_peers: 100,
                max_banned_peers: 100,
                peers_file_dump_interval: MassaTime::from(30000),
                message_timeout: MassaTime::from(5000u64),
                ask_peer_list_interval: MassaTime::from(50000u64),
                keypair_file: get_temp_keypair_file().path().to_path_buf(),
                max_send_wait: MassaTime::from(100),
                ban_timeout: MassaTime::from(100_000_000),
                initial_peers_file: peers_file.to_path_buf(),
                peer_list_send_timeout: MassaTime::from(50),
                max_in_connection_overflow: 10,
                peer_types_config,
                max_operations_per_message: MAX_OPERATIONS_PER_MESSAGE,
                max_bytes_read: std::f64::INFINITY,
                max_bytes_write: std::f64::INFINITY,
            }
        }
    }
}
