// Copyright (c) 2022 MASSA LABS <info@massa.net>

//! this file defines all possible execution error categories

use displaydoc::Display;
use thiserror::Error;

/// Errors of the execution component.
#[non_exhaustive]
#[derive(Clone, Display, Error, Debug)]
pub enum ExecutionError {
    /// Channel error
    ChannelError(String),

    /// Runtime error: {0}
    RuntimeError(String),

    /// `MassaHashError`: {0}
    MassaHashError(#[from] massa_hash::MassaHashError),

    /// `ModelsError`: {0}
    ModelsError(#[from] massa_models::error::ModelsError),

    /// `RollBuy` error: {0}
    RollBuyError(String),

    /// `RollSell` error: {0}
    RollSellError(String),

    /// `Transaction` error: {0}
    TransactionError(String),

    /// Block gas error: {0}
    BlockGasError(String),

    /// Invalid slot range
    InvalidSlotRange,

    /// Not enough gas in the block: {0}
    NotEnoughGas(String),

    /// Include operation error: {0}
    IncludeOperationError(String),
}
