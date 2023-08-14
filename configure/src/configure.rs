use crate::{
    ConfigureInstanceRef, Consensus, Db, Fees, GenesisBlock, JsonRpcConfig, Ledger, P2PNetwork,
    TxPoolConfig, SSL,Websocket,MetricConfig
};
use serde;
use serde::{Deserialize, Serialize};
use serde_derive;
use std::ops::Deref;
use crate::data_back::Data_back_config;

#[derive(Debug, Deserialize, Clone)]
pub struct Configure {
    pub network_id: u64,
    pub chain_id: String,
    pub chain_hub: String,
    pub ssl_enable: bool,
    pub address: String,
    pub node_private_key: String,
    pub key_version: u64,
    pub p2p_network: P2PNetwork,
    pub ssl: SSL,
    pub db: Db,
    pub genesis_block: GenesisBlock,
    pub consensus: Consensus,
    pub ledger: Ledger,
    pub fees: Fees,
    pub tx_pool_config: TxPoolConfig,
    pub json_rpc_config: JsonRpcConfig,
    pub websocket_config: Websocket,
    pub metric_config: MetricConfig,
    pub data_back_config:Data_back_config,
}

impl Default for Configure {
    fn default() -> Self {
        Self {
            ..Default::default()
        }
    }
}
