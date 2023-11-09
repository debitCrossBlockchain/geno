
use super::types::{GAS_ENV_FUNC_BASE, Context};

use anyhow::{ bail, Result};
use wasmtime::{Engine, Linker, Caller, Extern};
use tracing::{debug, error};

// env function for wasm
pub fn host_func_init(engine: &Engine) -> Result<Linker<Context>>
{
    let mut linker = Linker::new(&engine);

    linker.func_wrap("wasm", "get_owner",|mut caller: Caller<'_, Context>, ptr: i32| {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        let addr = caller.data_mut().owner;//
        let owner = &addr.0;

        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => bail!("get_owner:failed to find memory"),
        };
        match mem.write(caller, ptr as usize, owner){
            Ok(_) => (),
            Err(_) =>bail!("get_owner:MemoryAccessError"),
        };
        Ok(())
    })?;

    linker.func_wrap("wasm", "get_invoker", |mut caller: Caller<'_, Context>, ptr: i32| {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        let invoker = caller.data_mut().invoker;

        let mem = match caller.get_export("memory"){
            Some(Extern::Memory(mem)) => mem,
            _ => bail!("get_invoker:failed to find memory"),
        };
        match mem.write(caller, ptr as usize, &invoker.0){
            Ok(_) => (),
            Err(_) =>bail!("get_invoker:MemoryAccessError"),
        };
        Ok(())
    })?;

    linker.func_wrap("wasm", "get_contract_address", |mut caller: Caller<'_, Context>, ptr: i32| {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        let contract_addr = caller.data_mut().self_address;

        let mem = match caller.get_export("memory"){
            Some(Extern::Memory(mem)) => mem,
            _ => bail!("get_contract_address:failed to find memory"),
        };
        match mem.write(caller, ptr as usize, &contract_addr.0){
            Ok(_) => (),
            Err(_) =>bail!("get_contract_address:MemoryAccessError"),
        };
        Ok(())
    })?;

    linker.func_wrap("wasm", "get_contract_balance", |mut caller: Caller<'_, Context>|-> i64 {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        let t = caller.data_mut().self_balance;
        t as i64
    })?;

    linker.func_wrap("wasm", "get_parameter", |mut caller: Caller<'_, Context>, ptr: i32|-> i32 {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        
        if caller.data().param.len() > 4096{ //MAX_PREALLOCATED_CAPACITY
            return -103i32
        }
        //let param:&[u8] = &caller.data_mut().param.as_bytes();
        let args = caller.data_mut().param.clone();
        let param:&[u8] = &args.as_bytes();

        let mem = match caller.get_export("memory"){
            Some(Extern::Memory(mem)) => mem,
            _ => return -103i32,
        };
        
        match mem.write(caller, ptr as usize, param){
            Ok(_) => (),
            Err(e) => return -103i32,
        };
        param.len() as i32
    })?;


    linker.func_wrap("wasm", "get_block_time", |mut caller: Caller<'_, Context> |-> u64 {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        caller.data().metadata.block_time
    })?;

    linker.func_wrap("wasm", "get_block_height", |mut caller: Caller<'_, Context> |-> u64 {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        caller.data_mut().metadata.block_height
    })?;
    
    linker.func_wrap("wasm", "get_block_hash", |mut caller: Caller<'_, Context>, ptr:i32 |-> i32 {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        let tx:String = caller.data_mut().metadata.tx_hash.clone();
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => return -106i32,
        };
        match mem.write(caller, ptr as usize, &tx.as_bytes()){
            Ok(_) => (),
            Err(e) => return -103i32,
        };
        tx.as_bytes().len() as i32
    })?;

    linker.func_wrap("wasm", "set_event", |mut caller: Caller<'_, Context>, start: i32, length: i32 |-> i32 {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        if length <= 512 { //MAX_LOG_SIZE

            let mem = match caller.get_export("memory") {
                Some(Extern::Memory(mem)) => mem,
                _ => return -100i32,
            };
            let  data = mem.data(&caller)
                .get(start as u32 as usize..)
                .and_then(|arr| arr.get(..length as u32 as usize));
            let  event = match data {
                Some(e) => {
                    match String::from_utf8(e.to_vec()){
                        Ok(o) => o,
                        Err(_) => return -102i32,
                    }
                },
                None => return -101i32,
            };

            caller.data_mut().event.push(event);
            length
        } else {
            -102i32
        }
    })?;

    linker.func_wrap("wasm", "set_return_data",  |mut caller: Caller<'_, Context>, start: i32, len: i32| ->i32 {
        charge_gas(&mut caller, GAS_ENV_FUNC_BASE);
        
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => return -104i32,
        };
        let  data = mem.data(&caller)
            .get(start as u32 as usize..)
            .and_then(|arr| arr.get(..len as u32 as usize));
        let  output = match data {
            Some(e) => {
                match String::from_utf8(e.to_vec()){
                    Ok(o) => o,
                    Err(_) => return -102i32,
                }
            },
            None => return -105i32,
        };
        
        caller.data_mut().output_data = output;
        len as i32
    })?;

    linker.func_wrap("gas", "gas", |mut caller: Caller<'_, Context>, amount: u32| {

        if caller.data_mut().gas_outof{
            return ()
        }
        let prev = caller.data_mut().gas_counter;

        let ret = match prev.checked_add(amount as u64) {
            None => false,
            Some(val) if val > caller.data_mut().gas_limit => false,
            Some(_) => {
                caller.data_mut().gas_counter = prev + amount as u64;
                true
            }
        };

        return if ret {
            ()
        } else {
            caller.data_mut().gas_outof = true;
            ()
        }
    })?;

    linker.func_wrap("wasm", "Debug", |mut caller: Caller<'_, Context>, ptr: i32, len: i32| {
        let mem = match caller.get_export("memory"){
            Some(Extern::Memory(mem)) => mem,
            _ => { 
                error!("debug get mem err");
                return
            },
        };

        let  msg = match mem.data(&caller)
            .get(ptr as u32 as usize..)
            .and_then(|arr| arr.get(..len as u32 as usize)){
            Some(e) => match String::from_utf8(e.to_vec()){
                    Ok(o) => o,
                    Err(_) => return (),
                }
            None => return (),
        };
        debug!("WASM Debug:{:?}",msg);

    })?;

    Ok(linker)
}

fn charge_gas(caller: &mut Caller<'_, Context>, amount: u64){
    if !caller.data().gas{
        return ()
    }
    if caller.data_mut().gas_outof{
        return ()
    }
    let prev = caller.data_mut().gas_counter;
    let ret = match prev.checked_add(amount) {
        // gas charge overflow protection
        None => false,
        Some(val) if val > caller.data_mut().gas_limit => false,
        Some(_) => {
            caller.data_mut().gas_counter = prev + amount;
            true
        }
    };

    return if ret {
        ()
    } else {
        caller.data_mut().gas_outof = true;
        ()
    }
}