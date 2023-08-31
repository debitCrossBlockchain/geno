use hash_db::{AsHashDB, HashDB, Hasher as KeyHasher, Prefix};
use keccak_hasher::KeccakHasher;
use std::collections::HashMap;
use trie_db::{DBValue, HashDBRef};

type Hasher = KeccakHasher;

pub struct TrieMemDB<'a> {
    pub overlay: &'a mut HashMap<Vec<u8>, Option<Vec<u8>>>,
}

impl<'a> AsHashDB<Hasher, DBValue> for TrieMemDB<'a> {
    fn as_hash_db(&self) -> &dyn hash_db::HashDB<Hasher, DBValue> {
        &*self
    }

    fn as_hash_db_mut<'b>(&'b mut self) -> &'b mut (dyn HashDB<Hasher, DBValue> + 'b) {
        &mut *self
    }
}

impl<'a> HashDB<Hasher, DBValue> for TrieMemDB<'a> {
    fn get(&self, key: &[u8; 32], prefix: Prefix) -> Option<DBValue> {
        if key == &keccak_hasher::KeccakHasher::hash(&[0u8][..]) {
            return Some([0u8][..].into());
        }

        let key = memory_db::prefixed_key::<Hasher>(key, prefix);
        if let Some(value) = self.overlay.get(&key) {
            return value.clone();
        }
        None
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
        self.overlay.insert(key, Some(value));
    }

    fn remove(&mut self, key: &[u8; 32], prefix: Prefix) {
        let key = memory_db::prefixed_key::<Hasher>(key, prefix);
        self.overlay.insert(key, None);
    }
}

impl<'a> HashDBRef<Hasher, DBValue> for TrieMemDB<'a> {
    fn get(&self, key: &[u8; 32], prefix: Prefix) -> Option<DBValue> {
        HashDB::get(self, key, prefix)
    }
    fn contains(&self, key: &[u8; 32], prefix: Prefix) -> bool {
        HashDB::contains(self, key, prefix)
    }
}
