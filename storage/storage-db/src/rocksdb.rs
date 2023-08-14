use crate::key_value_db::{KeyValueDB, KeyValueDb, MemWriteBatch};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
#[cfg(not(target_os = "windows"))]
use rocksdb::{BlockBasedOptions, DBCompactionStyle, Options, WriteBatch, DB};
use std::sync::Arc;

pub const BACKGROUND_FLUSHES: i32 = 2;
pub const BACKGROUND_COMPACTIONS: i32 = 2;
pub const WRITE_BUFFER_SIZE: usize = 4 * 64 * 1024 * 1024;

/// The central object responsible for handling all the connections.
#[cfg(not(target_os = "windows"))]
pub struct RocksDbDriver {
    key_value_db: DB,
}

#[cfg(not(target_os = "windows"))]
impl RocksDbDriver {
    pub fn open(db_path: &str, max_open_files: u64) -> KeyValueDB {
        let mut opts = Options::default();
        opts.set_write_buffer_size(WRITE_BUFFER_SIZE);
        opts.set_max_background_jobs(BACKGROUND_FLUSHES);

        let block_opts = BlockBasedOptions::default();
        opts.set_block_based_table_factory(&block_opts);

        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let block_opts = BlockBasedOptions::default();
        opts.set_block_based_table_factory(&block_opts);

        opts.set_max_open_files(max_open_files as i32);
        opts.set_use_fsync(false);
        opts.set_compaction_style(DBCompactionStyle::Level);

        Arc::new(Mutex::new(Box::new(RocksDbDriver {
            key_value_db: DB::open(&opts, db_path).unwrap(),
        })))
    }
}

#[cfg(not(target_os = "windows"))]
impl KeyValueDb for RocksDbDriver {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.key_value_db.get(key) {
            Ok(value) => return Ok(value),
            Err(err) => return Err(anyhow::anyhow!("db get error:{}", err.to_string())),
        }
    }

    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        if let Err(err) = self.key_value_db.put(key, value) {
            return Err(anyhow!("db put error:{}", err.to_string()));
        }
        Ok(())
    }

    fn delete(&mut self, key: Vec<u8>) -> Result<()> {
        if let Err(err) = self.key_value_db.delete(key) {
            return Err(anyhow!("db delete error:{}", err.to_string()));
        }
        Ok(())
    }

    fn write_batch(&mut self, mem_bench: MemWriteBatch) -> Result<()> {
        let mut bench = WriteBatch::default();
        for (key, value) in mem_bench.insertions {
            bench.put(key, value);
        }
        for key in mem_bench.deletions {
            bench.delete(key);
        }

        if let Err(err) = self.key_value_db.write(bench) {
            return Err(anyhow!("db write_batch error:{}", err.to_string()));
        }
        Ok(())
    }
}
