// Copyright (c) 2022 MASSA LABS <info@massa.net>
use massa_consensus_exports::{
    commands::ConsensusCommand,
    error::{ConsensusError, ConsensusResult as Result},
    settings::ConsensusWorkerChannels,
    ConsensusConfig,
};
use massa_graph::{BlockGraph, BlockGraphExport};
use massa_models::timeslots::{get_block_slot_timestamp, get_latest_block_slot_at_timestamp};
use massa_models::{address::Address, block::BlockId, slot::Slot};
use massa_models::{block::WrappedHeader, prehash::PreHashMap};
use massa_models::{prehash::PreHashSet, stats::ConsensusStats};
use massa_protocol_exports::{ProtocolEvent, ProtocolEventReceiver};
use massa_storage::Storage;
use massa_time::MassaTime;
use std::{
    cmp::max,
    collections::{HashMap, VecDeque},
};
use tokio::time::{sleep, sleep_until, Sleep};
use tracing::{info, warn};

#[cfg(not(feature = "sandbox"))]
use massa_consensus_exports::events::ConsensusEvent;
#[cfg(not(feature = "sandbox"))]
use tokio::sync::mpsc::error::SendTimeoutError;
#[cfg(not(feature = "sandbox"))]
use tracing::debug;

/// Manages consensus.
pub struct ConsensusWorker {
    /// Consensus Configuration
    cfg: ConsensusConfig,
    /// Associated channels, sender and receivers
    channels: ConsensusWorkerChannels,
    /// Database containing all information about blocks, the `BlockGraph` and cliques.
    block_db: BlockGraph,
    /// Previous slot.
    previous_slot: Option<Slot>,
    /// Next slot
    next_slot: Slot,
    /// blocks we want
    wishlist: PreHashMap<BlockId, Option<WrappedHeader>>,
    /// latest final periods
    latest_final_periods: Vec<u64>,
    /// clock compensation
    clock_compensation: i64,
    /// Final block stats `(time, creator, is_from_protocol)`
    final_block_stats: VecDeque<(MassaTime, Address, bool)>,
    /// Blocks that come from protocol used for stats and ids are removed when inserted in `final_block_stats`
    protocol_blocks: VecDeque<(MassaTime, BlockId)>,
    /// Stale block timestamp
    stale_block_stats: VecDeque<MassaTime>,
    /// the time span considered for stats
    stats_history_timespan: MassaTime,
    /// the time span considered for desynchronization detection
    #[allow(dead_code)]
    stats_desync_detection_timespan: MassaTime,
    /// time at which the node was launched (used for desynchronization detection)
    launch_time: MassaTime,
    /// previous blockclique notified to Execution
    prev_blockclique: PreHashMap<BlockId, Slot>,
}

