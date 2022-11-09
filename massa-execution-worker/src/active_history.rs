use massa_execution_exports::ExecutionOutput;
use massa_ledger_exports::{
    LedgerEntry, LedgerEntryUpdate, SetOrDelete, SetOrKeep, SetUpdateOrDelete,
};
use massa_models::{
    address::Address, amount::Amount, operation::OperationId, prehash::PreHashMap, slot::Slot,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Default)]
/// History of the outputs of recently executed slots.
/// Slots should be consecutive, oldest at the beginning and latest at the back.
pub(crate) struct ActiveHistory(pub VecDeque<ExecutionOutput>);

/// Result of a lazy, active history search
pub enum HistorySearchResult<T> {
    Present(T),
    Absent,
    NoInfo,
}

/// Result of the search for a slot index in history
#[derive(Debug)]
pub enum SlotIndexPosition {
    /// out of bounds in the past
    Past,
    /// out of bounds in the future
    Future,
    /// found in history at a the given index
    Found(usize),
    /// history is empty
    NoHistory,
}

impl ActiveHistory {
    /// Remove `slot` and the slots after it from history
    pub fn truncate_from(&mut self, slot: &Slot, thread_count: u8) {
        match self.get_slot_index(slot, thread_count) {
            SlotIndexPosition::Past => self.0.clear(),
            SlotIndexPosition::Found(index) => self.0.truncate(index),
            _ => {}
        }
    }

    /// Lazily query (from end to beginning) the active list of executed ops to check if an op was executed.
    ///
    /// Returns a `HistorySearchResult`.
    pub fn fetch_executed_op(&self, op_id: &OperationId) -> HistorySearchResult<()> {
        for history_element in self.0.iter().rev() {
            if history_element
                .state_changes
                .executed_ops_changes
                .contains_key(op_id)
            {
                return HistorySearchResult::Present(());
            }
        }
        HistorySearchResult::NoInfo
    }

    /// Lazily query (from end to beginning) the active balance of an address after a given index.
    ///
    /// Returns a `HistorySearchResult`.
    pub fn fetch_balance(&self, addr: &Address) -> HistorySearchResult<Amount> {
        for output in self.0.iter().rev() {
            match output.state_changes.ledger_changes.0.get(addr) {
                Some(SetUpdateOrDelete::Set(v)) => return HistorySearchResult::Present(v.balance),
                Some(SetUpdateOrDelete::Update(LedgerEntryUpdate {
                    balance: SetOrKeep::Set(v),
                    ..
                })) => return HistorySearchResult::Present(*v),
                Some(SetUpdateOrDelete::Delete) => return HistorySearchResult::Absent,
                _ => (),
            }
        }
        HistorySearchResult::NoInfo
    }

    /// Lazily query (from end to beginning) the active bytecode of an address after a given index.
    ///
    /// Returns a `HistorySearchResult`.
    pub fn fetch_bytecode(&self, addr: &Address) -> HistorySearchResult<Vec<u8>> {
        for output in self.0.iter().rev() {
            match output.state_changes.ledger_changes.0.get(addr) {
                Some(SetUpdateOrDelete::Set(v)) => {
                    return HistorySearchResult::Present(v.bytecode.to_vec())
                }
                Some(SetUpdateOrDelete::Update(LedgerEntryUpdate {
                    bytecode: SetOrKeep::Set(v),
                    ..
                })) => return HistorySearchResult::Present(v.to_vec()),
                Some(SetUpdateOrDelete::Delete) => return HistorySearchResult::Absent,
                _ => (),
            }
        }
        HistorySearchResult::NoInfo
    }

    /// Lazily query (from end to beginning) the active datastore entry of an address after a given index.
    ///
    /// Returns a `HistorySearchResult`.
    pub fn fetch_active_history_data_entry(
        &self,
        addr: &Address,
        key: &[u8],
    ) -> HistorySearchResult<Vec<u8>> {
        for output in self.0.iter().rev() {
            match output.state_changes.ledger_changes.0.get(addr) {
                Some(SetUpdateOrDelete::Set(LedgerEntry { datastore, .. })) => {
                    match datastore.get(key) {
                        Some(value) => return HistorySearchResult::Present(value.to_vec()),
                        None => return HistorySearchResult::Absent,
                    }
                }
                Some(SetUpdateOrDelete::Update(LedgerEntryUpdate { datastore, .. })) => {
                    match datastore.get(key) {
                        Some(SetOrDelete::Set(value)) => {
                            return HistorySearchResult::Present(value.to_vec())
                        }
                        Some(SetOrDelete::Delete) => return HistorySearchResult::Absent,
                        None => (),
                    }
                }
                Some(SetUpdateOrDelete::Delete) => return HistorySearchResult::Absent,
                None => (),
            }
        }
        HistorySearchResult::NoInfo
    }

