use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Consensus {
    pub consensus_type: String,
    pub block_max_tx_size: u64,
    pub block_max_contract_size: u64,
}

impl Default for Consensus {
    fn default() -> Self {
        Self {
            consensus_type: "pbft".to_string(),
            block_max_tx_size: 100000,
            block_max_contract_size: 2500,
        }
    }
}

impl Clone for Consensus {
    fn clone(&self) -> Self {
        Self {
            consensus_type: self.consensus_type.clone(),
            block_max_tx_size: self.block_max_tx_size,
            block_max_contract_size: self.block_max_contract_size,
        }
    }
}
