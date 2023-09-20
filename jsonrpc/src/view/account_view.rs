use msp::bytes_to_hex_str;
use protos::ledger::Account;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AccountView {
    pub address: String,
    pub private_key: String,
    pub public_key: String,
    pub sign_type: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AccountInfoView {
    pub address: String,
    pub nonce: u64,
    pub metadatas_hash: String,
    pub balance: String,
}
impl AccountInfoView {
    pub fn new(account: Account) -> AccountInfoView {
        AccountInfoView {
            address: account.get_address().to_string(),
            nonce: account.nonce,
            metadatas_hash: bytes_to_hex_str(account.get_metadata_hash()),
            balance: account.balance.to_string(),
        }
    }
}
