use protos::common::{ContractEvent, ContractResult, TransactionResult};
use revm::primitives::{
    hash_map::{self, Entry},
    hex::ToHex,
    AccountInfo, Address, Bytecode, ExecutionResult, Log, ResultAndState, TransactTo, TxEnv, B160,
    B256, U256,
};
use state::{AccountFrame, CacheState};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};
use tracing::info;
use types::error::VmError;
mod account;
pub use account::{AccountChanges, PostAccount};

mod storage;
pub use storage::{Storage, StorageChanges, StorageChangeset, StorageWipe};

use crate::utils::{u256_2_u128, AddressConverter, StorageConverter};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Receipt {
    pub index: usize,
    pub success: bool,
    pub logs: Vec<Log>,
    pub gas_used: u64,
    pub contract_address: Option<B160>,
    pub output: Option<bytes::Bytes>,
    pub description: Option<String>,
}

impl Receipt {
    // lack error code,message,block_hash,block_height to set
    pub fn convert_to_geno_txresult(&self) -> TransactionResult {
        let mut tx_result = TransactionResult::default();

        let contract_result = self.to_contract_result();

        tx_result.set_err_code(contract_result.get_err_code());
        tx_result.set_contract_result(contract_result);
        tx_result.set_gas_used(self.gas_used);
        tx_result.set_index(self.index as u32);

        tx_result
    }

    pub fn to_contract_result(&self) -> ContractResult {
        let mut contract_result = ContractResult::default();

        let mut events = Vec::new();
        for log in self.logs.iter() {
            let mut contract_event = ContractEvent::default();
            contract_event.set_address(AddressConverter::from_evm_address(log.address));
            let topics = log
                .topics
                .iter()
                .map(|topic| format!("{topic:?}"))
                .collect::<Vec<_>>();
            contract_event.set_topic(topics.into());
            contract_event.set_data(vec![hex::encode(log.data.as_ref())].into());
            events.push(contract_event);
        }

        contract_result.set_contract_event(events.into());
        contract_result.set_err_code(if self.success == true { 0 } else { -1 });
        if let Some(description) = &self.description {
            contract_result.set_message(description.clone());
        }
        if let Some(address) = &self.contract_address {
            contract_result.set_message(AddressConverter::from_evm_address(address.clone()));
        }
        if let Some(out) = &self.output {
            contract_result.set_result(out.to_vec());
        }

        contract_result
    }

