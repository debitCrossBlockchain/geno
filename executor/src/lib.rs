pub mod block_executor;
pub mod block_result;
pub mod last_commit_info;
pub mod block_verify;

use parking_lot::RwLock;

use last_commit_info::LastCommittedInfo;

pub use block_executor::BlockExecutor;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref LAST_COMMITTED_BLOCK_INFO_REF: RwLock<LastCommittedInfo> =
        RwLock::new(LastCommittedInfo::default());
}
