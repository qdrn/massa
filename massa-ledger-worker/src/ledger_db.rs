//! Copyright (c) 2022 MASSA LABS <info@massa.net>

//! Module to interact with the disk ledger

use massa_hash::{Hash, HASH_SIZE_BYTES};
use massa_ledger_exports::*;
use massa_models::{
    address::{Address, ADDRESS_SIZE_BYTES},
    amount::AmountSerializer,
    error::ModelsError,
    serialization::{VecU8Deserializer, VecU8Serializer},
    slot::{Slot, SlotSerializer},
    streaming_step::StreamingStep,
};
use massa_serialization::{Deserializer, Serializer, U64VarIntSerializer};
use nom::multi::many0;
use nom::sequence::tuple;
use rocksdb::{
    ColumnFamily, ColumnFamilyDescriptor, Direction, IteratorMode, Options, ReadOptions,
    WriteBatch, DB,
};
use std::ops::Bound;
use std::path::PathBuf;
use std::rc::Rc;
use std::{collections::BTreeMap, fmt::Debug};
use std::{
    collections::{BTreeSet, HashMap},
    convert::TryInto,
};

#[cfg(feature = "testing")]
use massa_models::amount::{Amount, AmountDeserializer};

const LEDGER_CF: &str = "ledger";
const METADATA_CF: &str = "metadata";
const OPEN_ERROR: &str = "critical: rocksdb open operation failed";
const CRUD_ERROR: &str = "critical: rocksdb crud operation failed";
const CF_ERROR: &str = "critical: rocksdb column family operation failed";
const LEDGER_HASH_ERROR: &str = "critical: saved ledger hash is corrupted";
const KEY_LEN_SER_ERROR: &str = "critical: key length serialization failed";
const SLOT_KEY: &[u8; 1] = b"s";
const LEDGER_HASH_KEY: &[u8; 1] = b"h";
const LEDGER_HASH_INITIAL_BYTES: &[u8; 32] = &[0; HASH_SIZE_BYTES];

/// Ledger sub entry enum
pub enum LedgerSubEntry {
    /// Balance
    Balance,
    /// Bytecode
    Bytecode,
    /// Datastore entry
    Datastore(Vec<u8>),
}

/// Disk ledger DB module
///
/// Contains a `RocksDB` DB instance
pub(crate) struct LedgerDB {
    db: DB,
    thread_count: u8,
    amount_serializer: AmountSerializer,
    slot_serializer: SlotSerializer,
    len_serializer: U64VarIntSerializer,
    max_datastore_key_length: u8,
    ledger_part_size_message_bytes: u64,
    #[cfg(feature = "testing")]
    amount_deserializer: AmountDeserializer,
}

impl Debug for LedgerDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self.db)
    }
}

/// For a given start prefix (inclusive), returns the correct end prefix (non-inclusive).
/// This assumes the key bytes are ordered in lexicographical order.
/// Since key length is not limited, for some case we return `None` because there is
/// no bounded limit (every keys in the series `[]`, `[255]`, `[255, 255]` ...).
fn end_prefix(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end_range = prefix.to_vec();
    while let Some(0xff) = end_range.last() {
        end_range.pop();
    }
    if let Some(byte) = end_range.last_mut() {
        *byte += 1;
        Some(end_range)
    } else {
        None
    }
}

#[test]
fn test_end_prefix() {
    assert_eq!(end_prefix(&[5, 6, 7]), Some(vec![5, 6, 8]));
    assert_eq!(end_prefix(&[5, 6, 255]), Some(vec![5, 7]));
}

/// Batch containing write operations to perform on disk and cache for the ledger hash computing
pub struct LedgerBatch {
    // Rocksdb write batch
    write_batch: WriteBatch,
    // Ledger hash state in the current batch
    ledger_hash: Hash,
    // Added entry hashes in the current batch
    aeh_list: BTreeMap<Vec<u8>, Hash>,
}

impl LedgerBatch {
    pub fn new(ledger_hash: Hash) -> Self {
        Self {
            write_batch: WriteBatch::default(),
            ledger_hash,
            aeh_list: BTreeMap::new(),
        }
    }
}

