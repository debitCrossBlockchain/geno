use crate::account_frame::AccountFrame;
use crate::TrieHash;
use crate::TrieReader;
use crate::TRIE_KEY_MAX_LEN;

use log::*;
use parking_lot::RwLock;

use std::collections::HashMap;
use std::ops::Deref;
use std::{hash::Hash, sync::Arc};
use storage_db::STORAGE_INSTANCE_REF;

#[derive(Clone, Copy, PartialEq)]
pub enum StateMapActionType {
    HOTLOAD = 0,
    UPSERT = 1,
    DELETE = 2,
    MAX = 3,
}

pub enum StateMapQueryResult<V> {
    HasDeletedInCache,
    Exist(V),
    NeedLoadFormDb,
}

pub struct MapValue<V> {
    pub action: StateMapActionType,
    pub data: V,
}

impl<V> Clone for MapValue<V>
where
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            action: self.action,
            data: self.data.clone(),
        }
    }
}

impl<V> MapValue<V>
where
    V: Clone + Default,
{
    pub fn new(action: StateMapActionType, data: V) -> MapValue<V> {
        MapValue { action, data }
    }

    pub fn new_del() -> MapValue<V> {
        MapValue {
            action: StateMapActionType::DELETE,
            data: V::default(),
        }
    }
}

pub struct CacheMap<K, V> {
    pub buff: Arc<RwLock<HashMap<K, MapValue<V>>>>,
    pub store: Arc<RwLock<HashMap<K, MapValue<V>>>>,
}

impl<K, V> Default for CacheMap<K, V> {
    fn default() -> Self {
        Self {
            buff: Arc::new(RwLock::new(HashMap::default())),
            store: Arc::new(RwLock::new(HashMap::default())),
        }
    }
}

impl<K, V> Clone for CacheMap<K, V>
where
    K: Clone + Eq + PartialEq + Hash,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            buff: self.buff.clone(),
            store: self.store.clone(),
        }
    }
}

impl<K, V> CacheMap<K, V>
where
    K: Clone + Eq + PartialEq + Hash,
    V: Clone + Default,
{
    pub fn new(data: Arc<RwLock<HashMap<K, MapValue<V>>>>, double_copy: bool) -> Self {
        if double_copy {
            let buff = Arc::new(RwLock::new(HashMap::default()));
            for (k, item) in data.write().iter_mut() {
                let value_clone = item.data.clone();

                buff.write()
                    .insert(k.clone(), MapValue::new(item.action, value_clone));
            }
            Self {
                buff,
                store: data.clone(),
            }
        } else {
            Self {
                buff: Arc::new(RwLock::new(HashMap::default())),
                store: data.clone(),
            }
        }
    }

    pub fn set(&self, k: &K, action: StateMapActionType, value: V) {
        self.buff
            .write()
            .insert(k.clone(), MapValue::new(action, value));
    }

    pub fn get(&self, k: &K) -> StateMapQueryResult<V> {
        if let Some(item) = self.buff.read().get(k) {
            if item.action == StateMapActionType::DELETE {
                return StateMapQueryResult::HasDeletedInCache;
            }
            return StateMapQueryResult::Exist(item.data.clone());
        }

        if let Some(item) = self.store.write().get_mut(k) {
            if item.action == StateMapActionType::DELETE {
                return StateMapQueryResult::HasDeletedInCache;
            }
            let value_clone = item.data.clone();

            self.set(k, item.action, value_clone.clone());
            return StateMapQueryResult::Exist(value_clone);
        }

        StateMapQueryResult::NeedLoadFormDb
    }

    pub fn del(&self, k: &K) -> bool {
        self.buff.write().insert(k.clone(), MapValue::new_del());
        true
    }

    pub fn commit_to_store(&self) {
        for (k, v) in self.buff.read().iter() {
            self.store.write().insert(k.clone(), v.clone());
        }

        self.clear_buff();
    }

    pub fn clear_buff(&self) {
        self.buff.write().clear();
    }

    pub fn get_buff(&self) -> Arc<RwLock<HashMap<K, MapValue<V>>>> {
        self.buff.clone()
    }

    pub fn get_store(&self) -> Arc<RwLock<HashMap<K, MapValue<V>>>> {
        self.store.clone()
    }
}

