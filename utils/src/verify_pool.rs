use std::collections::HashSet;

use crate::POOL_VERIFY_REF;

pub struct PoolVerify {
    pool: HashSet<Vec<u8>>,
}

impl Default for PoolVerify {
    fn default() -> Self {
        Self {
            pool: HashSet::default(),
        }
    }
}

impl PoolVerify {
    pub fn contains(&self, hash: &[u8]) -> bool {
        self.pool.contains(hash)
    }

    pub fn insert(&mut self, hash: &[u8]) {
        self.pool.insert(hash.to_vec());
    }

    pub fn remove(&mut self, hash: &[u8]) {
        self.pool.remove(hash);
    }
}

pub fn verify_pool_exist(hash: &[u8]) -> bool {
    POOL_VERIFY_REF.read().contains(hash)
}
pub fn verify_pool_set(hash: &[u8]) {
    POOL_VERIFY_REF.write().insert(hash);
}

pub fn verify_pool_del(hash: &[u8]) {
    POOL_VERIFY_REF.write().remove(hash);
}