impl ConsensusWorker {
    /// Creates a new consensus controller.
    /// Initiates the random selector.
    ///
    /// # Arguments
    /// * `cfg`: consensus configuration.
    /// * `protocol_command_sender`: associated protocol controller
    /// * `block_db`: Database containing all information about blocks, the blockgraph and cliques.
    /// * `controller_command_rx`: Channel receiving consensus commands.
    /// * `controller_event_tx`: Channel sending out consensus events.
    /// * `controller_manager_rx`: Channel receiving consensus management commands.
    pub(crate) async fn new(
        cfg: ConsensusConfig,
        channels: ConsensusWorkerChannels,
        block_db: BlockGraph,
        clock_compensation: i64,
    ) -> Result<ConsensusWorker> {
        let now = MassaTime::now(clock_compensation)?;
        let previous_slot = get_latest_block_slot_at_timestamp(
            cfg.thread_count,
            cfg.t0,
            cfg.genesis_timestamp,
            now,
        )?;
        let next_slot = previous_slot.map_or(Ok(Slot::new(0u64, 0u8)), |s| {
            s.get_next_slot(cfg.thread_count)
        })?;
        let latest_final_periods: Vec<u64> = block_db
            .get_latest_final_blocks_periods()
            .iter()
            .map(|(_block_id, period)| *period)
            .collect();
        info!(
            "Started node at time {}, cycle {}, period {}, thread {}",
            now.to_utc_string(),
            next_slot.get_cycle(cfg.periods_per_cycle),
            next_slot.period,
            next_slot.thread,
        );
        if cfg.genesis_timestamp > now {
            let (days, hours, mins, secs) = cfg
                .genesis_timestamp
                .saturating_sub(now)
                .days_hours_mins_secs()?;
            info!(
                "{} days, {} hours, {} minutes, {} seconds remaining to genesis",
                days, hours, mins, secs,
            )
        }
        massa_trace!("consensus.consensus_worker.new", {});

        // desync detection timespan
        let stats_desync_detection_timespan = cfg.t0.checked_mul(cfg.periods_per_cycle * 2)?;

        // Notify execution module of current blockclique and all final blocks.
        // we need to do this because the bootstrap snapshots of the executor vs the consensus may not have been taken in sync
        // because the two modules run concurrently and out of sync.
        let mut block_storage: PreHashMap<BlockId, Storage> = Default::default();
        let notify_finals: HashMap<Slot, BlockId> = block_db
            .get_all_final_blocks()
            .into_iter()
            .map(|(b_id, slot)| {
                let (_a_block, storage) = block_db
                    .get_active_block(&b_id)
                    .expect("active block missing from block_db");
                block_storage.insert(b_id, storage.clone());
                (slot, b_id)
            })
            .collect();
        let notify_blockclique: HashMap<Slot, BlockId> = block_db
            .get_blockclique()
            .iter()
            .map(|b_id| {
                let (a_block, storage) = block_db
                    .get_active_block(b_id)
                    .expect("active block missing from block_db");
                let slot = a_block.slot;
                block_storage.insert(*b_id, storage.clone());
                (slot, *b_id)
            })
            .collect();
        let prev_blockclique: PreHashMap<BlockId, Slot> =
            notify_blockclique.iter().map(|(k, v)| (*v, *k)).collect();
        channels.execution_controller.update_blockclique_status(
            notify_finals,
            Some(notify_blockclique),
            block_storage,
        );

        Ok(ConsensusWorker {
            block_db,
            previous_slot,
            next_slot,
            wishlist: Default::default(),
            latest_final_periods,
            clock_compensation,
            channels,
            final_block_stats: Default::default(),
            protocol_blocks: Default::default(),
            stale_block_stats: VecDeque::new(),
            stats_desync_detection_timespan,
            stats_history_timespan: max(stats_desync_detection_timespan, cfg.stats_timespan),
            cfg,
            launch_time: MassaTime::now(clock_compensation)?,
            prev_blockclique,
        })
    }

