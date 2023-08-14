use bytes::Bytes;
use ledger_store::LedgerStorage;
use protobuf::Message;
use revm::{
    db::{CacheDB, DatabaseRef},
    primitives::{keccak256, AccountInfo, Bytecode, B160, B256, KECCAK_EMPTY, U256},
};
use state::{cache_state, AccountFrame, CacheState};
use state_store::StateStorage;
use std::convert::TryInto;
use std::str::FromStr;
use utils::general::{address_add_prefix, address_filter_prefix};
pub const ADDRESS_PREFIX: &str = "did:gdt:0x";

pub type VmState = CacheDB<State>;

#[derive(Clone)]
pub struct State {
    cache_state: CacheState,
}

impl State {
    pub fn new(cache_state: CacheState) -> State {
        State { cache_state }
    }

    fn get_account(
        &self,
        address: &String,
    ) -> std::result::Result<Option<AccountFrame>, StateError> {
        match self.cache_state.get(&address) {
            Ok(value) => {
                if let Some(account) = value {
                    return Ok(Some(account));
                } else {
                    return Ok(None);
                }
            }
            Err(e) => {
                return Err(StateError::CacheState(e.to_string()));
            }
        }
    }
}

pub enum StateError {
    CacheState(String),
    Database(String),
    Convert(String),
}

impl DatabaseRef for State {
    type Error = StateError;

    fn basic(&self, address: B160) -> std::result::Result<Option<AccountInfo>, Self::Error> {
        let key = address_add_prefix(ADDRESS_PREFIX, &address.to_string());
        let result = self.get_account(&key)?;
        if let Some(account) = result {
            let contract = account.get_contract();
            let b = if account.has_contract() && account.get_contract().get_code().len() > 0 {
                Bytes::from(contract.get_code().to_vec())
            } else {
                Bytes::default()
            };
            let byte_code = Bytecode::new_raw(b);
            let hash = byte_code.hash();

            let account_info = AccountInfo {
                balance: U256::from(account.balance()),
                nonce: account.nonce(),
                code_hash: hash,
                code: None,
            };
            return Ok(Some(account_info));
        } else {
            return Ok(None);
        }
    }

    fn code_by_hash(&self, code_hash: B256) -> std::result::Result<Bytecode, Self::Error> {
        // get account address from db
        let address = match StateStorage::load_codehash_address_map(code_hash.as_bytes()) {
            Ok(result) => {
                if let Some(value) = result {
                    value
                } else {
                    return Err(StateError::Database("code hash not exist".to_string()));
                }
            }
            Err(e) => return Err(StateError::Database(e.to_string())),
        };

        // get account from cache state
        let result = self.get_account(&address)?;
        if let Some(account) = result {
            let contract = account.get_contract();
            let b = if account.has_contract() && account.get_contract().get_code().len() > 0 {
                Bytes::from(contract.get_code().to_vec())
            } else {
                Bytes::default()
            };
            let byte_code = Bytecode::new_raw(b);

            return Ok(byte_code);
        } else {
            return Ok(Bytecode::new());
        }
    }

    fn storage(&self, address: B160, index: U256) -> std::result::Result<U256, Self::Error> {
        let key = address_add_prefix(ADDRESS_PREFIX, &address.to_string());

        let value = U256::default();

        match self.cache_state.get(&key) {
            Ok(value) => {
                if let Some(mut account) = value {
                    match account.get_contract_metadata(index.as_le_slice()) {
                        Ok(value) => match value {
                            Some(value) => {
                                if let Some(u_value) = U256::try_from_le_slice(&value) {
                                    return Ok(u_value);
                                } else {
                                    return Err(StateError::Convert(
                                        "value to u256 error".to_string(),
                                    ));
                                }
                            }
                            None => return Ok(U256::default()),
                        },
                        Err(e) => {
                            return Err(StateError::CacheState(e.to_string()));
                        }
                    }
                } else {
                    return Err(StateError::CacheState("account not found".to_string()));
                }
            }
            Err(e) => {
                return Err(StateError::CacheState(e.to_string()));
            }
        }
    }

    fn block_hash(&self, number: U256) -> std::result::Result<B256, Self::Error> {
        let seq = match number.try_into() {
            Ok(value) => value,
            Err(e) => return Err(StateError::Convert("u256 to u64 error".to_string())),
        };

        match LedgerStorage::load_ledger_header(seq) {
            Ok(result) => {
                if let Some(header) = result {
                    return Ok(B256::from_slice(header.get_hash()));
                } else {
                    return Ok(B256::default());
                }
            }
            Err(e) => return Err(StateError::Database(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::primitives::U256;

    #[test]
    fn u256_test() {
        let value = U256::from(10000000);
    }
}
