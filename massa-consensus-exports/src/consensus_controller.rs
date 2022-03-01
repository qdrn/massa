// Copyright (c) 2022 MASSA LABS <info@massa.net>
use massa_graph::{BlockGraphExport, BootstrapableGraph, ExportBlockStatus, Status};
use massa_models::{
    address::AddressState, api::EndorsementInfo, Endorsement, EndorsementId, OperationId,
};
use massa_models::{clique::Clique, stats::ConsensusStats};
use massa_models::{
    Address, Block, BlockId, OperationSearchResult, Slot, StakersCycleProductionStats,
};
use massa_proof_of_stake_exports::ExportProofOfStake;
use massa_protocol_exports::ProtocolEventReceiver;
use massa_signature::PrivateKey;

use std::collections::VecDeque;

use massa_models::prehash::{Map, Set};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    commands::{ConsensusCommand, ConsensusManagementCommand},
    error::ConsensusResult as Result,
    events::ConsensusEvent,
    ConsensusError,
};

#[derive(Clone)]
pub struct ConsensusCommandSender(pub mpsc::Sender<ConsensusCommand>);

impl ConsensusCommandSender {
    /// Gets all the aviable information on the block graph returning a Blockgraphexport.
    ///
    /// # Arguments
    /// * slot_start: optional slot start for slot-based filtering (included).
    /// * slot_end: optional slot end for slot-based filtering (excluded).
    pub async fn get_block_graph_status(
        &self,
        slot_start: Option<Slot>,
        slot_end: Option<Slot>,
    ) -> Result<BlockGraphExport> {
        let (response_tx, response_rx) = oneshot::channel::<BlockGraphExport>();
        massa_trace!("consensus.consensus_controller.get_block_graph_status", {});
        self.0
            .send(ConsensusCommand::GetBlockGraphStatus {
                slot_start,
                slot_end,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_block_graph_status".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_block_graph_status response read error".to_string(),
            )
        })
    }

    /// Gets all cliques.
    ///
    pub async fn get_cliques(&self) -> Result<Vec<Clique>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel::<Vec<Clique>>();
        massa_trace!("consensus.consensus_controller.get_cliques", {});
        self.0
            .send(ConsensusCommand::GetCliques(response_tx))
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_cliques".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_cliques response read error".to_string(),
            )
        })
    }

    /// Gets the whole block and its status corresponding to given hash.
    ///
    /// # Arguments
    /// * hash: hash corresponding to the block we want.
    pub async fn get_block_status(
        &self,
        block_id: BlockId,
    ) -> Result<Option<ExportBlockStatus>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel::<Option<ExportBlockStatus>>();
        massa_trace!("consensus.consensus_controller.get_active_block", {});
        self.0
            .send(ConsensusCommand::GetBlockStatus {
                block_id,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_block_status".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_block_status response read error".to_string(),
            )
        })
    }

    /// Gets the whole block corresponding to given hash.
    ///
    /// # Arguments
    /// * hash: hash corresponding to the block we want.
    pub async fn get_active_block(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Block>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel::<Option<Block>>();
        massa_trace!("consensus.consensus_controller.get_active_block", {});
        self.0
            .send(ConsensusCommand::GetActiveBlock {
                block_id,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_active_block".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_active_block response read error".to_string(),
            )
        })
    }

    /// Gets (slot, public_key) were the staker with public_key was selected for slot, between start_slot and end_slot.
    ///
    /// # Arguments
    /// * start_slot: begining of the considered interval.
    /// * end_slot: end of the considered interval.
    pub async fn get_selection_draws(
        &self,
        start: Slot,
        end: Slot,
    ) -> Result<Vec<(Slot, (Address, Vec<Address>))>, ConsensusError> {
        massa_trace!("consensus.consensus_controller.get_selection_draws", {});
        let (response_tx, response_rx) = oneshot::channel();
        self.0
            .send(ConsensusCommand::GetSelectionDraws {
                start,
                end,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_selection_draws".into(),
                )
            })?;
        let res = response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_selection_draws response read error".to_string(),
            )
        })?;
        res
    }

    pub async fn get_bootstrap_state(
        &self,
    ) -> Result<(ExportProofOfStake, BootstrapableGraph), ConsensusError> {
        let (response_tx, response_rx) =
            oneshot::channel::<(ExportProofOfStake, BootstrapableGraph)>();
        massa_trace!("consensus.consensus_controller.get_bootstrap_state", {});
        self.0
            .send(ConsensusCommand::GetBootstrapState(response_tx))
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_bootstrap_state".into(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_bootstrap_state response read error".to_string(),
            )
        })
    }
    pub async fn get_block_ids_by_creator(
        &self,
        address: Address,
    ) -> Result<Map<BlockId, Status>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!("consensus.consensus_controller.get_block_ids_by_creator", {
        });
        self.0
            .send(ConsensusCommand::GetBlockIdsByCreator {
                address,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_block_ids_by_creator".into(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_block_ids_by_creator response read error".to_string(),
            )
        })
    }

    pub async fn get_operations(
        &self,
        operation_ids: Set<OperationId>,
    ) -> Result<Map<OperationId, OperationSearchResult>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!("consensus.consensus_controller.get_operatiosn", {
            "operation_ids": operation_ids
        });
        self.0
            .send(ConsensusCommand::GetOperations {
                operation_ids,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_operations".into(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_operations response read error".to_string(),
            )
        })
    }

    /// Gets the candidate and final ledger data of a list of addresses
    pub async fn get_addresses_info(
        &self,
        addresses: Set<Address>,
    ) -> Result<Map<Address, AddressState>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel::<Map<Address, AddressState>>();
        massa_trace!("consensus.consensus_controller.get_addresses_info", {
            "addresses": addresses
        });
        self.0
            .send(ConsensusCommand::GetAddressesInfo {
                addresses,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_addresses_info".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_addresses_info response read error".to_string(),
            )
        })
    }

    /// Returns hashmap: Operation id -> if it is final
    pub async fn get_operations_involving_address(
        &self,
        address: Address,
    ) -> Result<Map<OperationId, OperationSearchResult>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!(
            "consensus.consensus_controller.get_operations_involving_address",
            { "address": address }
        );
        self.0
            .send(ConsensusCommand::GetRecentOperations {
                address,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_operations_involving_address".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_operations_involving_address response read error"
                    .to_string(),
            )
        })
    }

    pub async fn get_stats(&self) -> Result<ConsensusStats, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!("consensus.consensus_controller.get_stats", {});
        self.0
            .send(ConsensusCommand::GetStats(response_tx))
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_stats".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_stats response read error".to_string(),
            )
        })
    }

    pub async fn get_active_stakers(&self) -> Result<Map<Address, u64>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!("consensus.consensus_controller.get_active_stakers", {});
        self.0
            .send(ConsensusCommand::GetActiveStakers(response_tx))
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_active_stakers".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_active_stakers response read error".to_string(),
            )
        })
    }

    pub async fn register_staking_private_keys(
        &self,
        keys: Vec<PrivateKey>,
    ) -> Result<(), ConsensusError> {
        massa_trace!(
            "consensus.consensus_controller.register_staking_private_keys",
            {}
        );
        self.0
            .send(ConsensusCommand::RegisterStakingPrivateKeys(keys))
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError("send error consensus command".to_string())
            })
    }

    pub async fn remove_staking_addresses(
        &self,
        addresses: Set<Address>,
    ) -> Result<(), ConsensusError> {
        massa_trace!("consensus.consensus_controller.remove_staking_addresses", {
        });
        self.0
            .send(ConsensusCommand::RemoveStakingAddresses(addresses))
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError("send error consensus command".to_string())
            })
    }

    pub async fn get_staking_addresses(&self) -> Result<Set<Address>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!("consensus.consensus_controller.get_staking_addresses", {});
        self.0
            .send(ConsensusCommand::GetStakingAddressses(response_tx))
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_staking_addresses".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_staking_addresses response read error".to_string(),
            )
        })
    }

    pub async fn get_stakers_production_stats(
        &self,
        addrs: Set<Address>,
    ) -> Result<Vec<StakersCycleProductionStats>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!(
            "consensus.consensus_controller.get_stakers_production_stats",
            {}
        );
        self.0
            .send(ConsensusCommand::GetStakersProductionStats { addrs, response_tx })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_stakers_production_stats".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_stakers_production_statsresponse read error".to_string(),
            )
        })
    }

    pub async fn get_endorsements_by_address(
        &self,
        address: Address,
    ) -> Result<Map<EndorsementId, Endorsement>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!(
            "consensus.consensus_controller.get_endorsements_by_address",
            {}
        );
        self.0
            .send(ConsensusCommand::GetEndorsementsByAddress {
                address,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_endorsements_by_address".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_endorsements_by_address read error".to_string(),
            )
        })
    }

    pub async fn get_endorsements_by_id(
        &self,
        endorsements: Set<EndorsementId>,
    ) -> Result<Map<EndorsementId, EndorsementInfo>, ConsensusError> {
        let (response_tx, response_rx) = oneshot::channel();
        massa_trace!("consensus.consensus_controller.get_endorsements_by_id", {});
        self.0
            .send(ConsensusCommand::GetEndorsementsById {
                endorsements,
                response_tx,
            })
            .await
            .map_err(|_| {
                ConsensusError::SendChannelError(
                    "send error consensus command get_endorsements_by_id".to_string(),
                )
            })?;
        response_rx.await.map_err(|_| {
            ConsensusError::ReceiveChannelError(
                "consensus command get_endorsements_by_id read error".to_string(),
            )
        })
    }
}