impl LedgerDB {
    /// Create and initialize a new `LedgerDB`.
    ///
    /// # Arguments
    /// * path: path to the desired disk ledger db directory
    pub fn new(
        path: PathBuf,
        thread_count: u8,
        max_datastore_key_length: u8,
        ledger_part_size_message_bytes: u64,
    ) -> Self {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let db = DB::open_cf_descriptors(
            &db_opts,
            path,
            vec![
                ColumnFamilyDescriptor::new(LEDGER_CF, Options::default()),
                ColumnFamilyDescriptor::new(METADATA_CF, Options::default()),
            ],
        )
        .expect(OPEN_ERROR);

        LedgerDB {
            db,
            thread_count,
            amount_serializer: AmountSerializer::new(),
            slot_serializer: SlotSerializer::new(),
            len_serializer: U64VarIntSerializer::new(),
            max_datastore_key_length,
            ledger_part_size_message_bytes,
            #[cfg(feature = "testing")]
            amount_deserializer: AmountDeserializer::new(
                Bound::Included(Amount::MIN),
                Bound::Included(Amount::MAX),
            ),
        }
    }

    /// Loads the initial disk ledger
    ///
    /// # Arguments
    pub fn load_initial_ledger(&mut self, initial_ledger: HashMap<Address, LedgerEntry>) {
        // initial ledger_hash value to avoid matching an option in every XOR operation
        // because of a one time case being an empty ledger
        let ledger_hash = Hash::from_bytes(LEDGER_HASH_INITIAL_BYTES);
        let mut batch = LedgerBatch::new(ledger_hash);
        for (address, entry) in initial_ledger {
            self.put_entry(&address, entry, &mut batch);
        }
        self.set_slot(
            Slot::new(0, self.thread_count.saturating_sub(1)),
            &mut batch,
        );
        self.write_batch(batch);
    }

    /// Allows applying `LedgerChanges` to the disk ledger
    ///
    /// # Arguments
    /// * changes: ledger changes to be applied
    /// * slot: new slot associated to the final ledger
    pub fn apply_changes(&mut self, changes: LedgerChanges, slot: Slot) {
        // create the batch
        let mut batch = LedgerBatch::new(self.get_ledger_hash());
        // for all incoming changes
        for (addr, change) in changes.0 {
            match change {
                // the incoming change sets a ledger entry to a new one
                SetUpdateOrDelete::Set(new_entry) => {
                    // inserts/overwrites the entry with the incoming one
                    self.put_entry(&addr, new_entry, &mut batch);
                }
                // the incoming change updates an existing ledger entry
                SetUpdateOrDelete::Update(entry_update) => {
                    // applies the updates to the entry
                    // if the entry does not exist, inserts a default one and applies the updates to it
                    self.update_entry(&addr, entry_update, &mut batch);
                }
                // the incoming change deletes a ledger entry
                SetUpdateOrDelete::Delete => {
                    // delete the entry, if it exists
                    self.delete_entry(&addr, &mut batch);
                }
            }
        }
        // set the associated slot in metadata
        self.set_slot(slot, &mut batch);
        // write the batch
        self.write_batch(batch);
    }

    /// Apply the given operation batch to the disk ledger
    fn write_batch(&self, mut batch: LedgerBatch) {
        let handle = self.db.cf_handle(METADATA_CF).expect(CF_ERROR);
        batch
            .write_batch
            .put_cf(handle, LEDGER_HASH_KEY, batch.ledger_hash.to_bytes());
        self.db.write(batch.write_batch).expect(CRUD_ERROR);
    }

    /// Set the disk ledger slot metadata
    ///
    /// # Arguments
    /// * slot: associated slot of the current ledger
    /// * batch: the given operation batch to update
    fn set_slot(&self, slot: Slot, batch: &mut LedgerBatch) {
        let handle = self.db.cf_handle(METADATA_CF).expect(CF_ERROR);
        let mut slot_bytes = Vec::new();
        // Slot serialization never fails
        self.slot_serializer
            .serialize(&slot, &mut slot_bytes)
            .unwrap();
        batch
            .write_batch
            .put_cf(handle, SLOT_KEY, slot_bytes.clone());
        // XOR previous slot and new one
        if let Some(prev_bytes) = self.db.get_cf(handle, SLOT_KEY).expect(CRUD_ERROR) {
            batch.ledger_hash ^= Hash::compute_from(&prev_bytes);
        }
        batch.ledger_hash ^= Hash::compute_from(&slot_bytes);
    }

