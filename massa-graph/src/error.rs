// Copyright (c) 2022 MASSA LABS <info@massa.net>
use displaydoc::Display;
use massa_execution_exports::ExecutionError;
use massa_models::error::ModelsError;
use std::array::TryFromSliceError;
use thiserror::Error;

/// Result used in the graph
pub type GraphResult<T, E = GraphError> = core::result::Result<T, E>;

/// Result used in the ledger
pub type LedgerResult<T, E = LedgerError> = core::result::Result<T, E>;

/// Graph error
#[non_exhaustive]
#[derive(Display, Error, Debug)]
pub enum GraphError {
    /// execution error: {0}
    ExecutionError(#[from] ExecutionError),
    /// models error: {0}
    ModelsError(#[from] ModelsError),
    /// Could not create genesis block {0}
    GenesisCreationError(String),
    /// missing block {0}
    MissingBlock(String),
    /// missing operation {0}
    MissingOperation(String),
    /// there was an inconsistency between containers {0}
    ContainerInconsistency(String),
    /// fitness overflow
    FitnessOverflow,
    /// invalid ledger change: {0}
    InvalidLedgerChange(String),
    /// io error {0}
    IOError(#[from] std::io::Error),
    /// serde error
    SerdeError(#[from] serde_json::Error),
    /// Proof of stake cycle unavailable {0}
    PosCycleUnavailable(String),
    /// Ledger error {0}
    LedgerError(#[from] LedgerError),
    /// transaction error {0}
    TransactionError(String),
}

/// Internal error
#[non_exhaustive]
#[derive(Display, Error, Debug)]
pub enum InternalError {
    /// transaction error {0}
    TransactionError(String),
}

/// Ledger error
#[non_exhaustive]
#[derive(Display, Error, Debug)]
pub enum LedgerError {
    /// amount overflow
    AmountOverflowError,
    /// ledger inconsistency error {0}
    LedgerInconsistency(String),
    /// models error: {0}
    ModelsError(#[from] ModelsError),
    /// try from slice error {0}
    TryFromSliceError(#[from] TryFromSliceError),
    /// io error {0}
    IOError(#[from] std::io::Error),
    /// serde error
    SerdeError(#[from] serde_json::Error),
}