pub struct ConsensusEventReceiver(pub mpsc::Receiver<ConsensusEvent>);

impl ConsensusEventReceiver {
    pub async fn wait_event(&mut self) -> Result<ConsensusEvent, ConsensusError> {
        let evt = self
            .0
            .recv()
            .await
            .ok_or(ConsensusError::ControllerEventError);
        evt
    }

    /// drains remaining events and returns them in a VecDeque
    /// note: events are sorted from oldest to newest
    pub async fn drain(mut self) -> VecDeque<ConsensusEvent> {
        let mut remaining_events: VecDeque<ConsensusEvent> = VecDeque::new();

        while let Some(evt) = self.0.recv().await {
            remaining_events.push_back(evt);
        }
        remaining_events
    }
}

pub struct ConsensusManager {
    pub join_handle: JoinHandle<Result<ProtocolEventReceiver, ConsensusError>>,

    pub manager_tx: mpsc::Sender<ConsensusManagementCommand>,
}

impl ConsensusManager {
    pub async fn stop(
        self,
        consensus_event_receiver: ConsensusEventReceiver,
    ) -> Result<ProtocolEventReceiver, ConsensusError> {
        massa_trace!("consensus.consensus_controller.stop", {});
        drop(self.manager_tx);
        let _remaining_events = consensus_event_receiver.drain().await;
        let protocol_event_receiver = self.join_handle.await??;

        Ok(protocol_event_receiver)
    }
}