    /// Consensus work is managed here.
    /// It's mostly a tokio::select within a loop.
    pub async fn run_loop(mut self) -> Result<ProtocolEventReceiver> {
        // signal initial state to pool
        self.channels
            .pool_command_sender
            .notify_final_cs_periods(&self.latest_final_periods);

        // set slot timer
        let slot_deadline = get_block_slot_timestamp(
            self.cfg.thread_count,
            self.cfg.t0,
            self.cfg.genesis_timestamp,
            self.next_slot,
        )?
        .estimate_instant(self.clock_compensation)?;
        let next_slot_timer = sleep_until(tokio::time::Instant::from(slot_deadline));

        tokio::pin!(next_slot_timer);

        // set prune timer
        let prune_timer = sleep(self.cfg.block_db_prune_interval.to_duration());
        tokio::pin!(prune_timer);

        loop {
            massa_trace!("consensus.consensus_worker.run_loop.select", {});
            /*
                select! without the "biased" modifier will randomly select the 1st branch to check,
                then will check the next ones in the order they are written.
                We choose this order:
                    * manager commands: low freq, avoid having to wait to stop
                    * consensus commands (low to medium freq): respond quickly
                    * slot timer (low freq, timing is important but does not have to be perfect either)
                    * prune timer: low freq, timing not important but should not wait too long
                    * receive protocol events (high freq)
            */
            tokio::select! {
                // listen to manager commands
                cmd = self.channels.controller_manager_rx.recv() => {
                    massa_trace!("consensus.consensus_worker.run_loop.select.manager", {});
                    match cmd {
                    None => break,
                    Some(_) => {}
                }}

                // listen consensus commands
                Some(cmd) = self.channels.controller_command_rx.recv() => {
                    massa_trace!("consensus.consensus_worker.run_loop.consensus_command", {});
                    self.process_consensus_command(cmd).await?
                },

                // slot timer
                _ = &mut next_slot_timer => {
                    massa_trace!("consensus.consensus_worker.run_loop.select.slot_tick", {});
                    if let Some(end) = self.cfg.end_timestamp {
                        if MassaTime::now(self.clock_compensation)? > end {
                            info!("This episode has come to an end, please get the latest testnet node version to continue");
                            break;
                        }
                    }
                    self.slot_tick(&mut next_slot_timer).await?;
                },

                // prune timer
                _ = &mut prune_timer=> {
                    massa_trace!("consensus.consensus_worker.run_loop.prune_timer", {});
                    // prune block db
                    let _discarded_final_blocks = self.block_db.prune()?;

                    // reset timer
                    prune_timer.set(sleep( self.cfg.block_db_prune_interval.to_duration()))
                }

                // receive protocol controller events
                evt = self.channels.protocol_event_receiver.wait_event() =>{
                    massa_trace!("consensus.consensus_worker.run_loop.select.protocol_event", {});
                    match evt {
                        Ok(event) => self.process_protocol_event(event).await?,
                        Err(err) => return Err(ConsensusError::ProtocolError(Box::new(err)))
                    }
                },
            }
        }
        // after this curly brace you can find the end of the loop
        Ok(self.channels.protocol_event_receiver)
    }

    /// this function is called around every slot tick
    /// it checks for cycle increment
    /// detects desynchronization
    /// produce quite more logs than actual stuff
    async fn slot_tick(&mut self, next_slot_timer: &mut std::pin::Pin<&mut Sleep>) -> Result<()> {
        let now = MassaTime::now(self.clock_compensation)?;
        let observed_slot = get_latest_block_slot_at_timestamp(
            self.cfg.thread_count,
            self.cfg.t0,
            self.cfg.genesis_timestamp,
            now,
        )?;

        if observed_slot < Some(self.next_slot) {
            // reset timer for next slot
            let sleep_deadline = get_block_slot_timestamp(
                self.cfg.thread_count,
                self.cfg.t0,
                self.cfg.genesis_timestamp,
                self.next_slot,
            )?
            .estimate_instant(self.clock_compensation)?;
            next_slot_timer.set(sleep_until(tokio::time::Instant::from(sleep_deadline)));
            return Ok(());
        }

        let observed_slot = observed_slot.unwrap(); // does not panic, checked above

        massa_trace!("consensus.consensus_worker.slot_tick", {
            "slot": observed_slot
        });

        let previous_cycle = self
            .previous_slot
            .map(|s| s.get_cycle(self.cfg.periods_per_cycle));
        let observed_cycle = observed_slot.get_cycle(self.cfg.periods_per_cycle);
        if previous_cycle.is_none() {
            // first cycle observed
            info!("Massa network has started ! 🎉")
        }
        if previous_cycle < Some(observed_cycle) {
            info!("Started cycle {}", observed_cycle);
        }

        // check if there are any final blocks is coming from protocol
        // if none => we are probably desync
        #[cfg(not(feature = "sandbox"))]
        if now
            > max(self.cfg.genesis_timestamp, self.launch_time)
                .saturating_add(self.stats_desync_detection_timespan)
            && !self
                .final_block_stats
                .iter()
                .any(|(time, _, is_from_protocol)| {
                    time > &now.saturating_sub(self.stats_desync_detection_timespan)
                        && *is_from_protocol
                })
        {
            warn!("desynchronization detected because the recent final block history is empty or contains only blocks produced by this node");
            let _ = self.send_consensus_event(ConsensusEvent::NeedSync).await;
        }

        self.previous_slot = Some(observed_slot);
        self.next_slot = observed_slot.get_next_slot(self.cfg.thread_count)?;

        // signal tick to block graph
        self.block_db.slot_tick(Some(observed_slot))?;

        // take care of block db changes
        self.block_db_changed().await?;

        // reset timer for next slot
        let sleep_deadline = get_block_slot_timestamp(
            self.cfg.thread_count,
            self.cfg.t0,
            self.cfg.genesis_timestamp,
            self.next_slot,
        )?
        .estimate_instant(self.clock_compensation)?;
        next_slot_timer.set(sleep_until(tokio::time::Instant::from(sleep_deadline)));

        // prune stats
        self.prune_stats()?;

        Ok(())
    }