pub struct InnerCacheState {
    pub accounts: CacheMap<String, AccountFrame>,
    pub root_hash: TrieHash,
}

#[derive(Clone)]
pub struct CacheState(Arc<InnerCacheState>);

impl Deref for CacheState {
    type Target = Arc<InnerCacheState>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for CacheState {
    fn default() -> Self {
        CacheState(Arc::new(InnerCacheState {
            accounts: CacheMap::default(),
            root_hash: Default::default(),
        }))
    }
}

impl CacheState {
    pub fn new(root_hash: TrieHash) -> CacheState {
        CacheState(Arc::new(InnerCacheState {
            accounts: CacheMap::default(),
            root_hash: root_hash,
        }))
    }

    pub fn get(&self, k: &String) -> anyhow::Result<Option<AccountFrame>> {
        match self.get_from_account_cache(k) {
            StateMapQueryResult::HasDeletedInCache => {
                return Ok(None);
            }
            StateMapQueryResult::NeedLoadFormDb => {
                let root_hash = self.root_hash();

                match Self::get_account_from_db(k, &root_hash) {
                    Ok(value) => {
                        if let Some(account_frame) = value {
                            self.load(k, account_frame.clone());
                            return Ok(Some(account_frame));
                        } else {
                            return Ok(None);
                        }
                    }
                    Err(e) => {
                        error!("{}", e.to_string());
                        return Err(e);
                    }
                }
            }
            StateMapQueryResult::Exist(value) => {
                return Ok(Some(value));
            }
        }
    }

    fn get_from_account_cache(&self, k: &String) -> StateMapQueryResult<AccountFrame> {
        self.0.accounts.get(k)
    }

    pub fn root_hash(&self) -> TrieHash {
        self.0.root_hash.clone()
    }

    fn load(&self, k: &String, v: AccountFrame) -> bool {
        self.0.accounts.set(k, StateMapActionType::HOTLOAD, v);
        true
    }

    pub fn upsert(&self, k: &String, v: AccountFrame) -> bool {
        if k.len() >= TRIE_KEY_MAX_LEN {
            return false;
        }
        self.0.accounts.set(k, StateMapActionType::UPSERT, v);
        true
    }

    pub fn delete(&self, k: &String) -> bool {
        if k.len() >= TRIE_KEY_MAX_LEN {
            return false;
        }
        self.0.accounts.del(k)
    }

    pub fn commit_accounts(&self) {
        self.0.accounts.commit_to_store();
    }

    pub fn commit(&self) {
        self.commit_accounts();
    }

    pub fn clear_account_buff(&self) {
        self.0.accounts.clear_buff();
    }

    pub fn clear_cache(&self) {
        self.clear_account_buff();
    }

    pub fn get_store(&self) -> Arc<RwLock<HashMap<String, MapValue<AccountFrame>>>> {
        self.0.accounts.get_store()
    }

    pub fn get_buff(&self) -> Arc<RwLock<HashMap<String, MapValue<AccountFrame>>>> {
        self.0.accounts.get_buff()
    }

    pub fn get_commit_changes(&self) -> HashMap<String, MapValue<AccountFrame>> {
        let all = self.0.accounts.get_store();
        let mut changes = HashMap::new();
        for (k, v) in all.read().iter() {
            if v.action == StateMapActionType::HOTLOAD || v.action == StateMapActionType::MAX {
                continue;
            }
            changes.insert(k.clone(), v.clone());
        }
        changes
    }

    pub fn new_stack_state(&self, double_copy: bool) -> CacheState {
        let state = CacheState(Arc::new(InnerCacheState {
            accounts: CacheMap::new(self.get_buff(), double_copy),
            root_hash: self.root_hash(),
        }));
        state
    }

    pub fn get_account_from_db(
        key: &String,
        root_hash: &TrieHash,
    ) -> anyhow::Result<Option<AccountFrame>> {
        let state_db = STORAGE_INSTANCE_REF.account_db();
        let reader = TrieReader::new(state_db, Some(root_hash.clone()));
        match reader.get(key.as_bytes()) {
            Ok(value) => {
                if let Some(data) = value {
                    match AccountFrame::deserialize(&data) {
                        Ok(account) => {
                            return Ok(Some(account));
                        }
                        Err(e) => return Err(anyhow::anyhow!(e)),
                    }
                } else {
                    return Ok(None);
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
}
