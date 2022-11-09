//! Copyright (c) 2022 MASSA LABS <info@massa.net>

//! This file defines testing tools related to the configuration

use std::path::PathBuf;

use crate::{FinalState, FinalStateConfig};
use massa_async_pool::{AsyncPool, AsyncPoolConfig};
use massa_executed_ops::{ExecutedOps, ExecutedOpsConfig};
use massa_ledger_exports::LedgerConfig;
use massa_ledger_worker::FinalLedger;
use massa_models::{
    config::{
        DEFERRED_CREDITS_BOOTSTRAP_PART_SIZE, EXECUTED_OPS_BOOTSTRAP_PART_SIZE, PERIODS_PER_CYCLE,
        POS_SAVED_CYCLES, THREAD_COUNT,
    },
    slot::Slot,
};
use massa_pos_exports::{PoSConfig, PoSFinalState};

impl FinalState {
    /// Create a final stat
    pub fn create_final_state(pos_state: PoSFinalState, config: FinalStateConfig) -> Self {
        FinalState {
            slot: Slot::new(0, 0),
            ledger: Box::new(FinalLedger::new(config.ledger_config.clone())),
            async_pool: AsyncPool::new(config.async_pool_config.clone()),
            pos_state,
            executed_ops: ExecutedOps::new(config.executed_ops_config.clone()),
            changes_history: Default::default(),
            config,
        }
    }
}

/// Default value of `FinalStateConfig` used for tests
impl Default for FinalStateConfig {
    fn default() -> FinalStateConfig {
        FinalStateConfig {
            ledger_config: LedgerConfig::default(),
            async_pool_config: AsyncPoolConfig::default(),
            executed_ops_config: ExecutedOpsConfig {
                thread_count: THREAD_COUNT,
                bootstrap_part_size: EXECUTED_OPS_BOOTSTRAP_PART_SIZE,
            },
            pos_config: PoSConfig {
                periods_per_cycle: PERIODS_PER_CYCLE,
                thread_count: THREAD_COUNT,
                cycle_history_length: POS_SAVED_CYCLES,
                credits_bootstrap_part_size: DEFERRED_CREDITS_BOOTSTRAP_PART_SIZE,
            },
            final_history_length: 10,
            thread_count: 2,
            periods_per_cycle: 100,
            initial_rolls_path: PathBuf::new(),
            initial_seed_string: "".to_string(),
        }
    }
}
