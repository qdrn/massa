// Copyright (c) 2022 MASSA LABS <info@massa.net>

//! This file defines utilities to mock the crate for testing purposes

use crate::{ExecutionController, ExecutionError, ExecutionOutput, ReadOnlyExecutionRequest};
use massa_ledger_exports::LedgerEntry;
use massa_models::{api::EventFilter, output_event::SCOutputEvent, Address, Amount, BlockId, Slot};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
};

/// List of possible messages coming from the mock.
/// Each variant corresponds to a unique method in `ExecutionController`,
/// and is emitted in a thread-safe way by the mock whenever that method is called.
/// Some variants wait for a response on their `response_tx` field, if present.
/// See the documentation of `ExecutionController` for details on parameters and return values.
#[derive(Clone)]
pub enum MockExecutionControllerMessage {
    /// update blockclique status
    UpdateBlockcliqueStatus {
        /// newly finalized blocks
        finalized_blocks: HashMap<Slot, BlockId>,
        /// current clique of higher fitness
        blockclique: HashMap<Slot, BlockId>,
    },
    /// filter for smart contract output event request
    GetFilteredScOutputEvent {
        /// filter
        filter: EventFilter,
        /// response channel
        response_tx: mpsc::Sender<Vec<SCOutputEvent>>,
    },
    /// get full ledger entry
    GetFullLedgerEntry {
        /// address
        addr: Address,
        /// response channel
        response_tx: mpsc::Sender<(Option<LedgerEntry>, Option<LedgerEntry>)>,
    },
    /// read only execution request
    ExecuteReadonlyRequest {
        /// read only execution request
        req: ReadOnlyExecutionRequest,
        /// response channel
        response_tx: mpsc::Sender<Result<ExecutionOutput, ExecutionError>>,
    },
}

/// A mocked execution controller that will intercept calls on its methods
/// and emit corresponding `MockExecutionControllerMessage` messages through a MPSC in a thread-safe way.
/// For messages with a `response_tx` field, the mock will await a response through their `response_tx` channel
/// in order to simulate returning this value at the end of the call.
#[derive(Clone)]
pub struct MockExecutionController(Arc<Mutex<mpsc::Sender<MockExecutionControllerMessage>>>);

impl MockExecutionController {
    /// Create a new pair (mock execution controller, mpsc receiver for emitted messages)
    /// Note that unbounded mpsc channels are used
    pub fn new_with_receiver() -> (
        Box<dyn ExecutionController>,
        Receiver<MockExecutionControllerMessage>,
    ) {
        let (tx, rx) = mpsc::channel();
        (
            Box::new(MockExecutionController(Arc::new(Mutex::new(tx)))),
            rx,
        )
    }
}

/// Implements all the methods of the `ExecutionController` trait,
/// but simply make them emit a `MockExecutionControllerMessage`.
/// If the message contains a `response_tx`,
/// a response from that channel is read and returned as return value.
/// See the documentation of `ExecutionController` for details on each function.
impl ExecutionController for MockExecutionController {
    fn update_blockclique_status(
        &self,
        finalized_blocks: HashMap<Slot, BlockId>,
        blockclique: HashMap<Slot, BlockId>,
    ) {
        self.0
            .lock()
            .unwrap()
            .send(MockExecutionControllerMessage::UpdateBlockcliqueStatus {
                finalized_blocks,
                blockclique,
            })
            .unwrap();
    }

    fn get_filtered_sc_output_event(&self, filter: EventFilter) -> Vec<SCOutputEvent> {
        let (response_tx, response_rx) = mpsc::channel();
        self.0
            .lock()
            .unwrap()
            .send(MockExecutionControllerMessage::GetFilteredScOutputEvent {
                filter,
                response_tx,
            })
            .unwrap();
        response_rx.recv().unwrap()
    }

    fn get_final_and_active_parallel_balance(
        &self,
        _address: Vec<Address>,
    ) -> Vec<(Option<Amount>, Option<Amount>)> {
        Vec::default()
    }

    fn get_final_and_active_data_entry(
        &self,
        _: Vec<(Address, Vec<u8>)>,
    ) -> Vec<(Option<Vec<u8>>, Option<Vec<u8>>)> {
        Vec::default()
    }

    fn get_final_and_active_datastore_keys(
        &self,
        _addr: &Address,
    ) -> (BTreeSet<Vec<u8>>, BTreeSet<Vec<u8>>) {
        (BTreeSet::default(), BTreeSet::default())
    }

    fn execute_readonly_request(
        &self,
        req: ReadOnlyExecutionRequest,
    ) -> Result<ExecutionOutput, ExecutionError> {
        let (response_tx, response_rx) = mpsc::channel();
        self.0
            .lock()
            .unwrap()
            .send(MockExecutionControllerMessage::ExecuteReadonlyRequest { req, response_tx })
            .unwrap();
        response_rx.recv().unwrap()
    }

    fn clone_box(&self) -> Box<dyn ExecutionController> {
        Box::new(self.clone())
    }
}
