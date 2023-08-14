use crate::account_frame::AccountFrame;
use crate::TrieHash;
use crate::TrieReader;

use log::*;
use parking_lot::RwLock;
use protos::common::{Validator, ValidatorSet};

use std::collections::HashMap;
use std::ops::Deref;
use std::{hash::Hash, sync::Arc};
use storage::STORAGE_INSTANCE_REF;
use utils::general::TRIE_KEY_MAX_LEN;

const VALIDATORS_KEY: &str = "validators";
// const FEES_KEY: &str = "configFees";

#[derive(Clone, Copy, PartialEq)]
pub enum StateMapActionType {
    ADD = 0,
    MODIFY = 1,
    DELETE = 2,
    READ = 3,
    MAX = 4,
}

pub enum StateMapQueryResult<V> {
    HasDeletedInCache,
    Exist(Arc<RwLock<V>>),
    NeedLoadFormDb,
}

pub struct MapValue<V> {
    pub action: StateMapActionType,
    pub data: Arc<RwLock<V>>,
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
    pub fn new(action: StateMapActionType, data: Arc<RwLock<V>>) -> MapValue<V> {
        MapValue { action, data }
    }

    pub fn new_del() -> MapValue<V> {
        MapValue {
            action: StateMapActionType::DELETE,
            data: Arc::new(RwLock::new(V::default())),
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
                let value_clone = item.data.read().clone();
                let pv = Arc::new(RwLock::new(value_clone));

                buff.write()
                    .insert(k.clone(), MapValue::new(item.action, pv.clone()));
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

    pub fn set(&self, k: &K, action: StateMapActionType, ptr: Arc<RwLock<V>>) {
        self.buff
            .write()
            .insert(k.clone(), MapValue::new(action, ptr));
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
            let value_clone = item.data.read().clone();
            let pv = Arc::new(RwLock::new(value_clone));

            self.set(k, item.action, pv.clone());
            return StateMapQueryResult::Exist(pv);
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
    pub settings: CacheMap<String, serde_json::Value>,
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
            settings: CacheMap::default(),
            root_hash: Default::default(),
        }))
    }
}

impl CacheState {
    pub fn new(root_hash: &TrieHash, validators: &ValidatorSet) -> CacheState {
        let settings: CacheMap<String, serde_json::Value> = CacheMap::default();

        let value = utils::proto2json::proto_to_json(validators);
        let pv = Arc::new(RwLock::new(value));
        settings.set(&VALIDATORS_KEY.to_string(), StateMapActionType::READ, pv);

        CacheState(Arc::new(InnerCacheState {
            accounts: CacheMap::default(),
            settings: settings,
            root_hash: root_hash.clone(),
        }))
    }

    pub fn get(&self, k: &String) -> Option<Arc<RwLock<AccountFrame>>> {
        match self.get_from_account_cache(k) {
            StateMapQueryResult::HasDeletedInCache => {
                return None;
            }
            StateMapQueryResult::NeedLoadFormDb => {
                let root_hash = self.get_root();

                match Self::get_account_from_db(k, &root_hash) {
                    Ok(value) => {
                        if let Some(account_frame) = value {
                            let ptr = Arc::new(RwLock::new(account_frame));
                            self.load(k, ptr.clone());
                            return Some(ptr);
                        } else {
                            return None;
                        }
                    }
                    Err(e) => {
                        error!("{}", e.to_string());
                        return None;
                    }
                }
            }
            StateMapQueryResult::Exist(value) => {
                return Some(value);
            }
        }
    }

    pub fn get_validators(&self) -> linked_hash_map::LinkedHashMap<String, u64> {
        let mut vs: linked_hash_map::LinkedHashMap<String, u64> =
            linked_hash_map::LinkedHashMap::default();
        match self.get_from_settings(&VALIDATORS_KEY.to_string()) {
            StateMapQueryResult::Exist(validator_set_json) => {
                if let Some(validators) = validator_set_json.read()["validators"].as_array() {
                    for i in validators.iter() {
                        let address = i["address"].as_str().unwrap().to_string();
                        let mut role = 0;
                        if !i["pledge_amount"].is_null() {
                            role = i["pledge_amount"].as_u64().unwrap();
                        }
                        vs.insert(address, role);
                    }
                }
            }
            _ => {}
        }
        vs
    }

