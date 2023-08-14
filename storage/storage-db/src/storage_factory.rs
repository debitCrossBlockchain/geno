use configure::Db;

use crate::key_value_db::KeyValueDB;
#[cfg(target_os = "windows")]
use crate::leveldb::LevelDbDriver;
#[cfg(not(target_os = "windows"))]
use crate::rocksdb::RocksDbDriver;

#[derive(Clone)]
pub struct StorageFactory {
    pub key_value_db: KeyValueDB,
    pub ledger_db: KeyValueDB,
    pub account_db: KeyValueDB,
}

impl StorageFactory {
    pub fn initialize(db_config: &Db) -> StorageFactory {
        let max_open_files = db_config.key_vaule_max_open_files;
        let keyvaule_max_open_files = 2 + (max_open_files - 100) * 2 / 10;
        let ledger_max_open_files = 4 + (max_open_files - 100) * 4 / 10;
        let account_max_open_files = 4 + (max_open_files - 100) * 4 / 10;

        StorageFactory {
            #[cfg(target_os = "windows")]
            key_value_db: LevelDbDriver::open(
                db_config.key_value_db_path.as_str(),
                keyvaule_max_open_files,
            ),
            #[cfg(target_os = "windows")]
            ledger_db: LevelDbDriver::open(
                db_config.ledger_db_path.as_str(),
                ledger_max_open_files,
            ),
            #[cfg(target_os = "windows")]
            account_db: LevelDbDriver::open(
                db_config.account_db_path.as_str(),
                account_max_open_files,
            ),

            // for linux
            #[cfg(not(target_os = "windows"))]
            key_value_db: RocksDbDriver::open(
                &*db_config.key_value_db_path,
                keyvaule_max_open_files,
            ),
            #[cfg(not(target_os = "windows"))]
            ledger_db: RocksDbDriver::open(&*db_config.ledger_db_path, ledger_max_open_files),
            #[cfg(not(target_os = "windows"))]
            account_db: RocksDbDriver::open(&*db_config.account_db_path, account_max_open_files),
        }
    }

    //Store other data except account, ledger and transaction.
    pub fn key_value_db(&self) -> KeyValueDB {
        self.key_value_db.clone()
    }

    //Store state
    pub fn account_db(&self) -> KeyValueDB {
        self.account_db.clone()
    }

    //Store transactions and ledgers.
    pub fn ledger_db(&self) -> KeyValueDB {
        self.ledger_db.clone()
    }
}
