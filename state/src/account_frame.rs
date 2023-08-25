use crate::{cache_state::StateMapActionType, TrieHash, TrieReader, TrieWriter};
use log::*;
use protobuf::Message;
use protos::{
    common::KeyPair,
    ledger::{Account, Contract},
};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use storage_db::{MemWriteBatch, STORAGE_INSTANCE_REF};
use utils::{general::*, parse::ProtocolParser};

pub const CONTRACT_META_PREFIX: &str = "contract_meta";
pub struct DataCache<T>
where
    T: Clone,
{
    pub action: StateMapActionType,
    pub data: Option<T>,
}

impl<T> Clone for DataCache<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            action: self.action,
            data: self.data.clone(),
        }
    }
}

pub struct AccountFrame {
    pub account: protos::ledger::Account,
    pub metadata: HashMap<Vec<u8>, DataCache<KeyPair>>,
}

impl Default for AccountFrame {
    fn default() -> Self {
        Self {
            account: Default::default(),
            metadata: Default::default(),
        }
    }
}

impl Clone for AccountFrame {
    fn clone(&self) -> AccountFrame {
        let mut accout_frame = AccountFrame::default();
        accout_frame.account.clone_from(&self.account);

        for iter in self.metadata.iter() {
            accout_frame.metadata.insert(iter.0.clone(), iter.1.clone());
        }
        accout_frame
    }
}

impl TryFrom<Account> for AccountFrame {
    type Error = anyhow::Error;
    fn try_from(account: Account) -> anyhow::Result<Self> {
        let _ = u128::from_str_radix(account.get_balance(), 10)?;
        Ok(AccountFrame {
            account,
            metadata: Default::default(),
        })
    }
}

impl AccountFrame {
    pub fn new(address: String, balance: u128) -> AccountFrame {
        let mut account = Account::new();
        account.set_address(address);
        account.set_balance(balance.to_string());

        AccountFrame {
            account,
            metadata: Default::default(),
        }
    }

    pub fn set_contract(&mut self, contract: &Contract) {
        self.account.set_contract(contract.clone());
    }

    pub fn contract(&self) -> Contract {
        self.account.get_contract().clone()
    }

    pub fn has_contract(&self) -> bool {
        if self.account.has_contract() {
            if !self.account.get_contract().get_name().is_empty()
                && self.account.get_contract().get_code().len() != 0
            {
                return true;
            }
        }
        false
    }

    pub fn contract_code_hash(&self) -> Vec<u8> {
        Vec::from(Keccak256::digest(self.account.get_contract().get_code()).as_slice())
    }

    pub fn set_document(&mut self, document: String) {
        self.account.set_document(document);
    }

    pub fn document(&self) -> &str {
        self.account.get_document()
    }

    pub fn address(&self) -> String {
        self.account.get_address().clone().to_string()
    }

    pub fn account(&self) -> Account {
        self.account.clone()
    }

    pub fn balance(&self) -> u128 {
        u128::from_str_radix(self.account.get_balance(), 10).unwrap_or(0)
    }

    pub fn set_balance(&mut self, amount: u128) {
        self.account.set_balance(amount.to_string());
    }

    pub fn add_balance(&mut self, amount: u128) -> Option<u128> {
        let balance = self.balance();
        if let Some(account_new_balance) = balance.checked_add(amount) {
            self.set_balance(account_new_balance);
            return Some(account_new_balance);
        }
        None
    }

