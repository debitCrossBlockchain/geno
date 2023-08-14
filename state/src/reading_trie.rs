use crate::{AccountFrame, MemoryStateDB, ReadingTrieRef, TrieHashDB};
use hash_db::Hasher;
use keccak_hasher::KeccakHasher;
use log::*;
use reference_trie::{RefTrieDB, Trie};
use std::collections::HashMap;
use storage::StorageInstanceRef;

pub struct ReadingTrie {
    pub root_hash: <KeccakHasher as Hasher>::Out,
    pub triedb: TrieHashDB,
    pub state_cache: MemoryStateDB,
}

impl Default for ReadingTrie {
    fn default() -> Self {
        let storage = StorageInstanceRef.clone();
        ReadingTrie {
            root_hash: <KeccakHasher as Hasher>::Out::default(),
            triedb: TrieHashDB::new(storage.account_db(), None),
            state_cache: MemoryStateDB::default(),
        }
    }
}

impl ReadingTrie {
    pub fn is_change(&self, hash: &<KeccakHasher as Hasher>::Out) -> bool {
        self.root_hash != *hash
    }

    pub fn reset_hash(&mut self, hash: &<KeccakHasher as Hasher>::Out) {
        if self.root_hash != *hash {
            self.triedb.reset();
            self.root_hash.clone_from(hash);
        }
    }

    pub fn reset_account_cache(&mut self, set: HashMap<String, AccountFrame>) {
        self.state_cache.set(set)
    }

    pub fn get_account(
        &mut self,
        key: &str,
        hash: &<KeccakHasher as Hasher>::Out,
    ) -> std::option::Option<AccountFrame> {
        if let Some(v) = self.state_cache.get(key) {
            return Some(v);
        }

        if self.root_hash != *hash {
            self.triedb.reset();
            self.root_hash.clone_from(hash);
        }

        let mut account = AccountFrame::default();
        let result = RefTrieDB::new(&self.triedb, &self.root_hash);
        match result {
            Ok(trie) => match trie.get(key.as_bytes()) {
                Ok(value) => {
                    if let Some(item) = value {
                        if account.deserialize(&item) {
                            return Some(account);
                        } else {
                            error!("reading_trie get account frame error,deserialize error");
                            return None;
                        }
                    } else {
                        info!("reading_trie get account frame error,trie get account not exist");
                        return None;
                    }
                }
                Err(e) => {
                    error!("reading_trie get account frame error,trie get error({})", e);
                    return None;
                }
            },
            Err(e) => {
                error!("reading_trie get account frame error,create trie({})", e);
                return None;
            }
        };
    }

    pub fn get_account_nonce_banace(
        &mut self,
        key: &str,
        hash: &<KeccakHasher as Hasher>::Out,
    ) -> std::option::Option<(u64, u64)> {
        if let Some(v) = self.state_cache.get_nonce_balance(key) {
            return Some(v);
        }

        if self.root_hash != *hash {
            self.triedb.reset();
            self.root_hash.clone_from(hash);
        }

        let mut account = AccountFrame::default();
        let result = RefTrieDB::new(&self.triedb, &self.root_hash);
        match result {
            Ok(trie) => match trie.get(key.as_bytes()) {
                Ok(value) => {
                    if let Some(item) = value {
                        if account.deserialize(&item) {
                            return Some((account.get_nonce(), account.get_balance()));
                        } else {
                            error!("reading_trie get account frame error,deserialize error");
                            return None;
                        }
                    } else {
                        info!("reading_trie get account frame error,trie get account not exist");
                        return None;
                    }
                }
                Err(e) => {
                    error!("reading_trie get account frame error,trie get error({})", e);
                    return None;
                }
            },
            Err(e) => {
                error!("reading_trie get account frame error,create trie({})", e);
                return None;
            }
        };
    }
}

pub fn reading_trie_get(
    key: &str,
    hash: &<KeccakHasher as Hasher>::Out,
) -> std::option::Option<AccountFrame> {
    // let is_change = { ReadingTrieRef.read().is_change(hash) };
    // if is_change {
    //     ReadingTrieRef.write().reset_hash(hash);
    // }
    ReadingTrieRef.write().get_account(key, hash)
}

pub fn reading_trie_get_nonce_banace(
    key: &str,
    hash: &<KeccakHasher as Hasher>::Out,
) -> std::option::Option<(u64, u64)> {
    // let is_change = { ReadingTrieRef.read().is_change(hash) };
    // if is_change {
    //     ReadingTrieRef.write().reset_hash(hash);
    // }

    ReadingTrieRef.write().get_account_nonce_banace(key, hash)
}

pub fn reading_trie_update_account_cache(set: HashMap<String, AccountFrame>) {
    ReadingTrieRef.write().reset_account_cache(set);
}
