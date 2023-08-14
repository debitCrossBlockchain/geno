use hash_db::{AsHashDB, HashDB, Hasher as KeyHasher, Prefix};
use keccak_hasher::KeccakHasher;
use std::collections::HashMap;
use storage_db::KeyValueDB;
use trie_db::{DBValue, HashDBRef};

type Hasher = KeccakHasher;

pub struct TrieHashDB {
    pub db: KeyValueDB,
    pub cache: HashMap<Vec<u8>, Option<Vec<u8>>>,
}

impl Clone for TrieHashDB {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            cache: self.cache.clone(),
        }
    }
}

impl TrieHashDB {
    pub fn new(db: KeyValueDB, cache: Option<HashMap<Vec<u8>, Option<Vec<u8>>>>) -> TrieHashDB {
        if let Some(cache) = cache {
            TrieHashDB { db, cache }
        } else {
            TrieHashDB {
                db,
                cache: HashMap::default(),
            }
        }
    }

    pub fn reset(&mut self) {
        self.cache.clear();
    }
}

impl AsHashDB<Hasher, DBValue> for TrieHashDB {
    fn as_hash_db(&self) -> &dyn hash_db::HashDB<Hasher, DBValue> {
        &*self
    }

    fn as_hash_db_mut<'b>(&'b mut self) -> &'b mut (dyn HashDB<Hasher, DBValue> + 'b) {
        &mut *self
    }
}

impl HashDB<Hasher, DBValue> for TrieHashDB {
    fn get(&self, key: &[u8; 32], prefix: Prefix) -> Option<DBValue> {
        if key == &keccak_hasher::KeccakHasher::hash(&[0u8][..]) {
            return Some([0u8][..].into());
        }

        let key = memory_db::prefixed_key::<Hasher>(key, prefix);
        if let Some(value) = self.cache.get(&key) {
            return value.clone();
        }

        match self.db.lock().get(&key[..]) {
            Ok(value) => return value,
            Err(_) => return None,
        }
    }

    fn contains(&self, hash: &[u8; 32], prefix: Prefix) -> bool {
        HashDB::get(self, hash, prefix).is_some()
    }

    fn insert(&mut self, prefix: Prefix, value: &[u8]) -> [u8; 32] {
        let key = keccak_hasher::KeccakHasher::hash(value);
        self.emplace(key, prefix, value.to_vec());
        key
    }

    fn emplace(&mut self, key: [u8; 32], prefix: Prefix, value: DBValue) {
        let key = memory_db::prefixed_key::<Hasher>(&key, prefix);
        self.cache.insert(key, Some(value));
    }

    fn remove(&mut self, key: &[u8; 32], prefix: Prefix) {
        let key = memory_db::prefixed_key::<Hasher>(key, prefix);
        self.cache.insert(key, None);
    }
}

impl HashDBRef<Hasher, DBValue> for TrieHashDB {
    fn get(&self, key: &[u8; 32], prefix: Prefix) -> Option<DBValue> {
        HashDB::get(self, key, prefix)
    }

    fn contains(&self, key: &[u8; 32], prefix: Prefix) -> bool {
        HashDB::contains(self, key, prefix)
    }
}
