use revm::primitives::{
    hash_map::{self, Entry},
    AccountInfo, Address, Bytecode, ExecutionResult, Log, ResultAndState, TransactTo, TxEnv, B256,
    U256,
};
use state::{AccountFrame, CacheState};
use std::collections::{BTreeMap, BTreeSet};
use types::error::VmError;
mod account;
pub use account::{AccountChanges, PostAccount};

mod storage;
pub use storage::{Storage, StorageChanges, StorageChangeset, StorageWipe};

use crate::utils::{u256_2_u128, AddressConverter, StorageConverter};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Receipt {
    pub success: bool,
    pub logs: Vec<Log>,
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
    bytecode: BTreeMap<B256, Bytecode>,
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

    pub fn add_bytecode(&mut self, code_hash: B256, bytecode: Bytecode) {
        // Assumption: `insert` will override the value if present, but since the code hash for a
        // given bytecode will always be the same, we are overriding with the same value.
        //
        // In other words: if this entry already exists, replacing the bytecode will replace with
        // the same value, which is wasteful.
        self.bytecode.entry(code_hash).or_insert(bytecode);
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

    pub fn commit_to_geno_state(
        mut self,
        block_number: u64,
        state: CacheState,
    ) -> std::result::Result<(), VmError> {
        // process storages
        tracing::trace!(target: "post_state", "Process storages in block {}", block_number);
        for (address, storage) in self.storage.into_iter() {
            let geno_address = AddressConverter::from_evm_address(address);
            let geno_account = Self::get_geno_account(&state, &geno_address);
            if storage.wiped() {
                tracing::trace!(target: "post_state", "Wiping storage from state {} in block {}", geno_address,block_number);
            }

            for (key, value) in storage.storage {
                tracing::trace!(target: "post_state", "Updating state storage {} {:?} in block {}", geno_address,StorageConverter::from_evm_storage(key),block_number);
            }
        }

        // process accounts
        tracing::trace!(target: "post_state", "Process accounts in block {}", block_number);
        for (address, account) in self.accounts.into_iter() {
            let geno_address = AddressConverter::from_evm_address(address);

            if let Some(account) = account {
                tracing::trace!(target: "post_state", "Updating state account {}",geno_address);
                if let Some(mut geno_account) = Self::get_geno_account(&state, &geno_address)? {
                    // update account
                    Self::update_geno_account(&mut geno_account, &account);
                    state.upsert(&geno_address, geno_account);
                } else {
                    // create account
                    if account.nonce != 0 {
                        tracing::error!(target: "post_state", "Create account but nonce != 0 {}",geno_address);
                        panic!("post_state create account but nonce != 0");
                    }
                    let geno_account = Self::create_geno_account(geno_address.clone(), &account);
                    state.upsert(&geno_address, geno_account);
                }
            } else if Self::get_geno_account(&state, &geno_address)?.is_some() {
                tracing::trace!(target: "post_state","Deleting state account {}",geno_address);
                state.delete(&geno_address);
            }
        }

        // process contracts code
        tracing::trace!(target: "post_state", "Process contracts code in block {}", block_number);
        for (hash, bytecode) in self.bytecode.into_iter() {
            tracing::trace!(target: "post_state", "Process contract code hash {} in block {}",hash, block_number);
        }

        Ok(())
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
