use crate::TrieHashDB;
use hash_db::Hasher;
use keccak_hasher::KeccakHasher;
use reference_trie::ExtensionLayout;
use std::collections::HashMap;
use storage_db::{KeyValueDB, MemWriteBatch, WriteBatchTrait};
use trie_db::{Trie, TrieDBBuilder, TrieDBMutBuilder, TrieMut};

pub type TrieHash = <KeccakHasher as Hasher>::Out;

pub struct TrieReader {
    root: TrieHash,
    trie_db: TrieHashDB,
}

impl TrieReader {
    pub fn new(db: KeyValueDB, root_hash: Option<TrieHash>) -> Self {
        let trie_db = TrieHashDB::new(db, None);
        let root = match root_hash {
            Some(value) => value,
            None => TrieHash::default(),
        };

        Self { root, trie_db }
    }

    pub fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let trie = TrieDBBuilder::<ExtensionLayout>::new(&self.trie_db, &self.root).build();
        match trie.get(key) {
            Ok(value) => return Ok(value),
            Err(err) => return Err(anyhow::anyhow!("trie get error:{}", err.to_string())),
        }
    }

    pub fn all(&self) -> anyhow::Result<HashMap<Vec<u8>, Vec<u8>>> {
        let trie = TrieDBBuilder::<ExtensionLayout>::new(&self.trie_db, &self.root).build();

        let iter_result = trie.iter();
        match iter_result {
            Ok(iter) => {
                let mut result = HashMap::default();
                for kv in iter {
                    match kv {
                        Ok(kv) => {
                            let (k, v) = kv;
                            result.insert(k, v);
                        }
                        Err(err) => {
                            return Err(anyhow::anyhow!("trie iter kv error:{}", err.to_string()));
                        }
                    }
                }
                return Ok(result);
            }
            Err(err) => return Err(anyhow::anyhow!("trie iter error:{}", err.to_string())),
        }
    }
}

pub struct TrieWriter;

impl TrieWriter {
    pub fn commit(
        db: KeyValueDB,
        root_hash: Option<TrieHash>,
        datas: &HashMap<Vec<u8>, Option<Vec<u8>>>,
        batch: &mut MemWriteBatch,
    ) -> anyhow::Result<TrieHash> {
        let mut trie_db = TrieHashDB::new(db, None);
        let mut root = match root_hash {
            Some(value) => value,
            None => TrieHash::default(),
        };
        let new_root = {
            let mut trie: trie_db::TrieDBMut<'_, ExtensionLayout> =
                TrieDBMutBuilder::<ExtensionLayout>::new(&mut trie_db, &mut root).build();

            for (k, v) in datas.iter() {
                match v {
                    Some(value) => {
                        if let Err(err) = trie.insert(k.as_ref(), value) {
                            return Err(anyhow::anyhow!("trie insert error:{}", err.to_string()));
                        }
                    }
                    None => {
                        if let Err(errr) = trie.remove(k.as_ref()) {
                            return Err(anyhow::anyhow!("trie remove error:{}", errr.to_string()));
                        }
                    }
                }
            }
            trie.root().clone()
        };

        for (key, value) in trie_db.cache {
            match value {
                Some(value) => batch.set(key, value),
                None => batch.delete(key),
            }
        }

        Ok(new_root)
    }
}
