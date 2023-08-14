use crate::key_value_db::{KeyValueDB, KeyValueDb, MemWriteBatch};
#[cfg(target_os = "windows")]
use rusty_leveldb::{Options, WriteBatch, DB};

/// The central object responsible for handling all the connections.
#[cfg(target_os = "windows")]
pub struct LevelDbDriver {
    key_value_db: DB,
}

#[cfg(target_os = "windows")]
impl LevelDbDriver {
    pub fn open(db_path: &str, max_open_files: u64) -> KeyValueDB {
        let mut opt = Options::default();
        opt.reuse_logs = false;
        opt.reuse_manifest = false;
        opt.compression_type = rusty_leveldb::CompressionType::CompressionNone;
        opt.max_file_size = max_open_files as usize;
        Arc::new(Mutex::new(Box::new(LevelDbDriver {
            key_value_db: DB::open(db_path, opt).unwrap(),
        })))
    }
}

#[cfg(target_os = "windows")]
impl KeyValueDb for LevelDbDriver {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
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
