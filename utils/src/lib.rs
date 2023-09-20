extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_millis;

pub mod general;
pub mod logger;
pub mod parse;
pub mod proto2json;
pub mod signature;
pub mod timer;
pub mod timer_manager;
pub mod timing;
pub mod verify_pool;
pub mod verify_sign;
pub use logger::LogUtil;
use parking_lot::RwLock;
pub use protos::ledger::TransactionSign;

use crate::verify_pool::PoolVerify;
#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref POOL_VERIFY_REF: RwLock<PoolVerify> = RwLock::new(PoolVerify::default());
}
