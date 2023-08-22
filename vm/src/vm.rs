use crate::database::{State, VmState};
use crate::post_state::{PostAccount, PostState, Receipt};
use crate::utils::AddressConverter;
use bytes::Bytes;
use protos::ledger::LedgerHeader;
use revm::{
    db::{AccountState, CacheDB, DatabaseRef},
    primitives::{
        hash_map::{self, Entry},
        Account as RevmAccount, AccountInfo, ResultAndState, TransactTo, TxEnv, B160, KECCAK_EMPTY,
        U256,
    },
    EVM,
};
use state::CacheState;
use std::collections::BTreeMap;
use types::{error::VmError, transaction::TransactionSignRaw};

pub struct EvmExecutor {
    evm: EVM<VmState>,
}

impl EvmExecutor {
    pub fn new(cache_state: CacheState) -> EvmExecutor {
        let vm_state = VmState::new(State::new(cache_state));
        let mut evm = EVM::new();
        evm.database(vm_state);

        EvmExecutor { evm }
    }

    pub fn execute(
        &mut self,
        header: &LedgerHeader,
        transaction: &TransactionSignRaw,
        post_state: &mut PostState,
    ) -> std::result::Result<(), VmError> {
        Self::fill_tx(&mut self.evm.env.tx, &transaction)?;

        // main execution.
        let out = self.evm.transact();
        let ret_and_state = match out {
            Ok(ret_and_state) => ret_and_state,
            Err(e) => {
                return Err(VmError::VMExecuteError {
                    hash: transaction.tx.hash_hex(),
                    message: format!("{e:?}"),
                });
            }
        };

        let ResultAndState { result, state } = ret_and_state;
        self.commit_changes(header.get_height(), state, true, post_state);

        post_state.add_receipt(
            header.get_height(),
            Receipt {
                // Success flag was added in `EIP-658: Embedding transaction status code in
                // receipts`.
                success: result.is_success(),
                // convert to reth log
                logs: result.into_logs().into_iter().collect(),
            },
        );

        Ok(())
    }

    fn db(&mut self) -> &mut VmState {
        self.evm.db().expect("db to not be moved")
    }

    fn commit_changes(
        &mut self,
        block_number: u64,
        changes: hash_map::HashMap<B160, RevmAccount>,
        has_state_clear_eip: bool,
        post_state: &mut PostState,
    ) {
        let db = self.db();
        Self::commit_state_changes(db, post_state, block_number, changes, has_state_clear_eip);
    }

