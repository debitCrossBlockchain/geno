use crate::key_value_db::{KeyValueDB, KeyValueDb, MemWriteBatch};
use anyhow::Result;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

pub struct MemoryDB {
    // key_value_db: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    key_value_db: HashMap<Vec<u8>, Vec<u8>>,
}

impl MemoryDB {
    pub fn open() -> KeyValueDB {
        Arc::new(Mutex::new(Box::new(MemoryDB {
            key_value_db: HashMap::new(),
        })))
    }
}

impl Clone for MemoryDB {
    fn clone(&self) -> Self {
        Self {
            key_value_db: self.key_value_db.clone(),
        }
    }
}

impl KeyValueDb for MemoryDB {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Some(value) = self.key_value_db.get(key) {
            return Ok(Some(value.clone()));
        }
        Ok(None)
    }

    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.key_value_db.insert(key, value);
        Ok(())
    }

    fn delete(&mut self, key: Vec<u8>) -> Result<()> {
        self.key_value_db.remove(&key);
        Ok(())
    }
    fn write_batch(&mut self, mem_benchs: MemWriteBatch) -> Result<()> {
        for (key, value) in mem_benchs.insertions {
            self.key_value_db.insert(key, value);
        }
        for key in mem_benchs.deletions {
            self.key_value_db.remove(&key);
        }
        Ok(())
    }
}
