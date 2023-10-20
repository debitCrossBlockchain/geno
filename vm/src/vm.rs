use crate::post_state::PostState;
use crate::sysvm;
use crate::evm::gevm::EvmVM;

use protos::ledger::LedgerHeader;
use revm::primitives::ResultAndState;
use state::CacheState;
use types::{error::VmError, transaction::SignedTransaction};

pub struct Executor {
    evm: EvmVM,
    header: LedgerHeader,
    state: CacheState,
}

impl Executor {
    pub fn new(
        header: &LedgerHeader,
        cache_state: CacheState,
    ) -> std::result::Result<Executor, VmError> {
        let mut evm = EvmVM::new(cache_state.clone());

        evm.fill_block_env(header)?;
        evm.fill_cfg_env(header)?;
        Ok(Executor {
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
        self.evm.call(transaction)
    }
}
