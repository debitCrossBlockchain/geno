use crate::post_state::{PostAccount, PostState, Receipt};
use crate::traits::BlockEnv;
use state::CacheState;
use syscontract::contract_factory::SYSTEM_CONTRACT_FACTORY_INSTANCE;
use types::{error::VmError, transaction::SignedTransaction};

pub fn execute<E: BlockEnv>(
    index: usize,
    transaction: &SignedTransaction,
    post_state: &mut PostState,
    state: CacheState,
    env: E,
) -> std::result::Result<(), VmError> {
    let system_contract = match SYSTEM_CONTRACT_FACTORY_INSTANCE.get() {
        Some(s) => s,
        None => {
            return Err(VmError::InternalError {
                error: format!("can not find system contract {:?}", transaction.to()),
            })
        }
    };

    let tx_hash = transaction.hash_hex();

    let _ = match system_contract.invoke(
        String::new(),
        transaction.payload(),
        state,
        transaction.sender().to_owned(),
        transaction.to().to_owned(),
        env.height(),
        env.timestamp(),
        &tx_hash,
    ) {
        Ok(_) => (),
        Err(e) => {
            return Err(VmError::VMExecuteError {
                hash: tx_hash,
                message: e.to_string(),
            })
        }
    };

    post_state.add_receipt(
        env.height(),
        Receipt {
            index: index,
            success: true,
            gas_used: 0,
            contract_address: None,
            output: None,
            logs: vec![],
        },
    );

    return Ok(());
}
