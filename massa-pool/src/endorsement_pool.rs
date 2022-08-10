// Copyright (c) 2022 MASSA LABS <info@massa.net>

use crate::{settings::PoolConfig, PoolError};
use massa_models::prehash::{Map, Set};
use massa_models::wrapped::Wrapped;
use massa_models::{Address, BlockId, Endorsement, EndorsementId, Slot, WrappedEndorsement};

pub struct EndorsementPool {
    endorsements: Map<EndorsementId, WrappedEndorsement>,
    latest_final_periods: Vec<u64>,
    current_slot: Option<Slot>,
    cfg: &'static PoolConfig,
}

impl EndorsementPool {
    pub fn new(cfg: &'static PoolConfig) -> EndorsementPool {
        EndorsementPool {
            endorsements: Default::default(),
            cfg,
            current_slot: None,
            latest_final_periods: vec![0; cfg.thread_count as usize],
        }
    }

    pub fn len(&self) -> usize {
        self.endorsements.len()
    }

    /// Removes endorsements that are too old
    pub fn update_latest_final_periods(&mut self, periods: Vec<u64>) {
        self.endorsements.retain(
            |_,
             Wrapped {
                 content: Endorsement { slot, .. },
                 ..
             }| slot.period >= periods[slot.thread as usize],
        );
        self.latest_final_periods = periods;
    }

    /// gets ok endorsements for a given slot, with given endorsed block and endorsement creators at index
    /// returns sorted and deduped endorsements
    pub fn get_endorsements(
        &self,
        target_slot: Slot,
        parent: BlockId,
        creators: Vec<Address>,
    ) -> Result<Vec<WrappedEndorsement>, PoolError> {
        let mut candidates = self
            .endorsements
            .iter()
            .filter_map(|(_endo_id, endorsement)| {
                let creator = &endorsement.creator_address;
                if endorsement.content.endorsed_block == parent
                    && endorsement.content.slot == target_slot
                    && creators.get(endorsement.content.index as usize) == Some(creator)
                {
                    Some(Ok(endorsement.clone()))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<WrappedEndorsement>, PoolError>>()?;
        candidates.sort_unstable_by_key(|endo| endo.content.index);
        candidates.dedup_by_key(|endo| endo.content.index);
        Ok(candidates)
    }

    /// Incoming endorsements. Returns newly added.
    /// Prunes the pool if there are too many endorsements
    pub fn add_endorsements(
        &mut self,
        endorsements: Map<EndorsementId, WrappedEndorsement>,
    ) -> Result<Set<EndorsementId>, PoolError> {
        let mut newly_added = Set::<EndorsementId>::default();
        for (endorsement_id, endorsement) in endorsements.into_iter() {
            massa_trace!("pool add_endorsements endorsement", {
                "endorsement": endorsement
            });

            // Already present
            if self.endorsements.contains_key(&endorsement_id) {
                massa_trace!("pool add_endorsement endorsement already present", {});
                continue;
            }

            // too old
            if endorsement.content.slot.period
                < self.latest_final_periods[endorsement.content.slot.thread as usize]
            {
                massa_trace!("pool add_endorsement endorsement too old", {});
                continue;
            }

            self.endorsements.insert(endorsement_id, endorsement);
            newly_added.insert(endorsement_id);
        }

        // remove excess endorsements
        let removed = self.prune();
        for id in removed.into_iter() {
            newly_added.remove(&id);
        }

        Ok(newly_added)
    }

    pub fn update_current_slot(&mut self, slot: Slot) {
        self.current_slot = Some(slot);
    }

    /// Prune the pool while there are more endorsements than set max
    /// Kept endorsements are the one that are absolutely closer to the current slot
    fn prune(&mut self) -> Set<EndorsementId> {
        let mut removed = Set::<EndorsementId>::default();

        if self.endorsements.len() > self.cfg.settings.max_endorsement_count as usize {
            let excess = self.endorsements.len() - self.cfg.settings.max_endorsement_count as usize;
            let mut candidates: Vec<_> = self.endorsements.clone().into_iter().collect();
            let thread_count = self.latest_final_periods.len() as u8;
            let current_slot_index = self.current_slot.map_or(0u64, |s| {
                s.period
                    .saturating_mul(thread_count as u64)
                    .saturating_add(s.thread as u64)
            });
            candidates.sort_unstable_by_key(|(_, e)| {
                let slot_index = e
                    .content
                    .slot
                    .period
                    .saturating_mul(thread_count as u64)
                    .saturating_add(e.content.slot.thread as u64);
                let abs_diff = if slot_index >= current_slot_index {
                    slot_index - current_slot_index
                } else {
                    current_slot_index - slot_index
                };
                std::cmp::Reverse(abs_diff)
            });
            candidates.truncate(excess);
            for (c_id, _) in candidates.into_iter() {
                self.endorsements.remove(&c_id);
                removed.insert(c_id);
            }
        }
        massa_trace!("pool.endorsement_pool.prune", { "removed": removed });
        removed
    }

    pub fn get_endorsement_by_address(
        &self,
        address: Address,
    ) -> Result<Map<EndorsementId, WrappedEndorsement>, PoolError> {
        let mut res = Map::default();
        for (id, ed) in self.endorsements.iter() {
            if ed.creator_address == address {
                res.insert(*id, ed.clone());
            }
        }
        Ok(res)
    }

    pub fn get_endorsement_by_id(
        &self,
        endorsements: Set<EndorsementId>,
    ) -> Map<EndorsementId, WrappedEndorsement> {
        self.endorsements
            .iter()
            .filter_map(|(id, ed)| {
                if endorsements.contains(id) {
                    Some((*id, ed.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}
