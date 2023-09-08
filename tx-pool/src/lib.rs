#![forbid(unsafe_code)]
// Increase recursion limit to allow for use of select! macro.
#![recursion_limit = "1024"]

mod verify_pool;

mod index;
pub mod pool;
mod transaction;
mod store;
pub mod types;
pub use bootstrap::bootstrap;
pub mod status;
pub mod config;
pub mod bootstrap;

pub const TEST_TXPOOL_INCHANNEL_AND_SWPAN: bool = false;

use configure::TxPoolConfig;
use parking_lot::RwLock;
use std::sync::Arc;
#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref TxPoolInstanceRef: Arc<RwLock<pool::Pool>> = Arc::new(RwLock::new(
        pool::Pool::new(&TxPoolConfig::default(), None)
    ));
}

#[cfg(test)]
pub use self::{
    index::TxnPointer,
    pool::Pool,
    transaction::{TimelineState, TxState},
};
