pub mod key_value_db;
pub mod leveldb;
pub mod memorydb;
pub mod rocksdb;
pub mod storage_factory;

use configure::CONFIGURE_INSTANCE_REF;
pub use key_value_db::{KeyValueDB, MemWriteBatch, WriteBatchTrait};
use std::sync::Arc;
pub use storage_factory::StorageFactory;

#[macro_use]
extern crate lazy_static;
lazy_static! {
    pub static ref STORAGE_INSTANCE_REF: Arc<StorageFactory> =
        Arc::new(StorageFactory::initialize(&CONFIGURE_INSTANCE_REF.db));
}