    /// Starting from the newest element in history, return the first existing roll change of `addr`.
    ///
    /// # Arguments
    /// * `addr`: address to fetch the rolls from
    pub fn fetch_roll_count(&self, addr: &Address) -> Option<u64> {
        self.0.iter().rev().find_map(|output| {
            output
                .state_changes
                .pos_changes
                .roll_changes
                .get(addr)
                .cloned()
        })
    }

    /// Traverse the whole history and return every deferred credit of `addr` _after_ `slot` (included).
    ///
    /// # Arguments
    /// * `slot`: slot _after_ which we fetch the credits
    /// * `addr`: address to fetch the credits from
    #[allow(dead_code)]
    pub fn fetch_deferred_credits_after(
        &self,
        slot: &Slot,
        addr: &Address,
    ) -> BTreeMap<Slot, Amount> {
        self.0
            .iter()
            .flat_map(|output| {
                output
                    .state_changes
                    .pos_changes
                    .deferred_credits
                    .0
                    .range(slot..)
                    .filter_map(|(&slot, credits)| credits.get(addr).map(|&amount| (slot, amount)))
            })
            .collect()
    }

    /// Traverse the whole history and return every deferred credit _at_ `slot`
    ///
    /// # Arguments
    /// * `slot`: slot _at_ which we fetch the credits
    pub fn fetch_all_deferred_credits_at(&self, slot: &Slot) -> PreHashMap<Address, Amount> {
        self.0
            .iter()
            .filter_map(|output| {
                output
                    .state_changes
                    .pos_changes
                    .deferred_credits
                    .0
                    .get(slot)
                    .cloned()
            })
            .flatten()
            .collect()
    }

    /// Gets the index of a slot in history
    pub fn get_slot_index(&self, slot: &Slot, thread_count: u8) -> SlotIndexPosition {
        let first_slot = match self.0.front() {
            Some(itm) => &itm.slot,
            None => return SlotIndexPosition::NoHistory,
        };
        if slot < first_slot {
            return SlotIndexPosition::Past; // too old
        }
        let index: usize = match slot.slots_since(first_slot, thread_count) {
            Err(_) => return SlotIndexPosition::Future, // overflow
            Ok(d) => {
                match d.try_into() {
                    Ok(d) => d,
                    Err(_) => return SlotIndexPosition::Future, // usize overflow
                }
            }
        };
        if index >= self.0.len() {
            // in the future
            return SlotIndexPosition::Future;
        }
        SlotIndexPosition::Found(index)
    }

    /// Find the history range of a cycle
    ///
    /// # Return value
    ///
    /// Tuple with the following elements:
    /// * a range of indices
    /// * a boolean indicating that the cycle overflows before the beginning of history
    /// * a boolean indicating that the cycle overflows after the end of history
    pub fn find_cycle_indices(
        &self,
        cycle: u64,
        periods_per_cycle: u64,
        thread_count: u8,
    ) -> (std::ops::Range<usize>, bool, bool) {
        // Get first and last slots indices of cycle in history. We consider overflows as "Future"
        let first_index = Slot::new_first_of_cycle(cycle, periods_per_cycle).map_or_else(
            |_e| SlotIndexPosition::Future,
            |s| self.get_slot_index(&s, thread_count),
        );
        let last_index = Slot::new_last_of_cycle(cycle, periods_per_cycle, thread_count)
            .map_or_else(
                |_e| SlotIndexPosition::Future,
                |s| self.get_slot_index(&s, thread_count),
            );

        match (first_index, last_index) {
            // history is empty
            (SlotIndexPosition::NoHistory, _) => (0..0, true, true),
            (_, SlotIndexPosition::NoHistory) => (0..0, true, true),

            // cycle starts in the future
            (SlotIndexPosition::Future, _) => (0..0, false, true),

            // cycle ends before history starts
            (_, SlotIndexPosition::Past) => (0..0, true, false),

            // the history is strictly included within the cycle
            (SlotIndexPosition::Past, SlotIndexPosition::Future) => (0..self.0.len(), true, true),

            // cycle begins before and ends during history
            (SlotIndexPosition::Past, SlotIndexPosition::Found(idx)) => {
                (0..idx.saturating_add(1), true, false)
            }

            // cycle starts during the history and ends after the end of history
            (SlotIndexPosition::Found(idx), SlotIndexPosition::Future) => {
                (idx..self.0.len(), false, true)
            }

            // cycle starts and ends during active history
            (SlotIndexPosition::Found(idx1), SlotIndexPosition::Found(idx2)) => {
                (idx1..idx2.saturating_add(1), false, false)
            }
        }
    }
}
