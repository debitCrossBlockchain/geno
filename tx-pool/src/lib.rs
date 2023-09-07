#![forbid(unsafe_code)]
// Increase recursion limit to allow for use of select! macro.
#![recursion_limit = "1024"]


mod tx_verify_pool;


mod index;
mod mempool;
mod transaction;
mod transaction_store;
mod ttl_cache;

pub mod types;
pub use tasks::bootstrap;
#[cfg(any(test, feature = "fuzzing"))]
pub(crate) use tasks::start_shared_mempool;
mod coordinator;
pub mod status;
mod message_queues;
pub mod tx_pool_channel;
pub mod tx_pool_config;
pub mod tx_validator;

pub mod tasks;

pub const TEST_TXPOOL_INCHANNEL_AND_SWPAN: bool = false;

use configure::TxPoolConfig;
use parking_lot::{Mutex, Once, RawRwLock, RwLock};
use std::sync::Arc;
#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref TxPoolInstanceRef: Arc<RwLock<CoreMempool>> = Arc::new(RwLock::new(
        CoreMempool::new(&TxPoolConfig::default(), None)
    ));
}

#[cfg(test)]
pub use self::ttl_cache::TtlCache;
pub use self::{index::TxnPointer, mempool::Mempool as CoreMempool, transaction::{TxState,TimelineState}};
