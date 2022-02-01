use crate::prehash::Map;
use crate::{Block, BlockId, Endorsement, EndorsementId, Operation, OperationId};
use parking_lot::RwLock;
use std::sync::Arc;

pub struct Storage {
    blocks: Arc<RwLock<Map<BlockId, Arc<RwLock<Block>>>>>,
    operations: Arc<RwLock<Map<OperationId, Arc<RwLock<Operation>>>>>,
    endorsements: Arc<RwLock<Map<EndorsementId, Arc<RwLock<Endorsement>>>>>,
}

impl Storage {
    pub fn store_block(&self, block_id: BlockId, block: Block) {
        let to_store = Arc::new(RwLock::new(block));
        let mut blocks = self.blocks.write();
        blocks.insert(block_id, to_store);
    }

    pub fn retrieve_block(&self, block_id: &BlockId) -> Option<Arc<RwLock<Block>>> {
        let blocks = self.blocks.read();
        if let Some(block) = blocks.get(block_id) {
            return Some(Arc::clone(block));
        }
        None
    }

    pub fn store_operation(&self, operation_id: OperationId, operation: Operation) {
        let to_store = Arc::new(RwLock::new(operation));
        let mut operations = self.operations.write();
        operations.insert(operation_id, to_store);
    }

    pub fn retrieve_operation(&self, operation_id: &OperationId) -> Option<Arc<RwLock<Operation>>> {
        let operations = self.operations.read();
        if let Some(operation) = operations.get(operation_id) {
            return Some(Arc::clone(operation));
        }
        None
    }

    pub fn store_endorsement(&self, endorsement_id: EndorsementId, endorsement: Endorsement) {
        let to_store = Arc::new(RwLock::new(endorsement));
        let mut endorsements = self.endorsements.write();
        endorsements.insert(endorsement_id, to_store);
    }

    pub fn retrieve_endorsement(
        &self,
        endorsement_id: &EndorsementId,
    ) -> Option<Arc<RwLock<Endorsement>>> {
        let endorsements = self.endorsements.read();
        if let Some(endorsement) = endorsements.get(endorsement_id) {
            return Some(Arc::clone(endorsement));
        }
        None
    }
}
