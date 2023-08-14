use revm::EVM;
use state::CacheState;

use crate::database::{State, VmState};

pub struct EvmExecutor {
    evm: EVM<VmState>,
}

impl EvmExecutor {
    pub fn new(cache_state: CacheState) -> EvmExecutor {
        let vm_state = VmState::new(State::new(cache_state));
        let mut evm = EVM::new();
        evm.database(vm_state);

        EvmExecutor { evm }
    }
}
