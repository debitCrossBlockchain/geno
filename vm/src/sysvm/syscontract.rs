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
    nonce_increase(&transaction.sender().to_string(), &state)?;
    let system_contract = match SYSTEM_CONTRACT_FACTORY_INSTANCE.get() {
        Some(s) => s,
        None => {
            return Err(VmError::InternalError {
                error: format!("can not find system contract {:?}", transaction.to()),
            })
        }
    };

    let tx_hash = transaction.hash_hex();

    let contract_result = match system_contract.invoke(
        String::new(),
        transaction.payload(),
        state,
        transaction.sender().to_owned(),
        transaction.to().to_owned(),
        env.height(),
        env.timestamp(),
        &tx_hash,
    ) {
        Ok(result) => result,
        Err(e) => {
            return Err(VmError::VMExecuteError {
                hash: tx_hash,
                message: e.to_string(),
            })
        }
    };

    let mut receipt = match Receipt::from_contract_result(&contract_result) {
        Ok(receipt) => receipt,
        Err(e) => {
            return Err(VmError::InternalError {
                error: e.to_string(),
            })
        }
    };

    receipt.index = index;
    post_state.add_receipt(env.height(), receipt);

    return Ok(());
}

fn nonce_increase(source: &String, state: &CacheState) -> std::result::Result<(), VmError> {
    let account = match state.get(source) {
        Ok(result) => result,
        Err(e) => {
            return Err(VmError::StateError {
                error: format!("{:?}", e),
            })
        }
    };
    match account {
        Some(mut account) => {
            account.nonce_increase();
            state.upsert(source, account);
        }
        None => {
            return Err(VmError::StateError {
                error: format!("can not find account {:?}", source),
            })
        }
    };
    return Ok(());
}
