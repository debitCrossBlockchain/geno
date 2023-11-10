
use anyhow::{ bail, Result};

pub const ADDRESS_SIZE: usize = 20;
pub const GAS_ENV_FUNC_BASE: u64 = 1;


#[derive(Default, Clone, Copy)]
pub struct AccountAddress(pub [u8; ADDRESS_SIZE]);


#[derive(Default, Clone)]
pub struct Metadata {
    pub block_time: u64,
    pub block_height: u64,  
    pub tx_hash: String,  
}

impl Metadata{
    pub fn transfer(&self, address:AccountAddress, amount:u64)->Result<()>{
        bail!("transfer fail!");
    }

    pub fn call(&self, address:AccountAddress, amount:u64, func:String, arg:String)->Result<()>{
        bail!("invoker fail!");
    }

    pub fn set(&self, key:&[u8], value:&[u8]) -> Result<()>{
        bail!("set fail!");
    }

    pub fn get(&self, key:&[u8]) -> Result<Vec<u8>>{
        bail!("get fail!");
    }
}

#[derive(Default)]
pub struct Context {
    pub(crate) func_name: String,
    pub(crate) state: String,
    pub(crate) param: String,
    pub(crate) invoker:  AccountAddress,
    pub(crate) owner:   AccountAddress,
    pub(crate) self_address: AccountAddress,
    pub(crate) event: Vec<String>,
    pub(crate) self_balance: u64,
    pub(crate) output_data: String,
    pub(crate) metadata: Metadata,
    pub(crate) gas: bool,
    pub(crate) gas_counter: u64,
    pub(crate) gas_limit: u64,
    pub(crate) gas_outof: bool,
    
}

impl Context {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn init(
        func_name: String,
        state: String,
        param: String,
        invoker:  AccountAddress,
        owner:   AccountAddress,
        self_address: AccountAddress,
        self_balance: u64,
        metadata: Metadata,
        gas: bool,
        gas_limit: u64,
    ) -> Self{
        Context{
            func_name,
            state,
            param,
            invoker,
            owner,
            self_address,
            event: Vec::new(),
            self_balance,
            output_data: String::new(),
            metadata,
            gas,
            gas_counter: 0,
            gas_limit,
            gas_outof: false,
        }
    }
}

#[derive(Debug)]
pub enum WasmResult {
    Success {
        used_gas:  u64,
        event:     Vec<String>,
        data:      String,
    },
    Reject {
        code:     WasmError,
        reason:   String,
        used_gas: u64,
    },
    OutOfGas,
}

#[derive(Debug)]
pub enum WasmError {
    CodeNotFound = 1000,
    InvalidModule,
    CantDeserializeWasm,
    InvalidMemory,
    InvalidHeapPages,
    OutOfGas,
    ExecuteFail,
    InvalidReturn,
    Instantiation,
    KeyNotFound,
    HostFuncErr,
}

