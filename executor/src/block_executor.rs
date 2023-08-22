use crate::block_result::BlockResult;
use protos::ledger::*;
use state::{CacheState, TrieHash};
use types::error::BlockExecutionError;
use types::transaction::TransactionSignRaw;
use vm::{EvmExecutor, PostState};
pub struct BlockExecutor {}

impl BlockExecutor {
    pub fn execute_block(
        &self,
        block: Ledger,
    ) -> std::result::Result<BlockResult, BlockExecutionError> {
        let mut result = BlockResult::default();
        let header = block.get_header();

        // initialize state by last block state root
        let root_hash = TrieHash::default();
        let state = CacheState::new(root_hash);

        // initialize contract vm
        let mut vm = EvmExecutor::new(state.clone());
        let mut post_state = PostState::new();

        // execute block
        for tx in block.get_transaction_signs() {
            let tx_raw = match TransactionSignRaw::try_from(tx.clone()) {
                Ok(tx_raw) => tx_raw,
                Err(e) => {
                    return Err(BlockExecutionError::TransactionParamError {
                        error: e.to_string(),
                    })
                }
            };
            let _ = vm.execute(block.get_header(), &tx_raw, &mut post_state);
        }
        let r = post_state.commit_to_geno_state(header.get_height(), state);

        Ok(result)
    }

    pub fn commit_block(&self) {}
}
