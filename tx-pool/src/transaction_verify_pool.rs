
 use once_cell::sync::Lazy;
 use parking_lot::RwLock;
 use std::collections::HashSet;

 lazy_static! {
    pub static ref TxVerifyPoolRef: RwLock<TxVerifyPool> = RwLock::new(TxVerifyPool::default());
}

 pub struct TxVerifyPool {
     pool: HashSet<Vec<u8>>,
 }
 
 impl Default for TxVerifyPool {
     fn default() -> Self {
         Self {
             pool: HashSet::default(),
         }
     }
 }
 
 impl TxVerifyPool {
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
 
 pub fn tx_verify_pool_exist(hash: &[u8]) -> bool {
     TxVerifyPoolRef.read().contains(hash)
 }
 pub fn tx_verify_pool_set(hash: &[u8]) {
     TxVerifyPoolRef.write().insert(hash);
 }
 
 pub fn tx_verify_pool_del(hash: &[u8]) {
     TxVerifyPoolRef.write().remove(hash);
 }
 
 