use serde::{Deserialize, Serialize};
#[derive(Deserialize, Debug, Serialize)]
pub struct Fees {
    pub consume_gas: bool,
    pub base_reserve: u64,
    pub gas_price: u64,
    pub create_account: u64,
    pub pay_coin: u64,
    pub create_identity: u64,
    pub set_meta_data: u64,
    pub update_contract: u64,
}

impl Clone for Fees {
    fn clone(&self) -> Self {
        Self {
            consume_gas: self.consume_gas,
            base_reserve: self.base_reserve,
            gas_price: self.gas_price,
            create_account: self.create_account,
            pay_coin: self.pay_coin,
            create_identity: self.create_identity,
            set_meta_data: self.set_meta_data,
            update_contract: self.update_contract,
        }
    }
}

impl Default for Fees {
    fn default() -> Self {
        Self {
            consume_gas: true,
            base_reserve: 0,
            gas_price: 0,
            create_account: 0,
            pay_coin: 0,
            create_identity: 0,
            set_meta_data: 0,
            update_contract: 0,
        }
    }
}
