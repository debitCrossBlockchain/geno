
use anyhow::{anyhow, Result};
use wasmtime::{Engine, Config, Store, Module, Val};
use super::types::{Context, WasmResult, WasmError};
use super::host_func::host_func_init;
use wasmtime::Val::I64;

// execute contract
pub fn execute(func_name: &str, context: Context, binary: &[u8], amount: u64) -> Result<WasmResult> {

    let engine = Engine::new(Config::new().epoch_interruption(true))?;
    let module = Module::from_binary(&engine, binary)?;
    let linker = host_func_init(&engine)?;

    let mut store = Store::new(&engine, context );
    store.set_epoch_deadline(1);
    let instance = linker.instantiate(&mut store, &module)?;

    let engine_clone = engine.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        engine_clone.increment_epoch();
    });

    let run = match instance.get_func(&mut store, func_name){
        Some(r) => r,
        None => return Err(anyhow!("can't find {} func from contract!",func_name))
    };
    let mut r = [wasmtime::Val::null()];
    let ret = match run.call(&mut store, &[I64(amount as i64)], &mut r){
        Ok(_) => (),
        Err(trap) => {
            println!("call {} error: {:?}", func_name, trap);
            return Err(trap)
        },
    };
    let mut used_gas = store.data().gas_limit-store.data().gas_counter;
    let ret = Some(r[0].clone());

    if let Some(Val::I32(n)) = ret {
        if store.data().gas_outof{
            return Ok(WasmResult::OutOfGas)
        };
        
        if n == 1 {
            Ok(WasmResult::Success {
                event: store.data().event.clone(),
                data: store.data().output_data.clone(),
                used_gas,
            })
        } else {
            Ok(WasmResult::Reject {
                code: WasmError::ExecuteFail,
                reason: "wasm execute fail!".to_string(),
                used_gas,
            })
        }
    } else {
        Ok(WasmResult::Reject {
            code:WasmError::InvalidReturn,
            reason: "wasm return error!".to_string(),
            used_gas,
        })
    }
}

