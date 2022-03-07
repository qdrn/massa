// Copyright (c) 2022 MASSA LABS <info@massa.net>

//! This file defines the final ledger associating addresses to their balances, bytecode and data.

use crate::ledger_changes::LedgerChanges;
use crate::ledger_entry::LedgerEntry;
use crate::types::{Applicable, SetUpdateOrDelete};
use crate::{FinalLedgerBootstrapState, LedgerConfig, LedgerError};
use massa_hash::hash::Hash;
use massa_models::{Address, Amount, Slot};
use std::collections::{BTreeMap, VecDeque};

/// Represents a final ledger associating addresses to their balances, bytecode and data.
/// The final ledger is also attached to a final slot, can be boostrapped and allows others to bootstrap.
/// The ledger size can be very high: it can exceed 1TB.
/// To allow for storage on disk, the ledger uses trees and has `O(log(N))` access, insertion and deletion complexity.
///
/// Note: currently the ledger is stored in RAM. TODO put it on the hard drive with cache.
pub struct FinalLedger {
    /// ledger config
    config: LedgerConfig,
    /// slot at the output of which the final ledger is attached
    pub slot: Slot,
    /// ledger tree, sorted by address
    sorted_ledger: BTreeMap<Address, LedgerEntry>,
    /// history of recent final ledger changes, useful for streaming bootstrap
    /// front = oldest, back = newest
    changes_history: VecDeque<(Slot, LedgerChanges)>,
}

/// Allows applying LedgerChanges to the final ledger
///
/// Warning: this does not push the changes in changes_history.
/// Always use FinalLedger::settle_slot to apply bootstrapable changes.
impl Applicable<LedgerChanges> for FinalLedger {
    fn apply(&mut self, changes: LedgerChanges) {
        // for all incoming changes
        for (addr, change) in changes.0 {
            match change {
                // the incoming change sets a ledger entry to a new one
                SetUpdateOrDelete::Set(new_entry) => {
                    // inserts/overwrites the entry with the incoming one
                    self.sorted_ledger.insert(addr, new_entry);
                }

                // the incoming change updates an existing ledger entry
                SetUpdateOrDelete::Update(entry_update) => {
                    // applies the updates to the entry
                    // if the entry does not exist, inserts a default one and applies the updates to it
                    self.sorted_ledger
                        .entry(addr)
                        .or_insert_with(Default::default)
                        .apply(entry_update);
                }

                // the incoming change deletes a ledger entry
                SetUpdateOrDelete::Delete => {
                    // delete the entry, if it exists
                    self.sorted_ledger.remove(&addr);
                }
            }
        }
    }
}

/// Macro used to shorten file error returns
macro_rules! init_file_error {
    ($st:expr, $cfg:ident) => {
        |err| {
            LedgerError::FileError(format!(
                "error $st initial ledger file {}: {}",
                $cfg.initial_sce_ledger_path
                    .to_str()
                    .unwrap_or("(non-utf8 path)"),
                err
            ))
        }
    };
}
pub(crate) use init_file_error;

impl FinalLedger {
    /// Initializes a new FinalLedger by reading its initial state from file.
    pub fn new(config: LedgerConfig) -> Result<Self, LedgerError> {
        // load the ledger tree from file
        let sorted_ledger = serde_json::from_str::<BTreeMap<Address, Amount>>(
            &std::fs::read_to_string(&config.initial_sce_ledger_path)
                .map_err(init_file_error!("loading", config))?,
        )
        .map_err(init_file_error!("parsing", config))?
        .into_iter()
        .map(|(address, balance)| {
            (
                address,
                LedgerEntry {
                    parallel_balance: balance,
                    ..Default::default()
                },
            )
        })
        .collect();

        // the initial ledger is attached to the output of the last genesis block
        let slot = Slot::new(0, config.thread_count.saturating_sub(1));

        // generate the final ledger
        Ok(FinalLedger {
            slot,
            sorted_ledger,
            changes_history: Default::default(),
            config,
        })
    }

