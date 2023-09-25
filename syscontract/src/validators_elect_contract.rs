use protos::common::ContractResult;

use crate::contract::{ContractContext, ContractParameter, SystemContractTrait};

pub struct ValidatorsElectContract {
    pub context: ContractContext,
}

impl SystemContractTrait for ValidatorsElectContract {
    type Context = ContractContext;

    fn dispatch(&mut self, function: &str, params: ContractParameter) -> ContractResult {
        if function == "proposal" {
            return self.proposal(params);
        }
        let result = ContractResult::new();
        result
    }
    fn init_context(&mut self, context: Self::Context) {
        self.context.clone_from(&context);
    }
    fn contract_address(&self) -> String {
        self.context.base_info.address.clone()
    }
    fn invoker_address(&self) -> String {
        self.context.base_info.invoker.clone()
    }
    fn block_height(&self) -> u64 {
        self.context.base_info.block_height
    }
    fn block_timestamp(&self) -> i64 {
        self.context.base_info.block_timestamp
    }
    fn tx_hash(&self) -> String {
        self.context.base_info.tx_hash.clone()
    }
}

impl ValidatorsElectContract {
    pub fn new(contract_address: String) -> ValidatorsElectContract {
        let mut context = ContractContext::default();
        context.base_info.address.clone_from(&contract_address);
        ValidatorsElectContract { context }
    }

    fn proposal(&mut self, params: ContractParameter) -> ContractResult {
        let result = ContractResult::new();
        result
    }
}
