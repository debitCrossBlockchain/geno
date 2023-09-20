mod index;
pub mod pool;
mod store;
mod transaction;
pub mod types;
pub use bootstrap::start_txpool_service;
pub mod bootstrap;

use configure::TxPoolConfig;
use parking_lot::RwLock;
use std::sync::Arc;
#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref TX_POOL_INSTANCE_REF: Arc<RwLock<pool::Pool>> =
        Arc::new(RwLock::new(pool::Pool::new(&TxPoolConfig::default(), None)));
}
