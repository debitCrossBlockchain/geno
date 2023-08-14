extern crate serde;
#[cfg(test)]
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate config;
#[macro_use]
extern crate lazy_static;

mod configure;
mod consensus;
mod data_back;
mod db;
mod fees;
mod genesis_block;
mod json_rpc_config;
mod ledger;
mod metric_config;
mod p2p_network;
mod ssl;
mod tx_pool_config;
mod websocket_config;

use config::*;
pub use configure::Configure;
pub use consensus::Consensus;
pub use db::Db;
pub use fees::Fees;
pub use genesis_block::GenesisBlock;
pub use json_rpc_config::JsonRpcConfig;
pub use ledger::Ledger;
pub use metric_config::MetricConfig;
pub use p2p_network::P2PNetwork;
pub use ssl::SSL;
pub use tx_pool_config::TxPoolConfig;
pub use websocket_config::Websocket;

use parking_lot::{Mutex, RwLock};
use std::io;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::Once;
lazy_static! {
    pub static ref ConfigureInstanceRef: Arc<Configure> = Arc::new({
        let mut conf = Config::default();
        conf.merge(File::new("setting/config", FileFormat::Toml))
            .unwrap();
        let mut config: Configure = conf.try_into().unwrap();
        config.ledger.commit_interval = config.ledger.commit_interval * 1000;
        config
    });
}