    /// Manages given consensus command.
    /// They can come from the API or the bootstrap server
    /// Please refactor me
    ///
    /// # Argument
    /// * `cmd`: consensus command to process
    async fn process_consensus_command(&mut self, cmd: ConsensusCommand) -> Result<()> {
        match cmd {
            ConsensusCommand::GetBlockGraphStatus {
                slot_start,
                slot_end,
                response_tx,
            } => {
                massa_trace!(
                    "consensus.consensus_worker.process_consensus_command.get_block_graph_status",
                    {}
                );
                if response_tx
                    .send(BlockGraphExport::extract_from(
                        &self.block_db,
                        slot_start,
                        slot_end,
                    )?)
                    .is_err()
                {
                    warn!("consensus: could not send GetBlockGraphStatus answer");
                }
                Ok(())
            }
            // gets the graph status of a batch of blocks
            ConsensusCommand::GetBlockStatuses { ids, response_tx } => {
                massa_trace!(
                    "consensus.consensus_worker.process_consensus_command.get_block_statuses",
                    {}
                );
                let res: Vec<_> = ids
                    .iter()
                    .map(|id| self.block_db.get_block_status(id))
                    .collect();
                if response_tx.send(res).is_err() {
                    warn!("consensus: could not send get_block_statuses answer");
                }
                Ok(())
            }
            ConsensusCommand::GetCliques(response_tx) => {
                massa_trace!(
                    "consensus.consensus_worker.process_consensus_command.get_cliques",
                    {}
                );
                if response_tx.send(self.block_db.get_cliques()).is_err() {
                    warn!("consensus: could not send GetSelectionDraws response");
                }
                Ok(())
            }
            ConsensusCommand::GetBootstrapState(response_tx) => {
                massa_trace!(
                    "consensus.consensus_worker.process_consensus_command.get_bootstrap_state",
                    {}
                );
                let resp = self.block_db.export_bootstrap_graph()?;
                if response_tx.send(Box::new(resp)).await.is_err() {
                    warn!("consensus: could not send GetBootstrapState answer");
                }
                Ok(())
            }
            ConsensusCommand::GetStats(response_tx) => {
                massa_trace!(
                    "consensus.consensus_worker.process_consensus_command.get_stats",
                    {}
                );
                let res = self.get_stats()?;
                if response_tx.send(res).is_err() {
                    warn!("consensus: could not send get_stats response");
                }
                Ok(())
            }
            ConsensusCommand::GetBestParents { response_tx } => {
                if response_tx
                    .send(self.block_db.get_best_parents().clone())
                    .is_err()
                {
                    warn!("consensus: could not send get best parents response");
                }
                Ok(())
            }
            ConsensusCommand::GetBlockcliqueBlockAtSlot { slot, response_tx } => {
                let res = self.block_db.get_blockclique_block_at_slot(&slot);
                if response_tx.send(res).is_err() {
                    warn!("consensus: could not send get block clique block at slot response");
                }
                Ok(())
            }
            ConsensusCommand::GetLatestBlockcliqueBlockAtSlot { slot, response_tx } => {
                let res = self.block_db.get_latest_blockclique_block_at_slot(&slot);
                if response_tx.send(res).is_err() {
                    warn!(
                        "consensus: could not send get latest block clique block at slot response"
                    );
                }
                Ok(())
            }
            ConsensusCommand::SendBlock {
                block_id,
                slot,
                block_storage,
                response_tx,
            } => {
                self.block_db
                    .incoming_block(block_id, slot, self.previous_slot, block_storage)?;

                if response_tx.send(()).is_err() {
                    warn!("consensus: could not send get block clique block at slot response");
                }
                Ok(())
            }
        }
    }

