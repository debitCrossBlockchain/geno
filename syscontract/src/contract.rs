use std::{collections::HashMap, ops::Deref, sync::Arc};

use parking_lot::Mutex;
use protos::common::ContractResult;
use state::{AccountFrame, CacheState};

pub type ContractParameter = serde_json::Value;

pub trait SystemContractTrait {
    type Context;
    fn dispatch(&mut self, function: &str, params: ContractParameter) -> ContractResult;
    fn init_context(&mut self, context: Self::Context);
    fn contract_address(&self) -> String;
    fn invoker_address(&self) -> String;
    fn block_height(&self) -> u64;
    fn block_timestamp(&self) -> i64;
    fn tx_hash(&self) -> String;
}

#[derive(Clone, Default)]
pub struct ContractBaseInfo {
    pub name: String,
    pub address: String,
    pub invoker: String,
    pub block_height: u64,
    pub block_timestamp: i64,
    pub tx_hash: String,
}

#[derive(Clone, Default)]
pub struct ContractContext {
    pub base_info: ContractBaseInfo,
    pub state: CacheState,
}
