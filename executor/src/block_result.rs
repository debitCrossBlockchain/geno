use protos::common::TransactionResult;
use state::CacheState;
pub struct BlockResult {
    pub state: CacheState,
    pub tx_result_set: Vec<TransactionResult>,
}
