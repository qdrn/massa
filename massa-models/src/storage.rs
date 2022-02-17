use crate::prehash::Map;
use crate::{Block, BlockId, Endorsement, EndorsementId, Operation, OperationId};
use parking_lot::RwLock;
use std::sync::Arc;

pub struct StoredBlock {
    pub block: Block,
    pub serialized: Vec<u8>,
    pub serialized_header: Option<Vec<u8>>,
}

#[derive(Clone, Default)]
pub struct Storage {
    blocks: Arc<RwLock<Map<BlockId, Arc<RwLock<StoredBlock>>>>>,
    operations: Arc<RwLock<Map<OperationId, Arc<RwLock<Operation>>>>>,
    endorsements: Arc<RwLock<Map<EndorsementId, Arc<RwLock<Endorsement>>>>>,
}

impl Storage {
    pub fn store_block(&self, block_id: BlockId, block: Block, serialized: Vec<u8>) {
        // TODO: first check, and allow for, an already stored header for the block.
        let stored_block = StoredBlock {
            block,
            serialized,
            serialized_header: None,
        };
        let to_store = Arc::new(RwLock::new(stored_block));
        let mut blocks = self.blocks.write();
        blocks.insert(block_id, to_store);
    }

    pub fn retrieve_block(&self, block_id: &BlockId) -> Option<Arc<RwLock<StoredBlock>>> {
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
