//! Copyright (c) 2022 MASSA LABS <info@massa.net>

//! This file defines the factory settings

use massa_time::MassaTime;

/// Structure defining the settings of the factory
#[derive(Debug, Clone)]
pub struct FactoryConfig {
    /// number of threads
    pub thread_count: u8,

    /// genesis timestamp
    pub genesis_timestamp: MassaTime,

    /// period duration
    pub t0: MassaTime,

    /// clock compensation in relative milliseconds
    pub clock_compensation_millis: i64,

    /// initial delay before starting production, to avoid double-production on node restart
    pub initial_delay: MassaTime,

    /// maximal block size in bytes
    pub max_block_size: u64,

    /// maximal block gas
    pub max_block_gas: u64,
}
