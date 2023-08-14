use anyhow::Result;
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub type KeyValueDB = Arc<Mutex<Box<dyn KeyValueDb + Send + 'static>>>;

pub trait KeyValueDb {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()>;
    fn delete(&mut self, key: Vec<u8>) -> Result<()>;
    fn write_batch(&mut self, mem_benchs: MemWriteBatch) -> Result<()>;
}

pub trait WriteBatchTrait {
    fn new() -> Self;
    fn set(&mut self, key: &[u8], value: &[u8]);
    fn delete(&mut self, key: &[u8]);
}

pub struct MemWriteBatch {
    pub insertions: HashMap<Vec<u8>, Vec<u8>>,
    pub deletions: HashSet<Vec<u8>>,
}

impl WriteBatchTrait for MemWriteBatch {
    fn new() -> Self {
        MemWriteBatch {
            insertions: HashMap::new(),
            deletions: HashSet::new(),
        }
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        let _ = self.deletions.remove(key);
        self.insertions.insert(key.to_vec(), value.to_vec());
    }

    fn delete(&mut self, key: &[u8]) {
        let _ = self.insertions.remove(key);
        self.deletions.insert(key.to_vec());
    }
}
