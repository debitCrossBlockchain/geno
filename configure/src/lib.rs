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
mod db;
mod fees;
mod genesis_block;
mod jsonrpc;
mod p2p_network;
mod ssl;
mod tx_pool;
mod websocket;

use config::*;
pub use configure::Configure;
pub use consensus::Consensus;
pub use db::Db;
pub use fees::Fees;
pub use genesis_block::GenesisBlock;
pub use jsonrpc::JsonRpcConfig;
pub use p2p_network::P2PNetwork;
pub use ssl::SSL;
pub use tx_pool::TxPoolConfig;
pub use websocket::WebsocketConfig;

use std::sync::Arc;
lazy_static! {
    pub static ref CONFIGURE_INSTANCE_REF: Arc<Configure> = Arc::new({
        let mut conf = Config::default();
        conf.merge(File::new("setting/config", FileFormat::Toml))
            .unwrap();
        let mut config: Configure = conf.try_into().unwrap();
        config
    });
}