    /// Get the current disk ledger hash
    pub fn get_ledger_hash(&self) -> Hash {
        let handle = self.db.cf_handle(METADATA_CF).expect(CF_ERROR);
        if let Some(ledger_hash_bytes) = self.db.get_cf(handle, LEDGER_HASH_KEY).expect(CRUD_ERROR)
        {
            Hash::from_bytes(&ledger_hash_bytes.try_into().expect(LEDGER_HASH_ERROR))
        } else {
            // initial ledger_hash value to avoid matching an option in every XOR operation
            // because of a one time case being an empty ledger
            // also note that the if you XOR a hash with itself result is LEDGER_HASH_INITIAL_BYTES
            Hash::from_bytes(LEDGER_HASH_INITIAL_BYTES)
        }
    }

    /// Internal function to put a key & value and perform the ledger hash XORs
    fn put_entry_value(
        &self,
        handle: &ColumnFamily,
        batch: &mut LedgerBatch,
        key: &[u8],
        value: &[u8],
    ) {
        let mut len_bytes = Vec::new();
        self.len_serializer
            .serialize(&(key.len() as u64), &mut len_bytes)
            .expect(KEY_LEN_SER_ERROR);
        let hash = Hash::compute_from(&[&len_bytes, key, value].concat());
        batch.ledger_hash ^= hash;
        batch.aeh_list.insert(key.to_vec(), hash);
        batch.write_batch.put_cf(handle, key, value);
    }

    /// Add every sub-entry individually for a given entry.
    ///
    /// # Arguments
    /// * `addr`: associated address
    /// * `ledger_entry`: complete entry to be added
    /// * `batch`: the given operation batch to update
    fn put_entry(&mut self, addr: &Address, ledger_entry: LedgerEntry, batch: &mut LedgerBatch) {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);
        // Amount serialization never fails
        let mut bytes_balance = Vec::new();
        self.amount_serializer
            .serialize(&ledger_entry.balance, &mut bytes_balance)
            .unwrap();

        // balance
        self.put_entry_value(handle, batch, &balance_key!(addr), &bytes_balance);

        // bytecode
        self.put_entry_value(handle, batch, &bytecode_key!(addr), &ledger_entry.bytecode);

