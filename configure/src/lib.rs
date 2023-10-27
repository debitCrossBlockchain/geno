extern crate serde;
#[cfg(test)]
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate config;

mod configure;
mod consensus;
mod db;
mod genesis_block;
mod jsonrpc;
mod p2p_network;
mod ssl;
mod tx_pool;

use config::*;
pub use configure::Configure;
pub use consensus::Consensus;
pub use db::Db;
pub use genesis_block::GenesisBlock;
pub use jsonrpc::JsonRpcConfig;
use once_cell::sync::Lazy;
pub use p2p_network::P2PNetwork;
use parking_lot::RwLock;
pub use ssl::SSL;
pub use tx_pool::TxPoolConfig;

pub static CONFIG_FILE_PATH: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new("./setting/config.toml".to_string()));

pub fn parse_config(file_path: &str) -> Configure {
    let mut conf = Config::default();
    let file = File::new(file_path, FileFormat::Toml);
    match conf.merge(file) {
        Ok(_) => {}
        Err(e) => {
            panic!("config error:{:?}", e);
        }
    }
    let config: Configure = match conf.try_into() {
        Ok(config) => config,
        Err(e) => {
            panic!("config error:{:?}", e);
        }
    };
    config
}

pub static CONFIGURE_INSTANCE_REF: Lazy<Configure> =
    Lazy::new(|| parse_config(CONFIG_FILE_PATH.read().as_str()));
