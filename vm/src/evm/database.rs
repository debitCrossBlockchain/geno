use crate::utils::{AddressConverter, StorageConverter};
use bytes::Bytes;
use ledger_store::LedgerStorage;
use revm::{
    db::{CacheDB, DatabaseRef},
    primitives::{AccountInfo, Bytecode, B160, B256, U256},
};
use state::{AccountFrame, CacheState};
use state_store::StateStorage;
use std::convert::TryInto;
use types::error::VmError;

pub type VmState = CacheDB<State>;

#[derive(Clone)]
pub struct State {
    cache_state: CacheState,
}

impl State {
    pub fn new(cache_state: CacheState) -> State {
        State { cache_state }
    }

    pub fn state(&self) -> CacheState {
        self.cache_state.clone()
    }

    fn get_account(&self, address: &String) -> std::result::Result<Option<AccountFrame>, VmError> {
        match self.cache_state.get(&address) {
            Ok(value) => {
                if let Some(account) = value {
                    return Ok(Some(account));
                } else {
                    return Ok(None);
                }
            }
            Err(e) => {
                return Err(VmError::StateError {
                    error: e.to_string(),
                });
            }
        }
    }
}

impl DatabaseRef for State {
    type Error = VmError;

    fn basic(&self, address: B160) -> std::result::Result<Option<AccountInfo>, Self::Error> {
        let key = AddressConverter::from_evm_address(address);
        let result = self.get_account(&key)?;
        if let Some(account) = result {
            let contract = account.contract();
            let b = if account.has_contract() {
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
                    return Err(VmError::DatabaseError {
                        error: "code hash not exist for code_by_hash".to_string(),
                    });
                }
            }
            Err(e) => {
                return Err(VmError::DatabaseError {
                    error: format!("code_by_hash {}", e.to_string()),
                })
            }
        };

        // get account from cache state
        let result = self.get_account(&address)?;
        if let Some(account) = result {
            let contract = account.contract();
            let b = if account.has_contract() {
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
        let key = AddressConverter::from_evm_address(address);

        let value = U256::default();

        let result = self.get_account(&key)?;

        if let Some(mut account) = result {
            match account.get_contract_metadata(&StorageConverter::from_evm_storage(index)) {
                Ok(value) => match value {
                    Some(value) => {
                        return StorageConverter::to_evm_storage(&value);
                    }
                    None => return Ok(U256::default()),
                },
                Err(e) => {
                    return Err(VmError::StorageError {
                        error: e.to_string(),
                    });
                }
            }
        } else {
            return Err(VmError::StateError {
                error: "account not found for storage".to_string(),
            });
        }
    }

    fn block_hash(&self, number: U256) -> std::result::Result<B256, Self::Error> {
        let seq = match number.try_into() {
            Ok(value) => value,
            Err(e) => {
                return Err(VmError::ValueConvertError {
                    error: format!("u256 to u64 error"),
                })
            }
        };

        match LedgerStorage::load_ledger_header_by_seq(seq) {
            Ok(result) => {
                if let Some(header) = result {
                    return Ok(B256::from_slice(header.get_hash()));
                } else {
                    return Ok(B256::default());
                }
            }
            Err(e) => {
                return Err(VmError::StorageError {
                    error: format!("block_hash {}", e.to_string()),
                })
            }
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
