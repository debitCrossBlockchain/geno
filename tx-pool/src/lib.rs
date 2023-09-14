mod index;
pub mod pool;
mod store;
mod transaction;
pub mod types;
mod verify_pool;
pub use bootstrap::bootstrap;
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
