//! Copyright (c) 2022 MASSA LABS <info@massa.net>

use massa_models::slot::Slot;
use massa_models::stats::ExecutionStats;
use massa_time::MassaTime;
use std::collections::VecDeque;

/// Execution statistics counter
pub struct ExecutionStatsCounter {
    /// duration of the time window
    time_window_duration: MassaTime,
    /// time compensation (milliseconds)
    compensation_millis: i64,
    /// final blocks in the time window (count, instant)
    final_blocks: VecDeque<(usize, MassaTime)>,
    /// final operations executed in the time window (count, instant)
    final_executed_ops: VecDeque<(usize, MassaTime)>,
}

impl ExecutionStatsCounter {
    /// create a new `ExecutionStatsCounter`
    pub fn new(time_window_duration: MassaTime, compensation_millis: i64) -> Self {
        ExecutionStatsCounter {
            time_window_duration,
            compensation_millis,
            final_blocks: Default::default(),
            final_executed_ops: Default::default(),
        }
    }

    /// refresh the counters and delete old records
    fn refresh(&mut self, current_time: MassaTime) {
        let start_time = current_time.saturating_sub(self.time_window_duration);

        // prune final blocks
        while let Some((_, t)) = self.final_blocks.front() {
            if t < &start_time {
                self.final_blocks.pop_front();
            } else {
                break;
            }
        }

        // prune final executed ops
        while let Some((_, t)) = self.final_executed_ops.front() {
            if t < &start_time {
                self.final_executed_ops.pop_front();
            } else {
                break;
            }
        }
    }

    /// register final blocks
    pub fn register_final_blocks(&mut self, count: usize) {
        let current_time =
            MassaTime::now(self.compensation_millis).expect("could not get current time");
        self.final_blocks.push_back((count, current_time));
        self.refresh(current_time);
    }

    /// register final executed operations
    pub fn register_final_executed_operations(&mut self, count: usize) {
        let current_time =
            MassaTime::now(self.compensation_millis).expect("could not get current time");
        self.final_executed_ops.push_back((count, current_time));
        self.refresh(current_time);
    }

    /// get statistics
    pub fn get_stats(&self, active_cursor: Slot) -> ExecutionStats {
        let current_time =
            MassaTime::now(self.compensation_millis).expect("could not get current time");
        let start_time = current_time.saturating_sub(self.time_window_duration);
        let map_func = |pair: &(usize, MassaTime)| -> usize {
            let (cnt, t) = pair;
            if t >= &start_time && t <= &current_time {
                *cnt
            } else {
                0
            }
        };
        ExecutionStats {
            final_block_count: self.final_blocks.iter().map(map_func).sum(),
            final_executed_operations_count: self.final_executed_ops.iter().map(map_func).sum(),
            time_window_start: start_time,
            time_window_end: current_time,
            active_cursor,
        }
    }
}
