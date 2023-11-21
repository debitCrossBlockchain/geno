use std::collections::BTreeMap;

use crate::{
    database::{State, VmState},
    post_state::{PostAccount, Receipt},
    utils::AddressConverter,
    PostState,
};
use bytes::Bytes;
use protos::ledger::LedgerHeader;
use revm::{
    db::{AccountState, CacheDB, DatabaseRef},
    primitives::{
        hash_map::{self, Entry},
        Account as RevmAccount, AccountInfo, AnalysisKind, BlockEnv, CfgEnv, ExecutionResult,
        Output, ResultAndState, TransactTo, TxEnv, B160, B256, KECCAK_EMPTY, U256,
    },
    EVM,
};
use state::CacheState;
use types::{error::VmError, SignedTransaction};

pub struct EvmVM {
    evm: EVM<VmState>,
}

impl EvmVM {
    pub(crate) fn new(cache_state: CacheState) -> Self {
        let vm_state = VmState::new(State::new(cache_state.clone()));
        let mut evm = EVM::new();
        evm.database(vm_state);

        EvmVM { evm }
    }

    pub(crate) fn execute(
        &mut self,
        index: usize,
        transaction: &SignedTransaction,
        post_state: &mut PostState,
    ) -> std::result::Result<(), VmError> {
        self.fill_tx_env(&transaction)?;

        // main execution.
        let out = self.evm.transact();
        let ret_and_state = match out {
            Ok(ret_and_state) => ret_and_state,
            Err(e) => {
                return Err(VmError::VMExecuteError {
                    hash: transaction.hash_hex(),
                    message: format!("{:?}", e),
                });
            }
        };

        let ResultAndState { result, state } = ret_and_state;
        let (output, contract_address) = match result.clone() {
            ExecutionResult::Success { output, .. } => match output {
                Output::Call(value) => (Some(value.into()), None),
                Output::Create(value, address) => (Some(value.into()), address),
            },
            ExecutionResult::Revert { gas_used, output } => {
                println!(
                    "Execution reverted: gas used: {}, output: {:?}",
                    gas_used, output
                );
                (Some(output.into()), None)
            }
            ExecutionResult::Halt { reason, gas_used } => {
                println!("Execution halted: {:?}, gas used: {}", reason, gas_used);
                (None, None)
            }
        };

        let blocknum = match u64::try_from(self.evm.env.block.number) {
            Ok(n) => n,
            Err(e) => {
                return Err(VmError::InternalError {
                    error: format!("{:?}", e),
                });
            }
        };
        self.commit_changes(blocknum, state, true, post_state);

        post_state.add_receipt(
            blocknum,
            Receipt {
                index: index,
                // Success flag was added in `EIP-658: Embedding transaction status code in
                // receipts`.
                success: result.is_success(),
                gas_used: result.gas_used(),
                contract_address: contract_address,
                output: output,
                // convert to reth log
                logs: result.into_logs().into_iter().collect(),
                description: None,
            },
        );

        Ok(())
    }

    fn db(&mut self) -> &mut VmState {
        self.evm.db().expect("EVMdb to not be moved")
    }

    pub fn to_post_acc(revm_acc: &AccountInfo) -> PostAccount {
        let code_hash = revm_acc.code_hash;
        PostAccount {
            balance: revm_acc.balance,
            nonce: revm_acc.nonce,
            bytecode_hash: (code_hash != KECCAK_EMPTY).then_some(code_hash),
        }
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

    fn fill_tx_env(&mut self, tx_raw: &SignedTransaction) -> std::result::Result<(), VmError> {
        self.evm.env.tx.caller = AddressConverter::to_evm_address(tx_raw.sender())?;
        self.evm.env.tx.gas_limit = tx_raw.gas_limit();
        self.evm.env.tx.gas_price = U256::from(tx_raw.gas_price());
        self.evm.env.tx.gas_priority_fee = None;
        if tx_raw.to().is_empty() {
            self.evm.env.tx.transact_to = TransactTo::create();
        } else {
            let to = AddressConverter::to_evm_address(tx_raw.to())?;
            self.evm.env.tx.transact_to = TransactTo::Call(to);
        }

        self.evm.env.tx.value = U256::from(tx_raw.value());
        self.evm.env.tx.data = Bytes::from(tx_raw.payload().to_vec());

        let chain_id = match u64::from_str_radix(tx_raw.chain_id(), 10) {
            Ok(value) => value,
            Err(e) => {
                return Err(VmError::ValueConvertError {
                    error: format!(
                        "chain id {} convert error {}",
                        tx_raw.chain_id(),
                        e.to_string()
                    ),
                });
            }
        };
        self.evm.env.tx.chain_id = Some(chain_id);
        self.evm.env.tx.nonce = Some(tx_raw.nonce());
        self.evm.env.tx.access_list.clear();

        Ok(())
    }

    pub(crate) fn fill_block_env(
        &mut self,
        header: &LedgerHeader,
    ) -> std::result::Result<(), VmError> {
        self.evm.env.block.number = U256::from(header.get_height());
        self.evm.env.block.coinbase = AddressConverter::to_evm_address(header.get_proposer())?;
        self.evm.env.block.timestamp = U256::from(header.get_timestamp());

        self.evm.env.block.prevrandao = Some(B256::from(U256::from(1)));
        self.evm.env.block.difficulty = U256::ZERO;
        self.evm.env.block.basefee = U256::ZERO;
        self.evm.env.block.gas_limit = U256::MAX;
        Ok(())
    }

    pub(crate) fn fill_cfg_env(
        &mut self,
        header: &LedgerHeader,
    ) -> std::result::Result<(), VmError> {
        let chain_id = match u64::from_str_radix(header.get_chain_id(), 10) {
            Ok(value) => value,
            Err(e) => {
                return Err(VmError::ValueConvertError {
                    error: format!(
                        "chain id {} convert error {}",
                        header.get_chain_id(),
                        e.to_string()
                    ),
                });
            }
        };
        self.evm.env.cfg.chain_id = U256::from(chain_id);
        self.evm.env.cfg.spec_id = revm::primitives::CANCUN;
        self.evm.env.cfg.perf_all_precompiles_have_balance = false;
        self.evm.env.cfg.perf_analyse_created_bytecodes = AnalysisKind::Analyse;
        Ok(())
    }

    pub fn call(
        &mut self,
        transaction: &SignedTransaction,
    ) -> std::result::Result<ResultAndState, VmError> {
        self.fill_tx_env(&transaction)?;

        // main execution.
        let out = self.evm.transact();
        let ret_and_state = match out {
            Ok(ret_and_state) => ret_and_state,
            Err(e) => {
                return Err(VmError::VMExecuteError {
                    hash: transaction.hash_hex(),
                    message: format!("{:?}", e),
                });
            }
        };
        Ok(ret_and_state)
    }
}
