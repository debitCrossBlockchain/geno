use std::sync::Arc;

use crate::AccountFrame;
use parking_lot::{Mutex, RwLock};
// use std::collections::HashMap;
use lru_cache::LruCache;
pub struct MemoryStateDB {
    pool: LruCache<String, AccountFrame>,
}

impl Default for MemoryStateDB {
    fn default() -> Self {
        Self {
            pool: LruCache::new(10000),
        }
    }
}

impl MemoryStateDB {
    pub fn get(&mut self, key: &str) -> Option<AccountFrame> {
        match self.pool.get_mut(key) {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    pub fn get_nonce_balance(&mut self, key: &str) -> Option<(u64, u64)> {
        match self.pool.get_mut(key) {
            Some(v) => Some((v.get_nonce(), v.get_balance())),
            None => None,
        }
    }

    pub fn set(&mut self, set: std::collections::HashMap<String, AccountFrame>) {
        for (k, v) in set.iter() {
            self.pool.insert(k.clone(), v.clone());
        }
    }
}
