use std::{collections::HashMap, ops::Deref, sync::Arc};

use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use protos::common::TransactionResult;
use state::CacheState;
use types::error::BlockExecutionError;

use crate::{
    contract::{ContractBaseInfo, ContractContext, SystemContractTrait},
    validators_elect_contract::ValidatorsElectContract,
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

        Self::init_validators_elect_contract(&mut contracts);
        SystemContractFactory {
            contracts: RwLock::new(contracts),
        }
    }

    pub fn invoke(
        &mut self,
        name: String,
        payload: &[u8],
        state: CacheState,
        invoker_address: String,
        contract_address: String,
        block_height: u64,
        block_timestamp: i64,
        tx_hash: &String,
    ) -> std::result::Result<(), BlockExecutionError> {
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
                let contract_result = contract.lock().dispatch(&function, params);
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

    fn init_validators_elect_contract(contracts: &mut HashMap<String, SystemContract>) {
        let validators_elect_address = "".to_string();
        let contract = ValidatorsElectContract::new(validators_elect_address);
        contracts.insert(
            contract.contract_address(),
            SystemContract(Arc::new(Mutex::new(Box::new(contract)))),
        );
    }
}

pub fn initialize_system_contract_factory() {
    let factory = SystemContractFactory::initialize();
    let _ = SYSTEM_CONTRACT_FACTORY_INSTANCE.set(factory);
}
