// Copyright (c) 2022 MASSA LABS <info@massa.net>

//! This file defines testing tools related to the configuration

use crate::{ExecutionConfig, StorageCostsConstants};
use massa_models::config::*;
use massa_time::MassaTime;

impl Default for ExecutionConfig {
    /// default configuration used for testing
    fn default() -> Self {
        let storage_costs_constants = StorageCostsConstants {
            ledger_cost_per_byte: LEDGER_COST_PER_BYTE,
            ledger_entry_base_cost: LEDGER_COST_PER_BYTE
                .checked_mul_u64(LEDGER_ENTRY_BASE_SIZE as u64)
                .expect("Overflow when creating constant ledger_entry_base_cost"),
            ledger_entry_datastore_base_cost: LEDGER_COST_PER_BYTE
                .checked_mul_u64(LEDGER_ENTRY_DATASTORE_BASE_SIZE as u64)
                .expect("Overflow when creating constant ledger_entry_datastore_base_size"),
        };

        Self {
            readonly_queue_length: 100,
            max_final_events: 1000,
            max_async_gas: MAX_ASYNC_GAS,
            thread_count: THREAD_COUNT,
            roll_price: ROLL_PRICE,
            cursor_delay: MassaTime::from_millis(0),
            block_reward: BLOCK_REWARD,
            endorsement_count: ENDORSEMENT_COUNT as u64,
            max_gas_per_block: MAX_GAS_PER_BLOCK,
            operation_validity_period: OPERATION_VALIDITY_PERIODS,
            periods_per_cycle: PERIODS_PER_CYCLE,
            clock_compensation: Default::default(),
            // reset genesis timestamp because we are in test mode that can take a while to process
            genesis_timestamp: MassaTime::now(0)
                .expect("Impossible to reset the timestamp in test"),
            t0: 64.into(),
            stats_time_window_duration: MassaTime::from_millis(30000),
            max_miss_ratio: *POS_MISS_RATE_DEACTIVATION_THRESHOLD,
            max_datastore_key_length: MAX_DATASTORE_KEY_LENGTH,
            max_bytecode_size: MAX_BYTECODE_LENGTH,
            max_datastore_value_size: MAX_DATASTORE_VALUE_LENGTH,
            storage_costs_constants,
        }
    }
}
