use crate::{Consensus, Db, GenesisBlock, JsonRpcConfig, P2PNetwork, TxPoolConfig, SSL};
use serde;
use serde::Deserialize;
#[derive(Debug, Deserialize, Clone)]
pub struct Configure {
    pub network_id: u64,
    pub chain_id: String,
    pub chain_hub: String,
    pub ssl_enable: bool,
    pub node_address: String,
    pub node_private_key: String,
    pub key_version: u64,
    pub p2p_network: P2PNetwork,
    pub ssl: SSL,
    pub db: Db,
    pub genesis_block: GenesisBlock,
    pub consensus: Consensus,
    pub tx_pool: TxPoolConfig,
    pub json_rpc: JsonRpcConfig,
}
