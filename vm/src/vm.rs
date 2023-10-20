use crate::database::{State, VmState};
use crate::post_state::{PostAccount, PostState, Receipt};
use crate::sysvm;
use crate::utils::AddressConverter;
use crate::evm::gevm::EvmVM;

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
use tracing::{error, info};
use std::collections::BTreeMap;
use types::{error::VmError, transaction::SignedTransaction};

pub struct EvmExecutor {
    evm: EvmVM,
    header: LedgerHeader,
    state: CacheState,
}

impl EvmExecutor {
    pub fn new(
        header: &LedgerHeader,
        cache_state: CacheState,
    ) -> std::result::Result<EvmExecutor, VmError> {
        let mut evm = EvmVM::new(cache_state.clone());

        evm.fill_block_env(header)?;
        evm.fill_cfg_env(header)?;
        Ok(EvmExecutor {
            evm,
            header: header.clone(),
            state: cache_state,
        })
    }

    pub fn evm_execute(
        &mut self,
        index: usize,
        transaction: &SignedTransaction,
        post_state: &mut PostState,
    ) -> std::result::Result<(), VmError> {
        self.evm.execute(index, transaction, post_state)
    }

    pub fn wasm_execute(
        &mut self,
        index: usize,
        transaction: &SignedTransaction,
        post_state: &mut PostState,
    ) -> std::result::Result<(), VmError> {
        Ok(())
    }

    pub fn sysvm_execute(
        &mut self,
        index: usize,
        transaction: &SignedTransaction,
        post_state: &mut PostState,
    ) -> std::result::Result<(), VmError> {
        sysvm::execute(
            index,
            transaction,
            post_state,
            self.state.clone(),
            self.header.clone(),
        )
    }

    pub fn call(
        &mut self,
        transaction: &SignedTransaction,
    ) -> std::result::Result<ResultAndState, VmError> {
        self.call(transaction)
    }
}
