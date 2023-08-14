use serde::{Deserialize, Serialize};
#[derive(Deserialize, Debug)]
pub struct GenesisBlock {
    pub genesis_account: String,
    pub validators: Vec<String>,
}
impl Clone for GenesisBlock {
    fn clone(&self) -> Self {
        Self {
            genesis_account: self.genesis_account.clone(),
            validators: self.validators.clone(),
        }
    }
}

impl Default for GenesisBlock {
    fn default() -> Self {
        Self {
            genesis_account: "".to_string(),
            validators: vec![],
        }
    }
}