    fn commit_state_changes(
        db: &mut VmState,
        post_state: &mut PostState,
        block_number: u64,
        changes: hash_map::HashMap<B160, RevmAccount>,
        has_state_clear_eip: bool,
    ) {
        // iterate over all changed accounts
        for (address, account) in changes {
            if account.is_destroyed {
                // get old account that we are destroying.
                let db_account = match db.accounts.entry(address) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(_entry) => {
                        panic!("Left panic to critically jumpout if happens, as every account should be hot loaded.");
                    }
                };

                let account_exists = !matches!(db_account.account_state, AccountState::NotExisting);
                if account_exists {
                    // Insert into `change` a old account and None for new account
                    // and mark storage to be wiped
                    post_state.destroy_account(
                        block_number,
                        address,
                        Self::to_post_acc(&db_account.info),
                    );
                }

                // clear cached DB and mark account as not existing
                db_account.storage.clear();
                db_account.account_state = AccountState::NotExisting;
                db_account.info = AccountInfo::default();

                continue;
            } else {
                // check if account code is new or old.
                // does it exist inside cached contracts if it doesn't it is new bytecode that
                // we are inserting inside `change`
                if let Some(ref code) = account.info.code {
                    if !code.is_empty() && !db.contracts.contains_key(&account.info.code_hash) {
                        db.contracts.insert(account.info.code_hash, code.clone());
                        post_state.add_bytecode(account.info.code_hash, address, code.clone());
                    }
                }

                // get old account that is going to be overwritten or none if it does not exist
                // and get new account that was just inserted. new account mut ref is used for
                // inserting storage
                let cached_account = match db.accounts.entry(address) {
                    Entry::Vacant(entry) => {
                        let entry = entry.insert(Default::default());
                        entry.info = account.info.clone();
                        entry.account_state = AccountState::NotExisting; // we will promote account state down the road 在未来提升帐户状态
                        let new_account = Self::to_post_acc(&entry.info);

                        #[allow(clippy::nonminimal_bool)]
                        // If account was touched before state clear EIP, create it.
                        if !has_state_clear_eip ||
                        // If account was touched after state clear EIP, create it only if it is not empty.
                        (has_state_clear_eip && !new_account.is_empty())
                        {
                            post_state.create_account(block_number, address, new_account);
                        }

                        entry
                    }
                    Entry::Occupied(entry) => {
                        let entry = entry.into_mut();

                        let old_account = Self::to_post_acc(&entry.info);
                        let new_account = Self::to_post_acc(&account.info);

                        let account_non_existent =
                            matches!(entry.account_state, AccountState::NotExisting);

                        // Before state clear EIP, create account if it doesn't exist
                        if (!has_state_clear_eip && account_non_existent)
                        // After state clear EIP, create account only if it is not empty
                        || (has_state_clear_eip && entry.info.is_empty() && !new_account.is_empty())
                        {
                            post_state.create_account(block_number, address, new_account);
                        } else if old_account != new_account {
                            post_state.change_account(
                                block_number,
                                address,
                                Self::to_post_acc(&entry.info),
                                new_account,
                            );
                        } else if has_state_clear_eip
                            && new_account.is_empty()
                            && !account_non_existent
                        {
                            // The account was touched, but it is empty, so it should be deleted.
                            // This also deletes empty accounts which were created before state clear
                            // EIP.
                            post_state.destroy_account(block_number, address, new_account);
                        }

                        entry.info = account.info.clone();
                        entry
                    }
                };

                cached_account.account_state = if account.storage_cleared {
                    cached_account.storage.clear();
                    AccountState::StorageCleared
                } else if cached_account.account_state.is_storage_cleared() {
                    // the account already exists and its storage was cleared, preserve its previous
                    // state
                    AccountState::StorageCleared
                } else if has_state_clear_eip
                    && matches!(cached_account.account_state, AccountState::NotExisting)
                    && cached_account.info.is_empty()
                {
                    AccountState::NotExisting
                } else {
                    AccountState::Touched
                };

                // Insert storage.
                let mut storage_changeset = BTreeMap::new();

                // insert storage into new db account.
                cached_account
                    .storage
                    .extend(account.storage.into_iter().map(|(key, value)| {
                        if value.is_changed() {
                            storage_changeset
                                .insert(key, (value.original_value(), value.present_value()));
                        }
                        (key, value.present_value())
                    }));

                // Insert into change.
                if !storage_changeset.is_empty() {
                    post_state.change_storage(block_number, address, storage_changeset);
                }
            }
        }
    }

    fn fill_tx(
        tx_env: &mut TxEnv,
        tx_raw: &TransactionSignRaw,
    ) -> std::result::Result<(), VmError> {
        tx_env.gas_limit = tx_raw.tx.gas_limit();
        tx_env.gas_price = U256::from(tx_raw.tx.gas_price());
        tx_env.gas_priority_fee = None;
        if tx_raw.tx.to().is_empty() {
            tx_env.transact_to = TransactTo::create();
        } else {
            let to = AddressConverter::to_evm_address(tx_raw.tx.to())?;
            tx_env.transact_to = TransactTo::Call(to);
        }

        tx_env.value = U256::from(tx_raw.tx.value());
        tx_env.data = Bytes::from(tx_raw.tx.input().to_vec());

        let chain_id = match u64::from_str_radix(tx_raw.tx.chain_id(), 10) {
            Ok(value) => value,
            Err(e) => {
                return Err(VmError::ValueConvertError {
                    error: format!(
                        "chain id {} convert error {}",
                        tx_raw.tx.chain_id(),
                        e.to_string()
                    ),
                });
            }
        };
        tx_env.chain_id = Some(chain_id);
        tx_env.nonce = Some(tx_raw.tx.nonce());
        tx_env.access_list.clear();

        Ok(())
    }

    pub fn to_post_acc(revm_acc: &AccountInfo) -> PostAccount {
        let code_hash = revm_acc.code_hash;
        PostAccount {
            balance: revm_acc.balance,
            nonce: revm_acc.nonce,
            bytecode_hash: (code_hash != KECCAK_EMPTY).then_some(code_hash),
        }
    }
}
