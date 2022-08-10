// Copyright (c) 2022 MASSA LABS <info@massa.net>
//! proof of stake exports
#![feature(int_roundings)]
#![warn(missing_docs)]
#![warn(unused_crate_dependencies)]

/// proof of stake errors
pub mod error;
mod export_pos;
mod settings;

use massa_models::{
    rolls::{RollUpdate, RollUpdates},
    Address, OperationType, WrappedOperation,
};

mod proof_of_stake;
pub use proof_of_stake::*;

use error::ProofOfStakeError;
pub use export_pos::{
    ExportProofOfStake, ExportProofOfStakeDeserializer, ExportProofOfStakeSerializer,
};
pub use settings::ProofOfStakeConfig;

mod thread_cycle_state;
pub use thread_cycle_state::ThreadCycleState;

/// Roll specific method on operation
pub trait OperationRollInterface {
    /// get roll related modifications
    fn get_roll_updates(&self) -> Result<RollUpdates, ProofOfStakeError>;
}

impl OperationRollInterface for WrappedOperation {
    fn get_roll_updates(&self) -> Result<RollUpdates, ProofOfStakeError> {
        let mut res = RollUpdates::default();
        match self.content.op {
            OperationType::Transaction { .. } => {}
            OperationType::RollBuy { roll_count } => {
                res.apply(
                    &Address::from_public_key(&self.creator_public_key),
                    &RollUpdate {
                        roll_purchases: roll_count,
                        roll_sales: 0,
                    },
                )?;
            }
            OperationType::RollSell { roll_count } => {
                res.apply(
                    &Address::from_public_key(&self.creator_public_key),
                    &RollUpdate {
                        roll_purchases: 0,
                        roll_sales: roll_count,
                    },
                )?;
            }
            OperationType::ExecuteSC { .. } => {}
            OperationType::CallSC { .. } => {}
        }
        Ok(res)
    }
}
