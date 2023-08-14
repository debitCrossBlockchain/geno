extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_millis;

pub mod general;
pub mod logger;
pub mod parse;
pub mod proto2json;
pub mod tbft_proof;
pub mod timer;
pub mod timer_manager;
pub mod timing;
pub use logger::{LogInstance, LogUtil};
pub use protos::ledger::TransactionSign;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref LOG_INSTANCE_REF: LogInstance = LogInstance::new();
}
