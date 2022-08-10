// Copyright (c) 2022 MASSA LABS <info@massa.net>

use crate::{
    array_from_slice,
    error::ModelsResult as Result,
    node_configuration::ADDRESS_SIZE_BYTES,
    prehash::{Map, Set},
    u8_from_slice, Address, Amount, DeserializeCompact, DeserializeVarInt, ModelsError,
    SerializeCompact, SerializeVarInt,
};
use core::usize;
use serde::{Deserialize, Serialize};
use std::collections::hash_map;

/// a consensus ledger entry
#[derive(Debug, Default, Deserialize, Clone, Copy, Serialize)]
pub struct LedgerData {
    /// the balance in coins
    pub balance: Amount,
}

/// Checks performed:
/// - Balance.
impl SerializeCompact for LedgerData {
    fn to_bytes_compact(&self) -> Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::new();
        res.extend(&self.balance.to_bytes_compact()?);
        Ok(res)
    }
}

/// Checks performed:
/// - Balance.
impl DeserializeCompact for LedgerData {
    fn from_bytes_compact(buffer: &[u8]) -> Result<(Self, usize)> {
        let mut cursor = 0usize;
        let (balance, delta) = Amount::from_bytes_compact(&buffer[cursor..])?;
        cursor += delta;
        Ok((LedgerData { balance }, cursor))
    }
}

impl LedgerData {
    /// new `LedgerData` from an initial balance
    pub fn new(starting_balance: Amount) -> LedgerData {
        LedgerData {
            balance: starting_balance,
        }
    }

    /// apply a `LedgerChange` for an entry
    /// Can fail in overflow or underflow occur
    pub fn apply_change(&mut self, change: &LedgerChange) -> Result<()> {
        if change.balance_increment {
            self.balance = self
                .balance
                .checked_add(change.balance_delta)
                .ok_or_else(|| {
                    ModelsError::InvalidLedgerChange(
                        "balance overflow in LedgerData::apply_change".into(),
                    )
                })?;
        } else {
            self.balance = self
                .balance
                .checked_sub(change.balance_delta)
                .ok_or_else(|| {
                    ModelsError::InvalidLedgerChange(
                        "balance overflow in LedgerData::apply_change".into(),
                    )
                })?;
        }
        Ok(())
    }

    /// returns true if the balance is zero
    pub fn is_nil(&self) -> bool {
        self.balance == Amount::default()
    }
}

/// A balance change that can be applied to an address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerChange {
    /// Amount to add or subtract
    pub balance_delta: Amount,
    /// whether to increment or decrements balance of delta
    pub balance_increment: bool,
}

impl Default for LedgerChange {
    fn default() -> Self {
        LedgerChange {
            balance_delta: Amount::default(),
            balance_increment: true,
        }
    }
}

impl LedgerChange {
    /// Applies another ledger change on top of self
    pub fn chain(&mut self, change: &LedgerChange) -> Result<(), ModelsError> {
        if self.balance_increment == change.balance_increment {
            self.balance_delta = self
                .balance_delta
                .checked_add(change.balance_delta)
                .ok_or_else(|| {
                    ModelsError::InvalidLedgerChange("overflow in LedgerChange::chain".into())
                })?;
        } else if change.balance_delta > self.balance_delta {
            self.balance_delta = change
                .balance_delta
                .checked_sub(self.balance_delta)
                .ok_or_else(|| {
                    ModelsError::InvalidLedgerChange("underflow in LedgerChange::chain".into())
                })?;
            self.balance_increment = !self.balance_increment;
        } else {
            self.balance_delta = self
                .balance_delta
                .checked_sub(change.balance_delta)
                .ok_or_else(|| {
                    ModelsError::InvalidLedgerChange("underflow in LedgerChange::chain".into())
                })?;
        }
        if self.balance_delta == Amount::default() {
            self.balance_increment = true;
        }
        Ok(())
    }

