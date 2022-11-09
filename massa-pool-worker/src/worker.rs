//! Copyright (c) 2022 MASSA LABS <info@massa.net>

//! Write worker for the pools, allowing asynchronous writes.

use crate::controller_impl::{Command, PoolManagerImpl};
use crate::operation_pool::OperationPool;
use crate::{controller_impl::PoolControllerImpl, endorsement_pool::EndorsementPool};
use massa_execution_exports::ExecutionController;
use massa_pool_exports::PoolConfig;
use massa_pool_exports::{PoolController, PoolManager};
use massa_storage::Storage;
use parking_lot::RwLock;
use std::sync::mpsc::RecvError;
use std::thread;
use std::{
    sync::mpsc::{sync_channel, Receiver},
    sync::Arc,
    thread::JoinHandle,
};

/// Endorsement pool write thread instance
pub(crate) struct EndorsementPoolThread {
    /// Command reception channel
    receiver: Receiver<Command>,
    /// Shared reference to the pool
    endorsement_pool: Arc<RwLock<EndorsementPool>>,
}

impl EndorsementPoolThread {
    /// Spawns a pool writer thread, returning a join handle.
    pub(crate) fn spawn(
        receiver: Receiver<Command>,
        endorsement_pool: Arc<RwLock<EndorsementPool>>,
    ) -> JoinHandle<()> {
        let thread_builder = thread::Builder::new().name("endorsement-pool".into());
        thread_builder
            .spawn(|| {
                let this = Self {
                    receiver,
                    endorsement_pool,
                };
                this.run()
            })
            .expect("failed to spawn thread : endorsement-pool")
    }

    /// Runs the thread
    fn run(self) {
        loop {
            match self.receiver.recv() {
                Err(RecvError) => break,
                Ok(Command::Stop) => break,
                Ok(Command::AddItems(endorsements)) => {
                    self.endorsement_pool.write().add_endorsements(endorsements)
                }
                Ok(Command::NotifyFinalCsPeriods(final_cs_periods)) => self
                    .endorsement_pool
                    .write()
                    .notify_final_cs_periods(&final_cs_periods),
            }
        }
    }
}

/// Operation pool writer thread.
pub(crate) struct OperationPoolThread {
    /// Command reception channel
    receiver: Receiver<Command>,
    /// Shared reference to the operation pool
    operation_pool: Arc<RwLock<OperationPool>>,
}

impl OperationPoolThread {
    /// Spawns a pool writer thread, returning a join handle.
    pub(crate) fn spawn(
        receiver: Receiver<Command>,
        operation_pool: Arc<RwLock<OperationPool>>,
    ) -> JoinHandle<()> {
        let thread_builder = thread::Builder::new().name("operation-pool".into());
        thread_builder
            .spawn(|| {
                let this = Self {
                    receiver,
                    operation_pool,
                };
                this.run()
            })
            .expect("failed to spawn thread : operation-pool")
    }

    /// Run the thread.
    fn run(self) {
        loop {
            match self.receiver.recv() {
                Err(RecvError) => break,
                Ok(Command::Stop) => break,
                Ok(Command::AddItems(operations)) => {
                    self.operation_pool.write().add_operations(operations)
                }
                Ok(Command::NotifyFinalCsPeriods(final_cs_periods)) => self
                    .operation_pool
                    .write()
                    .notify_final_cs_periods(&final_cs_periods),
            };
        }
    }
}

/// Start pool manager and controller
#[allow(clippy::type_complexity)]
pub fn start_pool_controller(
    config: PoolConfig,
    storage: &Storage,
    execution_controller: Box<dyn ExecutionController>,
) -> (Box<dyn PoolManager>, Box<dyn PoolController>) {
    let (operations_input_sender, operations_input_receiver) = sync_channel(config.channels_size);
    let (endorsements_input_sender, endorsements_input_receiver) =
        sync_channel(config.channels_size);
    let operation_pool = Arc::new(RwLock::new(OperationPool::init(
        config,
        storage,
        execution_controller,
    )));
    let endorsement_pool = Arc::new(RwLock::new(EndorsementPool::init(config, storage)));
    let controller = PoolControllerImpl {
        _config: config,
        operation_pool: operation_pool.clone(),
        endorsement_pool: endorsement_pool.clone(),
        operations_input_sender: operations_input_sender.clone(),
        endorsements_input_sender: endorsements_input_sender.clone(),
    };

    let operations_thread_handle =
        OperationPoolThread::spawn(operations_input_receiver, operation_pool);
    let endorsements_thread_handle =
        EndorsementPoolThread::spawn(endorsements_input_receiver, endorsement_pool);

    let manager = PoolManagerImpl {
        operations_thread_handle: Some(operations_thread_handle),
        endorsements_thread_handle: Some(endorsements_thread_handle),
        operations_input_sender,
        endorsements_input_sender,
    };
    (Box::new(manager), Box::new(controller))
}
