//! Copyright (c) 2022 MASSA LABS <info@massa.net>

//! Pool of operation and endorsements waiting to be included in a block
//!
#![warn(missing_docs)]
#![warn(unused_crate_dependencies)]

mod config;
mod controller_traits;

pub use config::PoolConfig;
pub use controller_traits::{PoolController, PoolManager};

/// Test utils
#[cfg(feature = "testing")]
/// Exports related to tests as Mocks and configurations
pub mod test_exports;