    /// Intiialize a FinalLedger from a bootstrap state
    ///
    /// TODO: This loads the whole ledger in RAM. Switch to streaming in the future
    ///
    /// # Arguments
    /// * config: ledger config
    /// * state: bootstrap state
    pub fn from_bootstrap_state(config: LedgerConfig, state: FinalLedgerBootstrapState) -> Self {
        FinalLedger {
            slot: state.slot,
            sorted_ledger: state.sorted_ledger,
            changes_history: Default::default(),
            config,
        }
    }

    /// Gets a snapshot of the ledger to bootstrap other nodes
    ///
    /// TODO: This loads the whole ledger in RAM. Switch to streaming in the future
    pub fn get_bootstrap_state(&self) -> FinalLedgerBootstrapState {
        FinalLedgerBootstrapState {
            slot: self.slot,
            sorted_ledger: self.sorted_ledger.clone(),
        }
    }

    /// Gets a copy of a full ledger entry.
    ///
    /// # Returns
    /// A clone of the whole LedgerEntry, or None if not found.
    ///
    /// TODO: in the future, never manipulate full ledger entries because their datastore can be huge
    /// https://github.com/massalabs/massa/issues/2342
    pub fn get_full_entry(&self, addr: &Address) -> Option<LedgerEntry> {
        self.sorted_ledger.get(addr).cloned()
    }

    /// Applies changes to the ledger, pushes them to the bootstrap history,
    /// and sets the ledger's attachment final slot.
    /// After this is called, the final ledger is attached to the output of `slot`
    /// and ready to bootstrap nodes with this new state.
    pub fn settle_slot(&mut self, slot: Slot, changes: LedgerChanges) {
        // apply changes
        self.apply(changes.clone());

        // update the attachment final slot
        self.slot = slot;

        // update and prune changes history
        self.changes_history.push_back((slot, changes));
        while self.changes_history.len() > self.config.final_history_length {
            self.changes_history.pop_front();
        }
    }

    /// Gets the parallel balance of a ledger entry
    ///
    /// # Returns
    /// The parallel balance, or None if the ledger entry was not found
    pub fn get_parallel_balance(&self, addr: &Address) -> Option<Amount> {
        self.sorted_ledger.get(addr).map(|v| v.parallel_balance)
    }

    /// Gets a copy of the bytecode of a ledger entry
    ///
    /// # Returns
    /// A copy of the found bytecode, or None if the ledger entry was not found
    pub fn get_bytecode(&self, addr: &Address) -> Option<Vec<u8>> {
        self.sorted_ledger.get(addr).map(|v| v.bytecode.clone())
    }

    /// Checks if a ledger entry exists
    ///
    /// # Returns
    /// true if it exists, false otherwise.
    pub fn entry_exists(&self, addr: &Address) -> bool {
        self.sorted_ledger.contains_key(addr)
    }

    /// Gets a copy of the value of a datastore entry for a given address.
    ///
    /// # Arguments
    /// * addr: target address
    /// * key: datastore key
    ///
    /// # Returns
    /// A copy of the datastore value, or None if the ledger entry or datastore entry was not found
    pub fn get_data_entry(&self, addr: &Address, key: &Hash) -> Option<Vec<u8>> {
        self.sorted_ledger
            .get(addr)
            .and_then(|v| v.datastore.get(key).cloned())
    }

    /// Checks for the existence of a datastore entry for a given address.
    ///
    /// # Arguments
    /// * addr: target address
    /// * key: datastore key
    ///
    /// # Returns
    /// true if the datastore entry was found, or false if the ledger entry or datastore entry was not found
    pub fn has_data_entry(&self, addr: &Address, key: &Hash) -> bool {
        self.sorted_ledger
            .get(addr)
            .map_or(false, |v| v.datastore.contains_key(key))
    }
}