    /// true if the change is 0
    pub fn is_nil(&self) -> bool {
        self.balance_delta == Amount::default()
    }
}

/// Checks performed:
/// - Balance delta.
impl SerializeCompact for LedgerChange {
    fn to_bytes_compact(&self) -> Result<Vec<u8>, crate::ModelsError> {
        let mut res: Vec<u8> = Vec::new();
        res.push(if self.balance_increment { 1u8 } else { 0u8 });
        res.extend(&self.balance_delta.to_bytes_compact()?);
        Ok(res)
    }
}

/// Checks performed:
/// - Increment flag.
/// - Balance delta.
impl DeserializeCompact for LedgerChange {
    fn from_bytes_compact(buffer: &[u8]) -> Result<(Self, usize), crate::ModelsError> {
        let mut cursor = 0usize;

        let balance_increment = match u8_from_slice(&buffer[cursor..])? {
            0u8 => false,
            1u8 => true,
            _ => {
                return Err(ModelsError::DeserializeError(
                    "wrong boolean balance_increment encoding in LedgerChange deserialization"
                        .into(),
                ))
            }
        };
        cursor += 1;

        let (balance_delta, delta) = Amount::from_bytes_compact(&buffer[cursor..])?;
        cursor += delta;

        Ok((
            LedgerChange {
                balance_increment,
                balance_delta,
            },
            cursor,
        ))
    }
}

/// Map an address to a `LedgerChange`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LedgerChanges(pub Map<Address, LedgerChange>);

impl LedgerChanges {
    /// addresses that are impacted by these ledger changes
    pub fn get_involved_addresses(&self) -> Set<Address> {
        self.0.keys().copied().collect()
    }

    /// applies a `LedgerChange`
    pub fn apply(&mut self, addr: &Address, change: &LedgerChange) -> Result<()> {
        match self.0.entry(*addr) {
            hash_map::Entry::Occupied(mut occ) => {
                occ.get_mut().chain(change)?;
                if occ.get().is_nil() {
                    occ.remove();
                }
            }
            hash_map::Entry::Vacant(vac) => {
                let mut res = LedgerChange::default();
                res.chain(change)?;
                if !res.is_nil() {
                    vac.insert(res);
                }
            }
        }
        Ok(())
    }

    /// chain with another `LedgerChange`
    pub fn chain(&mut self, other: &LedgerChanges) -> Result<()> {
        for (addr, change) in other.0.iter() {
            self.apply(addr, change)?;
        }
        Ok(())
    }

    /// merge another ledger changes into self, overwriting existing data
    /// addresses that are in not other are removed from self
    pub fn sync_from(&mut self, addrs: &Set<Address>, mut other: LedgerChanges) {
        for addr in addrs.iter() {
            if let Some(new_val) = other.0.remove(addr) {
                self.0.insert(*addr, new_val);
            } else {
                self.0.remove(addr);
            }
        }
    }