    pub fn sub_balance(&mut self, amount: u128) -> Option<u128> {
        let balance = self.balance();
        if let Some(account_new_balance) = balance.checked_sub(amount) {
            self.set_balance(account_new_balance);
            return Some(account_new_balance);
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.balance() == 0
            && self.account.get_nonce() == 0
            && self.account.get_contract().get_code().is_empty()
            && self.metadata.is_empty()
    }

    pub fn get_metadata(&mut self, outer_key: &[u8]) -> anyhow::Result<Option<KeyPair>> {
        match self.metadata.get(outer_key) {
            None => {
                let mut root_hash: TrieHash = Default::default();
                let meta_hash = self.account.get_metadata_hash();
                if meta_hash.len() == 0 {
                    return Ok(None);
                }

                root_hash.clone_from_slice(&meta_hash[0..32]);

                let state_db = STORAGE_INSTANCE_REF.account_db();
                let reader = TrieReader::new(state_db, Some(root_hash.clone()));
                match reader.get(outer_key) {
                    Ok(value) => {
                        if let Some(data) = value {
                            match ProtocolParser::deserialize::<KeyPair>(&data) {
                                Ok(keypair) => {
                                    // insert canche
                                    let dc: DataCache<KeyPair> = DataCache {
                                        action: StateMapActionType::HOTLOAD,
                                        data: Some(keypair.clone()),
                                    };
                                    self.metadata.insert(outer_key.to_vec(), dc);

                                    return Ok(Some(keypair));
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
            Some(dc) => match dc.action {
                StateMapActionType::UPSERT | StateMapActionType::HOTLOAD => {
                    return Ok(dc.data.clone())
                }
                StateMapActionType::DELETE => return Ok(None),
                StateMapActionType::MAX => return Err(anyhow::anyhow!("StateMapActionType error")),
            },
        }
    }

    pub fn upsert_metadata(&mut self, outer_key: Vec<u8>, kp: KeyPair) -> bool {
        if TRIE_KEY_MAX_LEN <= outer_key.len() {
            error!(
                "Key({:?}) length({}) overflow,max allow len({})",
                outer_key,
                outer_key.len(),
                TRIE_KEY_MAX_LEN
            );
            return false;
        }
        let dc: DataCache<KeyPair> = DataCache {
            action: StateMapActionType::UPSERT,
            data: Some(kp),
        };

        self.metadata.insert(outer_key, dc);
        true
    }

    pub fn delete_metadata(&mut self, outer_key: Vec<u8>) -> bool {
        let dc: DataCache<KeyPair> = DataCache {
            action: StateMapActionType::DELETE,
            data: None,
        };

        self.metadata.insert(outer_key, dc);
        return true;
    }

    pub fn clear_metadata(&mut self) -> anyhow::Result<()> {
        let all = self.get_all_metadata()?;
        for (key, _) in all.iter() {
            self.delete_metadata(key.clone());
        }
        Ok(())
    }

    pub fn get_all_metadata(&mut self) -> anyhow::Result<HashMap<Vec<u8>, KeyPair>> {
        let mut keypairs = HashMap::default();
        let mut root_hash: TrieHash = Default::default();
        let meta_hash = self.account.get_metadata_hash();
        if meta_hash.len() == 0 {
            return Ok(keypairs);
        }

        root_hash.clone_from_slice(&meta_hash[0..32]);
        let state_db = STORAGE_INSTANCE_REF.account_db();
        let reader = TrieReader::new(state_db, Some(root_hash.clone()));
        match reader.all() {
            Ok(value) => {
                for (k, data) in value {
                    match ProtocolParser::deserialize::<KeyPair>(&data) {
                        Ok(keypair) => {
                            keypairs.insert(k, keypair);
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!(e));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("get all metadata failed ,reason({})", e));
            }
        }

        Ok(keypairs)
    }

    pub fn nonce_increase(&mut self) -> u64 {
        let new_nonce: u64 = self.account.get_nonce() + 1;
        self.account.set_nonce(new_nonce);
        new_nonce
    }

    pub fn nonce(&self) -> u64 {
        self.account.get_nonce()
    }

    pub fn set_nonce(&mut self, new_nonce: u64) {
        self.account.set_nonce(new_nonce);
    }

    pub fn metadatas_hash(&self) -> Vec<u8> {
        let hash = self.account.get_metadata_hash().clone();
        hash.to_vec()
    }

    pub fn set_metadatas_hash(&mut self, root: Vec<u8>) {
        self.account.set_metadata_hash(root);
    }

    pub fn commit_metadata_trie(&mut self, batch: &mut MemWriteBatch) -> anyhow::Result<()> {
        // no chang nothing to do
        if self.metadata.is_empty() {
            return Ok(());
        }

        let mut datas: HashMap<Vec<u8>, Option<Vec<u8>>> = HashMap::default();
        let map = std::mem::replace(&mut self.metadata, Default::default());
        for (k, v) in map {
            let action = v.action;

            match action {
                StateMapActionType::UPSERT => {
                    if let Some(kp_bytes) = v.data {
                        datas.insert(k, Some(kp_bytes.write_to_bytes().unwrap()));
                    }
                }
                StateMapActionType::DELETE => {
                    datas.insert(k, None);
                }
                StateMapActionType::HOTLOAD => {}
                StateMapActionType::MAX => {}
            }
        }

        let state_db = STORAGE_INSTANCE_REF.account_db();
        let meta_hash = self.account.get_metadata_hash();
        let result = if meta_hash.len() == 0 {
            TrieWriter::commit(state_db, None, &mut datas, batch)
        } else {
            let mut root_hash: TrieHash = Default::default();
            root_hash.clone_from_slice(&meta_hash[0..32]);
            TrieWriter::commit(state_db, Some(root_hash), &mut datas, batch)
        };

        match result {
            Ok(new_root) => {
                self.set_metadatas_hash(new_root.to_vec());
            }
            Err(e) => {
                return Err(e);
            }
        }

        Ok(())
    }

    pub fn upsert_contract_metadata(&mut self, inner_key: &[u8], value_bytes: &[u8]) -> bool {
        let outer_key =
            compose_metadata_key(CONTRACT_META_PREFIX, self.account.get_address(), inner_key);
        let mut kp = KeyPair::default();
        kp.set_key(inner_key.to_vec());
        kp.set_value(value_bytes.to_vec());
        self.upsert_metadata(outer_key, kp)
    }

    pub fn delete_contract_metadata(&mut self, inner_key: &[u8]) -> bool {
        let outer_key =
            compose_metadata_key(CONTRACT_META_PREFIX, self.account.get_address(), inner_key);

        self.delete_metadata(outer_key)
    }

    pub fn get_contract_metadata(&mut self, inner_key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let outer_key =
            compose_metadata_key(CONTRACT_META_PREFIX, self.account.get_address(), inner_key);
        match self.get_metadata(&outer_key) {
            Ok(value) => {
                if let Some(kp) = value {
                    return Ok(Some(kp.get_value().to_vec()));
                } else {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        self.account.write_to_bytes().unwrap().clone()
    }

    pub fn deserialize(data: &[u8]) -> anyhow::Result<AccountFrame> {
        match ProtocolParser::deserialize::<Account>(data) {
            Ok(account) => return AccountFrame::try_from(account),
            Err(err) => return Err(err),
        }
    }
}
