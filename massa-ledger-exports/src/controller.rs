use massa_hash::Hash;
use massa_models::{
    address::Address, amount::Amount, error::ModelsError, slot::Slot, streaming_step::StreamingStep,
};
use std::collections::BTreeSet;
use std::fmt::Debug;

use crate::{LedgerChanges, LedgerError};

pub trait LedgerController: Send + Sync + Debug {
    /// Allows applying `LedgerChanges` to the final ledger
    fn apply_changes(&mut self, changes: LedgerChanges, slot: Slot);

    /// Loads ledger from file
    fn load_initial_ledger(&mut self) -> Result<(), LedgerError>;

    /// Gets the balance of a ledger entry
    ///
    /// # Returns
    /// The balance, or None if the ledger entry was not found
    fn get_balance(&self, addr: &Address) -> Option<Amount>;

    /// Gets a copy of the bytecode of a ledger entry
    ///
    /// # Returns
    /// A copy of the found bytecode, or None if the ledger entry was not found
    fn get_bytecode(&self, addr: &Address) -> Option<Vec<u8>>;

    /// Checks if a ledger entry exists
    ///
    /// # Returns
    /// true if it exists, false otherwise.
    fn entry_exists(&self, addr: &Address) -> bool;

    /// Gets a copy of the value of a datastore entry for a given address.
    ///
    /// # Arguments
    /// * `addr`: target address
    /// * `key`: datastore key
    ///
    /// # Returns
    /// A copy of the datastore value, or `None` if the ledger entry or datastore entry was not found
    fn get_data_entry(&self, addr: &Address, key: &[u8]) -> Option<Vec<u8>>;

    /// Checks for the existence of a datastore entry for a given address.
    ///
    /// # Arguments
    /// * `addr`: target address
    /// * `key`: datastore key
    ///
    /// # Returns
    /// true if the datastore entry was found, or false if the ledger entry or datastore entry was not found
    fn has_data_entry(&self, addr: &Address, key: &[u8]) -> bool;

    /// Get every key of the datastore for a given address.
    ///
    /// # Returns
    /// A `BTreeSet` of the datastore keys
    fn get_datastore_keys(&self, addr: &Address) -> BTreeSet<Vec<u8>>;

    /// Get the current disk ledger hash
    fn get_ledger_hash(&self) -> Hash;

    /// Get a part of the ledger
    /// Used for bootstrap
    /// Return: Tuple with data and last key
    fn get_ledger_part(
        &self,
        last_key: StreamingStep<Vec<u8>>,
    ) -> Result<(Vec<u8>, StreamingStep<Vec<u8>>), ModelsError>;

    /// Set a part of the ledger
    /// Used for bootstrap
    /// Return: Last key inserted
    fn set_ledger_part(&self, data: Vec<u8>) -> Result<StreamingStep<Vec<u8>>, ModelsError>;

    /// Get every address and their corresponding balance.
    ///
    /// IMPORTANT: This should only be used for debug and test purposes.
    ///
    /// # Returns
    /// A `BTreeMap` with the address as key and the balance as value
    #[cfg(feature = "testing")]
    fn get_every_address(&self) -> std::collections::BTreeMap<Address, Amount>;

    /// Get the entire datastore for a given address.
    ///
    /// IMPORTANT: This should only be used for debug purposes.
    ///
    /// # Returns
    /// A `BTreeMap` with the entry hash as key and the data bytes as value
    #[cfg(feature = "testing")]
    fn get_entire_datastore(&self, addr: &Address) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>>;
}