    /// clone subset
    #[must_use]
    pub fn clone_subset(&self, addrs: &Set<Address>) -> Self {
        LedgerChanges(
            self.0
                .iter()
                .filter_map(|(a, dta)| {
                    if addrs.contains(a) {
                        Some((*a, dta.clone()))
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    /// add reward related changes
    pub fn add_reward(
        &mut self,
        creator: Address,
        endorsers: Vec<Address>,
        parent_creator: Address,
        reward: Amount,
        endorsement_count: u32,
    ) -> Result<()> {
        let endorsers_count = endorsers.len() as u64;
        let third = reward
            .checked_div_u64(3 * (1 + (endorsement_count as u64)))
            .ok_or(ModelsError::AmountOverflowError)?;
        for ed in endorsers {
            self.apply(
                &parent_creator,
                &LedgerChange {
                    balance_delta: third,
                    balance_increment: true,
                },
            )?;
            self.apply(
                &ed,
                &LedgerChange {
                    balance_delta: third,
                    balance_increment: true,
                },
            )?;
        }
        let total_credited = third
            .checked_mul_u64(2 * endorsers_count)
            .ok_or(ModelsError::AmountOverflowError)?;
        // here we credited only parent_creator and ed for every endorsement
        // total_credited now contains the total amount already credited

        let expected_credit = reward
            .checked_mul_u64(1 + endorsers_count)
            .ok_or(ModelsError::AmountOverflowError)?
            .checked_div_u64(1 + (endorsement_count as u64))
            .ok_or(ModelsError::AmountOverflowError)?;
        // here expected_credit contains the expected amount that should be credited in total
        // the difference between expected_credit and total_credited is sent to the block creator
        self.apply(
            &creator,
            &LedgerChange {
                balance_delta: expected_credit.saturating_sub(total_credited),
                balance_increment: true,
            },
        )
    }
}

impl SerializeCompact for LedgerChanges {
    /// ## Example
    /// ```rust
    /// # use massa_models::{SerializeCompact, DeserializeCompact, SerializationContext, Address, Amount};
    /// # use std::str::FromStr;
    /// # use massa_models::ledger_models::{LedgerChanges, LedgerChange};
    /// # let ledger_changes = LedgerChanges(vec![
    /// #   (
    /// #       Address::from_bs58_check("2oxLZc6g6EHfc5VtywyPttEeGDxWq3xjvTNziayWGDfxETZVTi".into()).unwrap(),
    /// #       LedgerChange {
    /// #           balance_delta: Amount::from_str("1149").unwrap(),
    /// #           balance_increment: true
    /// #       },
    /// #   ),
    /// #   (
    /// #       Address::from_bs58_check("2mvD6zEvo8gGaZbcs6AYTyWKFonZaKvKzDGRsiXhZ9zbxPD11q".into()).unwrap(),
    /// #       LedgerChange {
    /// #           balance_delta: Amount::from_str("1020").unwrap(),
    /// #           balance_increment: true
    /// #       },
    /// #   )
    /// # ].into_iter().collect());
    /// # massa_models::init_serialization_context(massa_models::SerializationContext::default());
    /// let bytes = ledger_changes.clone().to_bytes_compact().unwrap();
    /// let (res, _) = LedgerChanges::from_bytes_compact(&bytes).unwrap();
    /// for (address, data) in &ledger_changes.0 {
    ///    assert!(res.0.iter().filter(|(addr, dta)| &address == addr && dta.to_bytes_compact().unwrap() == data.to_bytes_compact().unwrap()).count() == 1)
    /// }
    /// assert_eq!(ledger_changes.0.len(), res.0.len());
    /// ```
    fn to_bytes_compact(&self) -> Result<Vec<u8>, ModelsError> {
        let mut res: Vec<u8> = Vec::new();

        let entry_count: u64 = self.0.len().try_into().map_err(|err| {
            ModelsError::SerializeError(format!(
                "too many entries in ConsensusLedgerSubset: {}",
                err
            ))
        })?;
        res.extend(entry_count.to_varint_bytes());
        for (address, data) in self.0.iter() {
            res.extend(address.to_bytes());
            res.extend(&data.to_bytes_compact()?);
        }

        Ok(res)
    }
}

impl DeserializeCompact for LedgerChanges {
    fn from_bytes_compact(buffer: &[u8]) -> Result<(Self, usize), ModelsError> {
        let mut cursor = 0usize;

        let (entry_count, delta) = u64::from_varint_bytes(&buffer[cursor..])?;
        // TODO: add entry_count checks ... see #1200
        cursor += delta;

        let mut ledger_subset = LedgerChanges(Map::default());
        for _ in 0..entry_count {
            let address = Address::from_bytes(&array_from_slice(&buffer[cursor..])?);
            cursor += ADDRESS_SIZE_BYTES;

            let (data, delta) = LedgerChange::from_bytes_compact(&buffer[cursor..])?;
            cursor += delta;

            ledger_subset.0.insert(address, data);
        }

        Ok((ledger_subset, cursor))
    }
}