        // datastore
        for (hash, entry) in ledger_entry.datastore {
            self.put_entry_value(handle, batch, &data_key!(addr, hash), &entry);
        }
    }

    /// Get the given sub-entry of a given address.
    ///
    /// # Arguments
    /// * `addr`: associated address
    /// * `ty`: type of the queried sub-entry
    ///
    /// # Returns
    /// An Option of the sub-entry value as bytes
    pub fn get_sub_entry(&self, addr: &Address, ty: LedgerSubEntry) -> Option<Vec<u8>> {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);

        match ty {
            LedgerSubEntry::Balance => self
                .db
                .get_cf(handle, balance_key!(addr))
                .expect(CRUD_ERROR),
            LedgerSubEntry::Bytecode => self
                .db
                .get_cf(handle, bytecode_key!(addr))
                .expect(CRUD_ERROR),
            LedgerSubEntry::Datastore(hash) => self
                .db
                .get_cf(handle, data_key!(addr, hash))
                .expect(CRUD_ERROR),
        }
    }

    /// Get every key of the datastore for a given address.
    ///
    /// # Returns
    /// A `BTreeSet` of the datastore keys
    pub fn get_datastore_keys(&self, addr: &Address) -> BTreeSet<Vec<u8>> {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);

        let mut opt = ReadOptions::default();
        opt.set_iterate_upper_bound(end_prefix(data_prefix!(addr)).unwrap());

        self.db
            .iterator_cf_opt(
                handle,
                opt,
                IteratorMode::From(data_prefix!(addr), Direction::Forward),
            )
            .flatten()
            .map(|(key, _)| key.split_at(ADDRESS_SIZE_BYTES + 1).1.to_vec())
            .collect()
    }

    /// Internal function to update a key & value and perform the ledger hash XORs
    fn update_key_value(
        &self,
        handle: &ColumnFamily,
        batch: &mut LedgerBatch,
        key: &[u8],
        value: &[u8],
    ) {
        let mut len_bytes = Vec::new();
        self.len_serializer
            .serialize(&(key.len() as u64), &mut len_bytes)
            .expect(KEY_LEN_SER_ERROR);
        if let Some(added_hash) = batch.aeh_list.get(key) {
            batch.ledger_hash ^= *added_hash;
        } else if let Some(prev_bytes) = self.db.get_cf(handle, key).expect(CRUD_ERROR) {
            batch.ledger_hash ^= Hash::compute_from(&[&len_bytes, key, &prev_bytes].concat());
        }
        let hash = Hash::compute_from(&[&len_bytes, key, value].concat());
        batch.ledger_hash ^= hash;
        batch.aeh_list.insert(key.to_vec(), hash);
        batch.write_batch.put_cf(handle, key, value);
    }

    /// Update the ledger entry of a given address.
    ///
    /// # Arguments
    /// * `entry_update`: a descriptor of the entry updates to be applied
    /// * `batch`: the given operation batch to update
    fn update_entry(
        &mut self,
        addr: &Address,
        entry_update: LedgerEntryUpdate,
        batch: &mut LedgerBatch,
    ) {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);

        // balance
        if let SetOrKeep::Set(balance) = entry_update.balance {
            let mut bytes = Vec::new();
            // Amount serialization never fails
            self.amount_serializer
                .serialize(&balance, &mut bytes)
                .unwrap();
            self.update_key_value(handle, batch, &balance_key!(addr), &bytes);
        }

        // bytecode
        if let SetOrKeep::Set(bytecode) = entry_update.bytecode {
            self.update_key_value(handle, batch, &bytecode_key!(addr), &bytecode);
        }

        // datastore
        for (hash, update) in entry_update.datastore {
            match update {
                SetOrDelete::Set(entry) => {
                    self.update_key_value(handle, batch, &data_key!(addr, hash), &entry)
                }
                SetOrDelete::Delete => self.delete_key(handle, batch, &data_key!(addr, hash)),
            }
        }
    }

    /// Internal function to delete a key and perform the ledger hash XOR
    fn delete_key(&self, handle: &ColumnFamily, batch: &mut LedgerBatch, key: &[u8]) {
        if let Some(added_hash) = batch.aeh_list.get(key) {
            batch.ledger_hash ^= *added_hash;
        } else if let Some(prev_bytes) = self.db.get_cf(handle, key).expect(CRUD_ERROR) {
            let mut len_bytes = Vec::new();
            self.len_serializer
                .serialize(&(key.len() as u64), &mut len_bytes)
                .expect(KEY_LEN_SER_ERROR);
            batch.ledger_hash ^= Hash::compute_from(&[&len_bytes, key, &prev_bytes].concat());
        }
        batch.write_batch.delete_cf(handle, key);
    }

    /// Delete every sub-entry associated to the given address.
    ///
    /// # Arguments
    /// * batch: the given operation batch to update
    fn delete_entry(&self, addr: &Address, batch: &mut LedgerBatch) {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);

        // balance
        self.delete_key(handle, batch, &balance_key!(addr));

        // bytecode
        self.delete_key(handle, batch, &bytecode_key!(addr));

        // datastore
        let mut opt = ReadOptions::default();
        opt.set_iterate_upper_bound(end_prefix(data_prefix!(addr)).unwrap());
        for (key, _) in self
            .db
            .iterator_cf_opt(
                handle,
                opt,
                IteratorMode::From(data_prefix!(addr), Direction::Forward),
            )
            .flatten()
        {
            self.delete_key(handle, batch, &key);
        }
    }

    /// Get a part of the disk Ledger.
    /// Mainly used in the bootstrap process.
    ///
    /// # Arguments
    /// * `last_key`: key where the part retrieving must start
    ///
    /// # Returns
    /// A tuple containing:
    /// * The ledger part as bytes
    /// * The last taken key (this is an optimization to easily keep a reference to the last key)
    pub fn get_ledger_part(
        &self,
        cursor: StreamingStep<Vec<u8>>,
    ) -> Result<(Vec<u8>, StreamingStep<Vec<u8>>), ModelsError> {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);
        let opt = ReadOptions::default();
        let ser = VecU8Serializer::new();
        let key_serializer = KeySerializer::new();
        let mut ledger_part = Vec::new();

        // Creates an iterator from the next element after the last if defined, otherwise initialize it at the first key of the ledger.
        let (db_iterator, mut new_cursor) = match cursor {
            StreamingStep::Started => (
                self.db.iterator_cf_opt(handle, opt, IteratorMode::Start),
                StreamingStep::Started,
            ),
            StreamingStep::Ongoing(last_key) => {
                let mut iter = self.db.iterator_cf_opt(
                    handle,
                    opt,
                    IteratorMode::From(&last_key, Direction::Forward),
                );
                iter.next();
                (iter, StreamingStep::Finished)
            }
            StreamingStep::Finished => return Ok((ledger_part, cursor)),
        };

        // Iterates over the whole database
        for (key, entry) in db_iterator.flatten() {
            if (ledger_part.len() as u64) < (self.ledger_part_size_message_bytes) {
                key_serializer.serialize(&key.to_vec(), &mut ledger_part)?;
                ser.serialize(&entry.to_vec(), &mut ledger_part)?;
                new_cursor = StreamingStep::Ongoing(key.to_vec());
            } else {
                break;
            }
        }
        Ok((ledger_part, new_cursor))
    }

    /// Set a part of the ledger in the database.
    /// We deserialize in this function because we insert in the ledger while deserializing.
    /// Used for bootstrap.
    ///
    /// # Arguments
    /// * data: must be the serialized version provided by `get_ledger_part`
    ///
    /// # Returns
    /// The last key of the inserted entry (this is an optimization to easily keep a reference to the last key)
    pub fn set_ledger_part<'a>(
        &self,
        data: &'a [u8],
    ) -> Result<StreamingStep<Vec<u8>>, ModelsError> {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);
        let vec_u8_deserializer =
            VecU8Deserializer::new(Bound::Included(0), Bound::Excluded(u64::MAX));
        let key_deserializer = KeyDeserializer::new(self.max_datastore_key_length);
        let mut last_key = Rc::new(Vec::new());
        let mut batch = LedgerBatch::new(self.get_ledger_hash());

        // Since this data is coming from the network, deser to address and ser back to bytes for a security check.
        let (rest, _) = many0(|input: &'a [u8]| {
            let (rest, (key, value)) = tuple((
                |input| key_deserializer.deserialize(input),
                |input| vec_u8_deserializer.deserialize(input),
            ))(input)?;
            *Rc::get_mut(&mut last_key).ok_or_else(|| {
                nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail))
            })? = key.clone();
            self.put_entry_value(handle, &mut batch, &key, &value);
            Ok((rest, ()))
        })(data)
        .map_err(|_| ModelsError::SerializeError("Error in deserialization".to_string()))?;

        // Every byte should have been read
        if rest.is_empty() {
            self.write_batch(batch);
            Ok(StreamingStep::Ongoing((*last_key).clone()))
        } else {
            Err(ModelsError::SerializeError(
                "rest is not empty.".to_string(),
            ))
        }
    }

    /// Get every address and their corresponding balance.
    ///
    /// IMPORTANT: This should only be used for debug purposes.
    ///
    /// # Returns
    /// A `BTreeMap` with the address as key and the balance as value
    #[cfg(feature = "testing")]
    pub fn get_every_address(
        &self,
    ) -> std::collections::BTreeMap<Address, massa_models::amount::Amount> {
        use massa_models::address::AddressDeserializer;
        use massa_serialization::DeserializeError;

        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);

        let ledger = self
            .db
            .iterator_cf(handle, IteratorMode::Start)
            .collect::<Vec<_>>();

        let mut addresses = std::collections::BTreeMap::new();
        let address_deserializer = AddressDeserializer::new();
        for (key, entry) in ledger.iter().flatten() {
            let (rest, address) = address_deserializer
                .deserialize::<DeserializeError>(&key[..])
                .unwrap();
            if rest.first() == Some(&BALANCE_IDENT) {
                let (_, amount) = self
                    .amount_deserializer
                    .deserialize::<DeserializeError>(entry)
                    .unwrap();
                addresses.insert(address, amount);
            }
        }
        addresses
    }

    /// Get the entire datastore for a given address.
    ///
    /// IMPORTANT: This should only be used for debug purposes.
    ///
    /// # Returns
    /// A `BTreeMap` with the entry hash as key and the data bytes as value
    #[cfg(any(test, feature = "testing"))]
    pub fn get_entire_datastore(
        &self,
        addr: &Address,
    ) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
        let handle = self.db.cf_handle(LEDGER_CF).expect(CF_ERROR);

        let mut opt = ReadOptions::default();
        opt.set_iterate_upper_bound(end_prefix(data_prefix!(addr)).unwrap());

        self.db
            .iterator_cf_opt(
                handle,
                opt,
                IteratorMode::From(data_prefix!(addr), Direction::Forward),
            )
            .flatten()
            .map(|(key, data)| {
                (
                    key.split_at(ADDRESS_SIZE_BYTES + 1).1.to_vec(),
                    data.to_vec(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::LedgerDB;
    use crate::ledger_db::{LedgerBatch, LedgerSubEntry, LEDGER_HASH_INITIAL_BYTES};
    use massa_hash::Hash;
    use massa_ledger_exports::{LedgerEntry, LedgerEntryUpdate, SetOrKeep};
    use massa_models::{
        address::Address,
        amount::{Amount, AmountDeserializer},
        streaming_step::StreamingStep,
    };
    use massa_serialization::{DeserializeError, Deserializer};
    use massa_signature::KeyPair;
    use std::collections::BTreeMap;
    use std::ops::Bound::Included;
    use std::str::FromStr;
    use tempfile::TempDir;

    #[cfg(test)]
    fn init_test_ledger(addr: Address) -> (LedgerDB, BTreeMap<Vec<u8>, Vec<u8>>) {
        // init data
        let mut data = BTreeMap::new();
        data.insert(b"1".to_vec(), b"a".to_vec());
        data.insert(b"2".to_vec(), b"b".to_vec());
        data.insert(b"3".to_vec(), b"c".to_vec());
        let entry = LedgerEntry {
            balance: Amount::from_str("42").unwrap(),
            datastore: data.clone(),
            ..Default::default()
        };
        let entry_update = LedgerEntryUpdate {
            balance: SetOrKeep::Set(Amount::from_str("21").unwrap()),
            bytecode: SetOrKeep::Keep,
            ..Default::default()
        };

        // write data
        let temp_dir = TempDir::new().unwrap();
        let mut db = LedgerDB::new(temp_dir.path().to_path_buf(), 32, 255, 1_000_000);
        let mut batch = LedgerBatch::new(Hash::from_bytes(LEDGER_HASH_INITIAL_BYTES));
        db.put_entry(&addr, entry, &mut batch);
        db.update_entry(&addr, entry_update, &mut batch);
        db.write_batch(batch);

        // return db and initial data
        (db, data)
    }

    /// Functional test of `LedgerDB`
    #[test]
    fn test_ledger_db() {
        let addr = Address::from_public_key(&KeyPair::generate().get_public_key());
        let (db, data) = init_test_ledger(addr);
        let ledger_hash = db.get_ledger_hash();
        let amount_deserializer =
            AmountDeserializer::new(Included(Amount::MIN), Included(Amount::MAX));

        // check initial state and entry update
        assert!(db.get_sub_entry(&addr, LedgerSubEntry::Balance).is_some());
        assert_eq!(
            amount_deserializer
                .deserialize::<DeserializeError>(
                    &db.get_sub_entry(&addr, LedgerSubEntry::Balance).unwrap()
                )
                .unwrap()
                .1,
            Amount::from_str("21").unwrap()
        );
        assert_eq!(data, db.get_entire_datastore(&addr));
        assert_ne!(
            Hash::from_bytes(LEDGER_HASH_INITIAL_BYTES),
            db.get_ledger_hash()
        );

        // delete entry
        let mut batch = LedgerBatch::new(ledger_hash);
        db.delete_entry(&addr, &mut batch);
        db.write_batch(batch);

        // check deleted address and ledger hash
        assert_eq!(
            Hash::from_bytes(LEDGER_HASH_INITIAL_BYTES),
            db.get_ledger_hash()
        );
        assert!(db.get_sub_entry(&addr, LedgerSubEntry::Balance).is_none());
        assert!(db.get_entire_datastore(&addr).is_empty());
    }

    #[test]
    fn test_ledger_parts() {
        let pub_a = KeyPair::generate().get_public_key();
        let a = Address::from_public_key(&pub_a);
        let (db, _) = init_test_ledger(a);
        let res = db.get_ledger_part(StreamingStep::Started).unwrap();
        db.set_ledger_part(&res.0[..]).unwrap();
    }
}