    pub fn from_contract_result(contract_result: &ContractResult) -> anyhow::Result<Receipt> {
        let mut logs = Vec::new();
        for event in contract_result.get_contract_event().iter() {
            let address = AddressConverter::to_evm_address(event.get_address())?;
            let mut topics = Vec::new();
            let mut bytes = bytes::BytesMut::default();

            for t in event.get_topic().iter() {
                let topic = B256::from_str(t)?;
                topics.push(topic);
            }
            for data in event.get_data().iter() {
                let value = hex::decode(data)?;
                bytes.clone_from_slice(&value);
                break;
            }
            let log = Log {
                address,
                topics,
                data: bytes.freeze(),
            };
            logs.push(log);
        }

        let description = if contract_result.get_message().len() > 0 {
            Some(contract_result.get_message().to_string())
        } else {
            None
        };

        let output = if contract_result.get_result().len() > 0 {
            Some(bytes::Bytes::copy_from_slice(contract_result.get_result()))
        } else {
            None
        };

        let receipt = Receipt {
            index: 0,
            success: contract_result.get_err_code() == 0,
            logs,
            gas_used: 0,
            contract_address: None,
            output,
            description,
        };
        Ok(receipt)
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct PostState {
    /// The state of all modified accounts after execution.
    ///
    /// If the value contained is `None`, then the account should be deleted.
    accounts: BTreeMap<Address, Option<PostAccount>>,
    /// The state of all modified storage after execution
    ///
    /// If the contained [Storage] is marked as wiped, then all storage values should be cleared
    /// from the database.
    storage: BTreeMap<Address, Storage>,
    /// The state of accounts before they were changed in the given block.
    ///
    /// If the value is `None`, then the account is new, otherwise it is a change.
    account_changes: AccountChanges,
    /// The state of account storage before it was changed in the given block.
    ///
    /// This map only contains old values for storage slots.
    storage_changes: StorageChanges,
    /// New code created during the execution
    bytecode: BTreeMap<B256, (B160, Bytecode)>,
    /// The receipt(s) of the executed transaction(s).
    receipts: BTreeMap<u64, Vec<Receipt>>,
}

impl PostState {
    /// Create an empty [PostState].
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark an account as destroyed.
    pub fn destroy_account(&mut self, block_number: u64, address: Address, account: PostAccount) {
        // accounts插入None 将被删除
        self.accounts.insert(address, None);
        // 是新账户插入None,其他是被更改过的,把旧的账户信息保留
        self.account_changes
            .insert(block_number, address, Some(account), None);

        let storage = self.storage.entry(address).or_default();
        storage.times_wiped += 1;
        let wipe = if storage.times_wiped == 1 {
            StorageWipe::Primary
        } else {
            StorageWipe::Secondary
        };

        let wiped_storage = std::mem::take(&mut storage.storage);
        // storage_changes 保存所有旧的元数据
        self.storage_changes.insert_for_block_and_address(
            block_number,
            address,
            wipe,
            wiped_storage.into_iter(),
        );
    }

    /// Add a newly created account to the post-state.
    pub fn create_account(&mut self, block_number: u64, address: Address, account: PostAccount) {
        self.accounts.insert(address, Some(account));
        self.account_changes
            .insert(block_number, address, None, Some(account));
    }

    /// Add a changed account to the post-state.
    ///
    /// If the account also has changed storage values, [PostState::change_storage] should also be
    /// called.
    pub fn change_account(
        &mut self,
        block_number: u64,
        address: Address,
        old: PostAccount,
        new: PostAccount,
    ) {
        self.accounts.insert(address, Some(new));
        self.account_changes
            .insert(block_number, address, Some(old), Some(new));
    }

    pub fn add_bytecode(&mut self, code_hash: B256, address: B160, bytecode: Bytecode) {
        // Assumption: `insert` will override the value if present, but since the code hash for a
        // given bytecode will always be the same, we are overriding with the same value.
        //
        // In other words: if this entry already exists, replacing the bytecode will replace with
        // the same value, which is wasteful.
        self.bytecode
            .entry(code_hash)
            .or_insert((address, bytecode));
    }

    /// Add changed storage values to the post-state.
    pub fn change_storage(
        &mut self,
        block_number: u64,
        address: Address,
        changeset: StorageChangeset,
    ) {
        self.storage
            .entry(address)
            .or_default()
            .storage
            .extend(changeset.iter().map(|(slot, (_, new))| (*slot, *new)));
        self.storage_changes.insert_for_block_and_address(
            block_number,
            address,
            StorageWipe::None,
            changeset.into_iter().map(|(slot, (old, _))| (slot, old)),
        );
    }

    /// Add a transaction receipt to the post-state.
    ///
    /// Transactions should always include their receipts in the post-state.
    pub fn add_receipt(&mut self, block: u64, receipt: Receipt) {
        self.receipts.entry(block).or_default().push(receipt);
    }

    pub fn convert_to_geno_state(
        &mut self,
        block_number: u64,
        state: CacheState,
    ) -> std::result::Result<(), VmError> {
        // process accounts
        tracing::trace!(target: "post_state", "Process accounts in block {}", block_number);
        for (address, paccount) in self.accounts.iter() {
            let geno_address = AddressConverter::from_evm_address(address.clone());

            if let Some(account) = paccount {
                tracing::info!(target: "post_state", "Updating state account {}, {:?}",geno_address, account);
                if let Some(mut geno_account) = Self::get_geno_account(&state, &geno_address)? {
                    // update accountp
                    Self::update_geno_account(&mut geno_account, &account);
                    state.upsert(&geno_address, geno_account);
                } else {
                    // create account
                    if account.nonce > 1 {
                        tracing::error!(target: "post_state", "Create account but nonce != 1 {}",geno_address);
                        panic!("post_state create account but nonce != 1 {}", geno_address);
                    }
                    let geno_account = Self::create_geno_account(geno_address.clone(), &account);
                    state.upsert(&geno_address, geno_account);
                }
            } else if Self::get_geno_account(&state, &geno_address)?.is_some() {
                tracing::trace!(target: "post_state","Deleting state account {}",geno_address);
                state.delete(&geno_address);
            }
        }

        // process storages
        tracing::trace!(target: "post_state", "Process storages in block {}", block_number);
        for (address, storage) in self.storage.iter() {
            let geno_address = AddressConverter::from_evm_address(address.clone());
            let mut geno_account = match Self::get_geno_account(&state, &geno_address)? {
                Some(geno_account) => geno_account,
                None => {
                    tracing::error!("post_state Process storages failed,get geno account error");
                    panic!("post_state Process storages failed,get geno account error");
                }
            };

            if storage.wiped() {
                tracing::trace!(target: "post_state", "Wiping storage from state {} in block {}", geno_address,block_number);
                if let Err(e) = geno_account.clear_metadata() {
                    tracing::error!("post_state Process storages failed,clear metadata error");
                    panic!("post_state Process storages failed,clear metadata error");
                }
            }

            for (key, value) in storage.storage.iter() {
                tracing::trace!(target: "post_state", "Updating state storage {} {:?} in block {}", geno_address,StorageConverter::from_evm_storage(key.clone()),block_number);

                if *value != U256::ZERO {
                    if !geno_account.upsert_contract_metadata(
                        &StorageConverter::from_evm_storage(key.clone()),
                        &StorageConverter::from_evm_storage(value.clone()),
                    ) {
                        tracing::error!("post_state Process storages failed,update storage error");
                        panic!("post_state Process storages failed,update storage error");
                    }
                }
            }
            state.upsert(&geno_address, geno_account);
        }

        // process contracts code
        tracing::trace!(target: "post_state", "Process contracts code in block {}", block_number);
        for (hash, (address, bytecode)) in self.bytecode.iter() {
            tracing::trace!(target: "post_state", "Process contract code hash {} in block {}",hash, block_number);
            let geno_address = AddressConverter::from_evm_address(address.clone());
            match Self::get_geno_account(&state, &geno_address)? {
                Some(mut geno_account) => {
                    let mut contract = geno_account.contract();
                    contract.set_code(bytecode.bytecode.to_vec());
                    geno_account.set_contract(&contract);
                    state.upsert(&geno_address, geno_account);
                }
                None => {
                    tracing::error!("post_state Process contract failed,get geno account error");
                    panic!("post_state Process contract failed,get geno account error");
                }
            }
        }

        Ok(())
    }

    pub fn convert_to_geno_txresult(&self, block_number: u64) -> Vec<TransactionResult> {
        let mut results = Vec::new();
        if let Some(receipts) = self.receipts.get(&block_number) {
            for receipt in receipts {
                let mut result = receipt.convert_to_geno_txresult();
                result.set_block_height(block_number);

                results.push(result);
            }
        }
        results
    }

    fn get_geno_account(
        state: &CacheState,
        address: &String,
    ) -> std::result::Result<Option<AccountFrame>, VmError> {
        match state.get(&address) {
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

    fn update_geno_account(geno_account: &mut AccountFrame, post_account: &PostAccount) {
        geno_account.set_balance(u256_2_u128(post_account.balance));
        geno_account.set_nonce(post_account.nonce);
    }

    fn create_geno_account(address: String, post_account: &PostAccount) -> AccountFrame {
        AccountFrame::new(address, u256_2_u128(post_account.balance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use revm::primitives::{B160, B256, U256};
    use std::str::FromStr;

    #[test]
    fn convert_test() {
        let value = U256::from(10);
        let v1 = B256::from(value);
        println!("{}", v1.to_string());
        println!("{}", &format!("{v1:?}"));

        let v2 = B256::from_str("topics").unwrap();

        if v1 != v2 {
            println!("not equal");
        } else {
            println!("equal");
        }
    }
}
