use massa_consensus_exports::ConsensusCommandSender;
use massa_models::block::Block;
use massa_pool_exports::PoolController;
use massa_pos_exports::SelectorController;
use massa_protocol_exports::ProtocolCommandSender;
use massa_storage::Storage;

/// History of block production from latest to oldest
/// todo: redesign type (maybe add slots, draws...)
pub type ProductionHistory = Vec<Block>;

/// List of channels the factory will send commands to
#[derive(Clone)]
pub struct FactoryChannels {
    /// selector controller to get draws
    pub selector: Box<dyn SelectorController>,
    /// consensus controller
    pub consensus: ConsensusCommandSender,
    /// pool controller
    pub pool: Box<dyn PoolController>,
    /// protocol controller
    pub protocol: ProtocolCommandSender,
    /// storage instance
    pub storage: Storage,
}
