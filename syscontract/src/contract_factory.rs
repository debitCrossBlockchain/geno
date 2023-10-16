use std::{collections::HashMap, ops::Deref, sync::Arc};

use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use protos::common::{ContractResult, TransactionResult, Validator, ValidatorSet};
use state::{AccountFrame, CacheState};
use types::error::BlockExecutionError;
use utils::{general::genesis_block_config, parse::ProtocolParser};

use crate::{
    contract::{self, ContractBaseInfo, ContractContext, SystemContractTrait},
    system_address::get_system_address,
    validators_elect_contract::{ValidatorsElectContract, VALIDATORS_KEY},
};

#[derive(Clone)]
pub struct SystemContract(
    Arc<Mutex<Box<dyn SystemContractTrait<Context = ContractContext> + Send + 'static>>>,
);

impl Deref for SystemContract {
    type Target =
        Arc<Mutex<Box<dyn SystemContractTrait<Context = ContractContext> + Send + 'static>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct SystemContractFactory {
    pub contracts: RwLock<HashMap<String, SystemContract>>,
    pub contract_accounts: RwLock<HashMap<String, AccountFrame>>,
}

pub static SYSTEM_CONTRACT_FACTORY_INSTANCE: OnceCell<SystemContractFactory> = OnceCell::new();

impl SystemContractFactory {
    pub fn instance() -> &'static SystemContractFactory {
        SYSTEM_CONTRACT_FACTORY_INSTANCE
            .get()
            .expect("SystemContractFactory is not initialized")
    }

    pub fn initialize() -> SystemContractFactory {
        let mut contracts: HashMap<String, SystemContract> = HashMap::new();
        let mut contract_accounts: HashMap<String, AccountFrame> = HashMap::new();

        Self::init_validators_elect_contract(&mut contracts, &mut contract_accounts, 0);
        SystemContractFactory {
            contracts: RwLock::new(contracts),
            contract_accounts: RwLock::new(contract_accounts),
        }
    }

    pub fn invoke(
        &self,
        name: String,
        payload: &[u8],
        state: CacheState,
        invoker_address: String,
        contract_address: String,
        block_height: u64,
        block_timestamp: i64,
        tx_hash: &String,
    ) -> std::result::Result<(), BlockExecutionError> {
        match state.get(&contract_address) {
            Ok(acct) => {
                if acct.is_none() {
                    // is create sys contract
                    self.create_system_contract(&invoker_address, &contract_address);
                    return Ok(());
                }
            }
            Err(e) => {
                return Err(BlockExecutionError::InternalError {
                    error: format!("system contract state get error, {}", e),
                });
            }
        }

        let contract = {
            let r = self.contracts.read();
            let contract = match r.get(&contract_address) {
                Some(contract) => contract,
                None => {
                    return Err(BlockExecutionError::NotExistError {
                        error: format!("system contract not exist {}", contract_address),
                    });
                }
            };
            contract.clone()
        };

        let context = ContractContext {
            base_info: ContractBaseInfo {
                name,
                address: contract_address.clone(),
                invoker: invoker_address,
                block_height,
                block_timestamp,
                tx_hash: tx_hash.clone(),
            },
            state,
        };

        {
            contract.lock().init_context(context);
        }
        match Self::parse_params(payload) {
            Ok((function, params)) => {
                let contract_result = match contract.lock().dispatch(&function, params) {
                    Ok(result) => result,
                    Err(e) => {
                        let mut result = ContractResult::new();
                        result.set_err_code(-1);
                        result.set_message(format!("{}", e));
                        result
                    }
                };
                // if contract_result.err_code == 0 {}
                return Ok(());
            }
            Err(e) => {
                return Err(BlockExecutionError::TransactionParamError {
                    error: format!("system contract payload error {}", e),
                });
            }
        }
    }

    pub fn parse_params(payload: &[u8]) -> anyhow::Result<(String, serde_json::Value)> {
        let json_str = match String::from_utf8(payload.to_vec()) {
            Ok(value) => value,
            Err(e) => return Err(anyhow::anyhow!("parse params error:{}", e)),
        };

        let v: serde_json::Value = match serde_json::from_str(&json_str) {
            Ok(value) => value,
            Err(e) => return Err(anyhow::anyhow!("parse params error:{}", e)),
        };

        let method = match v.get("method") {
            Some(v) => match v.as_str() {
                Some(value) => value.to_string(),
                None => return Err(anyhow::anyhow!("parse params error:method type error")),
            },
            None => return Err(anyhow::anyhow!("parse params error:no method")),
        };
        let params = match v.get("params") {
            Some(v) => v.clone(),
            None => serde_json::Value::default(),
        };

        Ok((method, params))
    }

    fn init_validators_elect_contract(
        contracts: &mut HashMap<String, SystemContract>,
        contract_accounts: &mut HashMap<String, AccountFrame>,
        index: usize,
    ) {
        let validators_elect_address =
            get_system_address(index).expect("get validators elect address");
        let contract = ValidatorsElectContract::new(validators_elect_address.clone());
        contracts.insert(
            contract.contract_address(),
            SystemContract(Arc::new(Mutex::new(Box::new(contract)))),
        );

        let genesis_block = genesis_block_config();
        let genesis_account_address = genesis_block.genesis_account.clone();
        let mut account =
            Self::create_account_frame(&genesis_account_address, &validators_elect_address, 100000);

        //create accounts of validators from config
        let mut validator_set = ValidatorSet::new();
        for address in genesis_block.validators.iter() {
            let mut validator = Validator::new();
            validator.set_address(address.clone());
            validator.set_pledge_amount(0);
            validator_set.mut_validators().push(validator);
        }
        account.upsert_contract_metadata(
            VALIDATORS_KEY.as_bytes(),
            &ProtocolParser::serialize::<ValidatorSet>(&validator_set),
        );
        contract_accounts.insert(validators_elect_address, account);
    }

    pub fn create_account_frame(
        creator: &str,
        contract_address: &str,
        balance: u128,
    ) -> AccountFrame {
        let mut contract = protos::ledger::Contract::default();
        contract.set_creator(creator.to_string());
        let mut account = AccountFrame::new(contract_address.to_string(), balance);
        account.set_contract(&contract);
        account
    }

    pub fn all_account(&self) -> Vec<AccountFrame> {
        let mut arr = Vec::new();
        for acct in self.contract_accounts.read().values() {
            arr.push(acct.clone());
        }
        arr
    }

    pub fn create_system_contract(&self, creator: &str, contract_address: &str) {}
}

pub fn initialize_system_contract_factory() {
    let factory = SystemContractFactory::initialize();
    let _ = SYSTEM_CONTRACT_FACTORY_INSTANCE.set(factory);
}