    pub fn update_new_validators(&self, validators: &Vec<String>) {
        let mut set = ValidatorSet::new();
        for address in validators.iter() {
            let mut v = Validator::new();
            v.set_address(address.clone());
            set.mut_validators().push(v);
        }
        let value = utils::proto2json::proto_to_json(&set);
        let ptr = Arc::new(RwLock::new(value));
        self.set_to_settings(&VALIDATORS_KEY.to_string(), ptr);
    }

    pub fn get_voted_validators(&self, set: &mut ValidatorSet) -> bool {
        let arr_old = set
            .get_validators()
            .iter()
            .map(|x| x.get_address().to_string())
            .collect::<Vec<_>>();

        let mut arr_new: Vec<String> = Vec::new();
        let mut vs = ValidatorSet::new();
        match self.get_from_settings(&VALIDATORS_KEY.to_string()) {
            StateMapQueryResult::Exist(validator_set_json) => {
                if let Some(validators) = validator_set_json.read()["validators"].as_array() {
                    for i in validators.iter() {
                        let address = i["address"].as_str().unwrap().to_string();
                        arr_new.push(address.clone());

                        let mut va = Validator::new();
                        va.set_address(address);
                        va.set_pledge_amount(0);

                        vs.mut_validators().push(va);
                    }
                }
            }
            _ => {}
        }
        if arr_old != arr_new {
            set.clone_from(&vs);
            return true;
        }
        false
    }

    fn get_from_account_cache(&self, k: &String) -> StateMapQueryResult<AccountFrame> {
        self.0.accounts.get(k)
    }

    pub fn get_from_settings(&self, k: &String) -> StateMapQueryResult<serde_json::Value> {
        self.0.settings.get(k)
    }

    pub fn set_to_settings(&self, k: &String, v: Arc<RwLock<serde_json::Value>>) {
        self.0.settings.set(k, StateMapActionType::MODIFY, v);
    }

    pub fn get_root(&self) -> TrieHash {
        self.0.root_hash.clone()
    }

    fn load(&self, k: &String, v: Arc<RwLock<AccountFrame>>) -> bool {
        self.0.accounts.set(k, StateMapActionType::READ, v);
        true
    }

    pub fn add(&self, k: &String, v: Arc<RwLock<AccountFrame>>) -> bool {
        if k.len() >= TRIE_KEY_MAX_LEN {
            return false;
        }
        self.0.accounts.set(k, StateMapActionType::ADD, v);
        true
    }

    pub fn update(&self, k: &String, v: Arc<RwLock<AccountFrame>>) -> bool {
        if k.len() >= TRIE_KEY_MAX_LEN {
            return false;
        }
        self.0.accounts.set(k, StateMapActionType::MODIFY, v);
        true
    }

    pub fn commit_accounts(&self) {
        self.0.accounts.commit_to_store();
    }

    pub fn commit_settings(&self) {
        self.0.settings.commit_to_store();
    }

    pub fn commit(&self) {
        self.commit_accounts();
        self.commit_settings();
    }

    pub fn clear_account_buff(&self) {
        self.0.accounts.clear_buff();
    }

    pub fn clear_settings_buff(&self) {
        self.0.settings.clear_buff();
    }

    pub fn clear_cache(&self) {
        self.clear_account_buff();
        self.clear_settings_buff();
    }

    pub fn get_store(&self) -> Arc<RwLock<HashMap<String, MapValue<AccountFrame>>>> {
        self.0.accounts.get_store()
    }

    pub fn get_settings_store(&self) -> Arc<RwLock<HashMap<String, MapValue<serde_json::Value>>>> {
        self.0.settings.get_store()
    }

    pub fn get_buff(&self) -> Arc<RwLock<HashMap<String, MapValue<AccountFrame>>>> {
        self.0.accounts.get_buff()
    }

    pub fn get_settings_buff(&self) -> Arc<RwLock<HashMap<String, MapValue<serde_json::Value>>>> {
        self.0.settings.get_buff()
    }

    pub fn new_stack_state(&self, double_copy: bool) -> CacheState {
        let state = CacheState(Arc::new(InnerCacheState {
            accounts: CacheMap::new(self.get_buff(), double_copy),
            settings: CacheMap::new(self.get_settings_buff(), double_copy),
            root_hash: self.get_root(),
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
                return Err(anyhow::anyhow!(
                    "get account({}) failed ,reason({})",
                    key,
                    e
                ));
            }
        }
    }
}