    /// retrieve stats
    /// Used in response to a API request
    fn get_stats(&mut self) -> Result<ConsensusStats> {
        let timespan_end = max(self.launch_time, MassaTime::now(self.clock_compensation)?);
        let timespan_start = max(
            timespan_end.saturating_sub(self.cfg.stats_timespan),
            self.launch_time,
        );
        let final_block_count = self
            .final_block_stats
            .iter()
            .filter(|(t, _, _)| *t >= timespan_start && *t < timespan_end)
            .count() as u64;
        let stale_block_count = self
            .stale_block_stats
            .iter()
            .filter(|t| **t >= timespan_start && **t < timespan_end)
            .count() as u64;
        let clique_count = self.block_db.get_clique_count() as u64;
        Ok(ConsensusStats {
            final_block_count,
            stale_block_count,
            clique_count,
            start_timespan: timespan_start,
            end_timespan: timespan_end,
        })
    }

    /// Manages received protocol events.
    ///
    /// # Arguments
    /// * `event`: event type to process.
    async fn process_protocol_event(&mut self, event: ProtocolEvent) -> Result<()> {
        match event {
            ProtocolEvent::ReceivedBlock {
                block_id,
                slot,
                storage,
            } => {
                massa_trace!(
                    "consensus.consensus_worker.process_protocol_event.received_block",
                    { "block_id": block_id }
                );
                self.block_db
                    .incoming_block(block_id, slot, self.previous_slot, storage)?;
                let now = MassaTime::now(self.clock_compensation)?;
                self.protocol_blocks.push_back((now, block_id));
                self.block_db_changed().await?;
            }
            ProtocolEvent::ReceivedBlockHeader { block_id, header } => {
                massa_trace!("consensus.consensus_worker.process_protocol_event.received_header", { "block_id": block_id, "header": header });
                self.block_db
                    .incoming_header(block_id, header, self.previous_slot)?;
                self.block_db_changed().await?;
            }
            ProtocolEvent::InvalidBlock { block_id, header } => {
                massa_trace!(
                    "consensus.consensus_worker.process_protocol_event.invalid_block",
                    { "block_id": block_id }
                );
                self.block_db.invalid_block(&block_id, header)?;
                // Say it to consensus
            }
        }
        Ok(())
    }

