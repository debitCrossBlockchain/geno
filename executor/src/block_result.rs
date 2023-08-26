use protos::common::{TransactionResult, ValidatorSet};
use state::CacheState;
pub struct BlockResult {
    pub state: CacheState,
    pub tx_result_set: Vec<TransactionResult>,
    pub validator_set: ValidatorSet,
}
