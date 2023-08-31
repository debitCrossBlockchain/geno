use serde::Deserialize;

pub const DEFAULT_GENESIS_ADDRESS: &str = "did:gdt:0xf6b02a2d47b84e845b7e3623355f041bcb36daf1";

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
            genesis_account: DEFAULT_GENESIS_ADDRESS.to_string(),
            validators: vec![DEFAULT_GENESIS_ADDRESS.to_string()],
        }
    }
}