    /// prune statistics according to the stats span
    fn prune_stats(&mut self) -> Result<()> {
        let start_time =
            MassaTime::now(self.clock_compensation)?.saturating_sub(self.stats_history_timespan);
        while let Some((t, _, _)) = self.final_block_stats.front() {
            if t < &start_time {
                self.final_block_stats.pop_front();
            } else {
                break;
            }
        }
        while let Some(t) = self.stale_block_stats.front() {
            if t < &start_time {
                self.stale_block_stats.pop_front();
            } else {
                break;
            }
        }
        while let Some((t, _)) = self.protocol_blocks.front() {
            if t < &start_time {
                self.protocol_blocks.pop_front();
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Notify execution about blockclique changes and finalized blocks.
    fn notify_execution(&mut self, finalized_blocks: HashMap<Slot, BlockId>) {
        // List new block storage instances that Execution doesn't know about.
        // That's blocks that have not been sent to execution before, ie. in the previous blockclique).
        let mut new_blocks_storage: PreHashMap<BlockId, Storage> = finalized_blocks
            .iter()
            .filter_map(|(_slot, b_id)| {
                if self.prev_blockclique.contains_key(b_id) {
                    // was previously sent as a blockclique element
                    return None;
                }
                let (_a_block, storage) = self
                    .block_db
                    .get_active_block(b_id)
                    .expect("final block not found in active blocks");
                Some((*b_id, storage.clone()))
            })
            .collect();

        // Get new blockclique block list with slots.
        let mut blockclique_changed = false;
        let new_blockclique: PreHashMap<BlockId, Slot> = self
            .block_db
            .get_blockclique()
            .iter()
            .map(|b_id| {
                if let Some(slot) = self.prev_blockclique.remove(b_id) {
                    // The block was already sent in the previous blockclique:
                    // the slot can be gathered from there without locking Storage.
                    // Note: the block is removed from self.prev_blockclique.
                    (*b_id, slot)
                } else {
                    // The block was not present in the previous blockclique:
                    // the blockclique has changed => get the block's slot by querying Storage.
                    blockclique_changed = true;
                    let (a_block, storage) = self
                        .block_db
                        .get_active_block(b_id)
                        .expect("blockclique block not found in active blocks");
                    new_blocks_storage.insert(*b_id, storage.clone());
                    (*b_id, a_block.slot)
                }
            })
            .collect();
        if !self.prev_blockclique.is_empty() {
            // All elements present in the new blockclique have been removed from `prev_blockclique` above.
            // If `prev_blockclique` is not empty here, it means that it contained elements that are not in the new blockclique anymore.
            // In that case, we mark the blockclique as having changed.
            blockclique_changed = true;
        }
        // Overwrite previous blockclique.
        // Should still be done even if unchanged because elements were removed from it above.
        self.prev_blockclique = new_blockclique.clone();

        if finalized_blocks.is_empty() && !blockclique_changed {
            // There are no changes (neither block finalizations not blockclique changes) to send to execution.
            return;
        }

        // Notify execution of block finalizations and blockclique changes
        self.channels
            .execution_controller
            .update_blockclique_status(
                finalized_blocks,
                if blockclique_changed {
                    Some(new_blockclique.into_iter().map(|(k, v)| (v, k)).collect())
                } else {
                    None
                },
                new_blocks_storage,
            );
    }

    /// call me if the block database changed
    /// Processing of final blocks, pruning.
    ///
    /// 1. propagate blocks
    /// 2. Notify of attack attempts
    /// 3. get new final blocks
    /// 4. get blockclique
    /// 5. notify Execution
    /// 6. Process new final blocks
    /// 7. Notify pool of new final ops
    /// 8. Notify PoS of final blocks
    /// 9. notify protocol of block wish list
    /// 10. note new latest final periods (prune graph if changed)
    /// 11. add stale blocks to stats
    async fn block_db_changed(&mut self) -> Result<()> {
        massa_trace!("consensus.consensus_worker.block_db_changed", {});

        // Propagate new blocks
        for (block_id, storage) in self.block_db.get_blocks_to_propagate().into_iter() {
            massa_trace!("consensus.consensus_worker.block_db_changed.integrated", {
                "block_id": block_id
            });
            self.channels
                .protocol_command_sender
                .integrated_block(block_id, storage)
                .await?;
        }

        // Notify protocol of attack attempts.
        for hash in self.block_db.get_attack_attempts().into_iter() {
            self.channels
                .protocol_command_sender
                .notify_block_attack(hash)
                .await?;
            massa_trace!("consensus.consensus_worker.block_db_changed.attack", {
                "hash": hash
            });
        }

        // manage finalized blocks
        let timestamp = MassaTime::now(self.clock_compensation)?;
        let finalized_blocks = self.block_db.get_new_final_blocks();
        let mut final_block_slots = HashMap::with_capacity(finalized_blocks.len());
        for b_id in finalized_blocks {
            if let Some((a_block, _block_store)) = self.block_db.get_active_block(&b_id) {
                // add to final blocks to notify execution
                final_block_slots.insert(a_block.slot, b_id);

                // add to stats
                let block_is_from_protocol = self
                    .protocol_blocks
                    .iter()
                    .any(|(_, block_id)| block_id == &b_id);
                self.final_block_stats.push_back((
                    timestamp,
                    a_block.creator_address,
                    block_is_from_protocol,
                ));
            }
        }

        // notify execution
        self.notify_execution(final_block_slots);

        // notify protocol of block wishlist
        let new_wishlist = self.block_db.get_block_wishlist()?;
        let new_blocks: PreHashMap<BlockId, Option<WrappedHeader>> = new_wishlist
            .iter()
            .filter_map(|(id, header)| {
                if !self.wishlist.contains_key(id) {
                    Some((*id, header.clone()))
                } else {
                    None
                }
            })
            .collect();
        let remove_blocks: PreHashSet<BlockId> = self
            .wishlist
            .iter()
            .filter_map(|(id, _)| {
                if !new_wishlist.contains_key(id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        if !new_blocks.is_empty() || !remove_blocks.is_empty() {
            massa_trace!("consensus.consensus_worker.block_db_changed.send_wishlist_delta", { "new": new_wishlist, "remove": remove_blocks });
            self.channels
                .protocol_command_sender
                .send_wishlist_delta(new_blocks, remove_blocks)
                .await?;
            self.wishlist = new_wishlist;
        }

        // note new latest final periods
        let latest_final_periods: Vec<u64> = self
            .block_db
            .get_latest_final_blocks_periods()
            .iter()
            .map(|(_block_id, period)| *period)
            .collect();
        // if changed...
        if self.latest_final_periods != latest_final_periods {
            // signal new last final periods to pool
            self.channels
                .pool_command_sender
                .notify_final_cs_periods(&latest_final_periods);
            // update final periods
            self.latest_final_periods = latest_final_periods;
        }

        // add stale blocks to stats
        let new_stale_block_ids_creators_slots = self.block_db.get_new_stale_blocks();
        let timestamp = MassaTime::now(self.clock_compensation)?;
        for (_b_id, (_b_creator, _b_slot)) in new_stale_block_ids_creators_slots.into_iter() {
            self.stale_block_stats.push_back(timestamp);

            /*
            TODO add this again
            let creator_addr = Address::from_public_key(&b_creator);
            if self.staking_keys.contains_key(&creator_addr) {
                warn!("block {} that was produced by our address {} at slot {} became stale. This is probably due to a temporary desynchronization.", b_id, creator_addr, b_slot);
            }
            */
        }

        Ok(())
    }

    /// Channel management stuff
    /// todo delete
    /// or at least introduce some generic
    #[cfg(not(feature = "sandbox"))]
    async fn send_consensus_event(&self, event: ConsensusEvent) -> Result<()> {
        let result = self
            .channels
            .controller_event_tx
            .send_timeout(event, self.cfg.max_send_wait.to_duration())
            .await;
        match result {
            Ok(()) => return Ok(()),
            Err(SendTimeoutError::Closed(event)) => {
                debug!(
                    "failed to send ConsensusEvent due to channel closure: {:?}",
                    event
                );
            }
            Err(SendTimeoutError::Timeout(event)) => {
                debug!("failed to send ConsensusEvent due to timeout: {:?}", event);
            }
        }
        Err(ConsensusError::ChannelError("failed to send event".into()))
    }
}
