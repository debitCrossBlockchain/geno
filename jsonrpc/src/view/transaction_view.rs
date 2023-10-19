use crate::errors::JsonRpcError;
use anyhow::Error;
use msp::signing::check_address;
use protos::{
    common::{ContractEvent, Signature},
    ledger::{TransactionSign, TransactionSignStore},
};
use serde::{Deserialize, Serialize};
use syscontract::system_address::is_system_contract;
use utils::{
    general::{hash_crypto_byte, self_chain_hub, self_chain_id},
    parse::ProtocolParser,
};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ContractEventView {
    pub address: String,
    pub topic: Vec<String>,
    pub data: Vec<String>,
}

impl From<&ContractEvent> for ContractEventView {
    fn from(event: &ContractEvent) -> Self {
        Self {
            address: event.get_address().to_string(),
            topic: event.get_topic().to_vec(),
            data: event.get_data().to_vec(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct TransactionResultView {
    pub hash: String,
    pub tx_type: i32,
    pub source: String,
    pub nonce: u64,
    pub to: String,
    pub value: String,
    pub payload: String,
    pub gas_limit: u64,
    pub gas_price: String,
    pub hub_id: String,
    pub chain_id: String,
    pub error_code: i32,
    pub error_msg: String,
    pub block_height: u64,
    pub block_hash: String,
    pub gas_used: u64,
    pub events: Vec<ContractEventView>,
    pub index: i32,
    pub result: (String, Vec<u8>),
}

impl From<&TransactionSignStore> for TransactionResultView {
    fn from(tx_store: &TransactionSignStore) -> Self {
        let tx_sign = tx_store.get_transaction_sign();
        let result = tx_store.get_transaction_result();

        let hash = hash_crypto_byte(&ProtocolParser::serialize(tx_sign.get_transaction()));
        Self {
            hash: hex::encode(hash),
            tx_type: tx_sign.get_transaction().get_tx_type() as i32,
            source: tx_sign.get_transaction().get_source().to_string(),
            nonce: tx_sign.get_transaction().get_nonce(),
            to: tx_sign.get_transaction().get_to().to_string(),
            value: tx_sign.get_transaction().get_value().to_string(),
            payload: hex::encode(tx_sign.get_transaction().get_payload()),
            gas_limit: tx_sign.get_transaction().get_gas_limit(),
            gas_price: tx_sign.get_transaction().get_gas_price().to_string(),
            hub_id: tx_sign.get_transaction().get_hub_id().to_string(),
            chain_id: tx_sign.get_transaction().get_chain_id().to_string(),

            error_code: result.get_err_code(),
            error_msg: result.get_message().to_string(),
            block_height: result.get_block_height(),
            block_hash: hex::encode(result.get_block_hash()),
            gas_used: result.get_gas_used(),
            index: result.get_index() as i32,
            events: result
                .get_contract_result()
                .get_contract_event()
                .iter()
                .map(|e| ContractEventView::from(e))
                .collect(),
            result:(result.get_contract_result().get_message().to_string() ,result.get_contract_result().get_result().to_vec()),
        }
    }
}

impl From<&TransactionSign> for TransactionResultView {
    fn from(tx_sign: &TransactionSign) -> Self {
        let hash = hash_crypto_byte(&ProtocolParser::serialize(tx_sign.get_transaction()));
        Self {
            hash: hex::encode(hash),
            tx_type: tx_sign.get_transaction().get_tx_type() as i32,
            source: tx_sign.get_transaction().get_source().to_string(),
            nonce: tx_sign.get_transaction().get_nonce(),
            to: tx_sign.get_transaction().get_to().to_string(),
            value: tx_sign.get_transaction().get_value().to_string(),
            payload: hex::encode(tx_sign.get_transaction().get_payload()),
            gas_limit: tx_sign.get_transaction().get_gas_limit(),
            gas_price: tx_sign.get_transaction().get_gas_price().to_string(),
            hub_id: tx_sign.get_transaction().get_hub_id().to_string(),
            chain_id: tx_sign.get_transaction().get_chain_id().to_string(),

            error_code: 0,
            error_msg: "".to_string(),
            block_height: 0,
            block_hash: "".to_string(),
            gas_used: 0,
            index: -1,
            events: Vec::new(),
            result: ("".to_string(), Vec::new()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitTx {
    pub transaction: TransactionRaw,
    pub signature: Option<SignatureRaw>,
    #[serde(rename = "private_key")]
    pub private_key: Option<PrivateKeyRaw>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct TransactionRaw {
    pub tx_type: u32,
    pub source: String,
    pub nonce: u64,
    pub to: Option<String>,
    pub value: String,
    pub payload: Option<String>,
    pub gas_limit: u64,
    pub gas_price: String,
    pub chain_id: String,
    pub hub_id: String,
}

impl TransactionRaw {
    pub fn check_parms(&self) -> Option<Error> {
        if !check_address(self.source.as_str()) {
            return Some(Error::new(JsonRpcError::invalid_address(
                self.source.as_str(),
            )));
        } else if self.chain_id != self_chain_id() {
            return Some(Error::new(JsonRpcError::invalid_parameter(
                "chain_id",
                self.chain_id.as_str(),
            )));
        } else if self.hub_id != self_chain_hub().clone() {
            return Some(Error::new(JsonRpcError::invalid_parameter(
                "hub_id",
                self.hub_id.as_str(),
            )));
        } else {
            if let Err(e) = u128::from_str_radix(&self.value, 10) {
                return Some(Error::new(JsonRpcError::invalid_parameter(
                    "value",
                    self.value.as_str(),
                )));
            }
            if let Err(e) = u128::from_str_radix(&self.gas_price, 10) {
                return Some(Error::new(JsonRpcError::invalid_parameter(
                    "gas_price",
                    self.gas_price.as_str(),
                )));
            }

            if let Some(to) = &self.to {
                if !check_address(to.as_str()) {
                    return Some(Error::new(JsonRpcError::invalid_address(to.as_str())));
                }
            }
            return None;
        }
    }

    pub fn to_protocol(&self) -> anyhow::Result<protos::ledger::Transaction> {
        let mut protocol_tx = protos::ledger::Transaction::default();

        if self.tx_type == 0 {
            protocol_tx.set_tx_type(protos::ledger::TransactionType::EVM_GENO);
        } else if self.tx_type == 1 {
            protocol_tx.set_tx_type(protos::ledger::TransactionType::ETH_LEGACY);
        } else if self.tx_type == 2 {
            protocol_tx.set_tx_type(protos::ledger::TransactionType::WASM);
        } else {
            return Err(Error::new(JsonRpcError::invalid_parameter(
                "tx_type",
                "exception(0,1,2)",
            )));
        }

        let mut is_sys_address = false;
        protocol_tx.set_source(self.source.clone());
        protocol_tx.set_nonce(self.nonce);
        if let Some(to) = &self.to {
            protocol_tx.set_to(to.clone());
            if is_system_contract(to) {
                is_sys_address = true;
            }
        }
        protocol_tx.set_value(self.value.clone());

        if is_sys_address {
            if let Some(payload) = &self.payload {
                protocol_tx.set_payload(payload.as_bytes().to_vec());
            }
        } else {
            if let Some(payload) = &self.payload {
                match hex::decode(payload) {
                    Ok(value) => protocol_tx.set_payload(value),
                    Err(e) => {
                        return Err(Error::new(JsonRpcError::invalid_parameter(
                            "payload",
                            "decode error",
                        )))
                    }
                }
            }
        }

        protocol_tx.set_gas_limit(self.gas_limit);
        protocol_tx.set_gas_price(self.gas_price.clone());
        protocol_tx.set_chain_id(self.chain_id.clone());
        protocol_tx.set_hub_id(self.hub_id.clone());

        Ok(protocol_tx)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureRaw {
    #[serde(rename = "public_key")]
    pub public_key: ::std::string::String,
    #[serde(rename = "sign_data")]
    pub sign_data: ::std::string::String,
    #[serde(rename = "encryption_type")]
    pub encryption_type: ::std::string::String,
}

impl SignatureRaw {
    pub fn to_protocol(self) -> anyhow::Result<Signature> {
        let mut signature = Signature::new();
        let _public_key = match hex::decode(&self.public_key) {
            Ok(v) => v,
            Err(e) => {
                return Err(anyhow::anyhow!(JsonRpcError::invalid_parameter(
                    "signature",
                    "public key is error!",
                )))
            }
        };
        let _sign_data = match hex::decode(&self.sign_data) {
            Ok(v) => v,
            Err(e) => {
                return Err(anyhow::anyhow!(JsonRpcError::invalid_parameter(
                    "signature",
                    " sign_data is error!",
                )));
            }
        };
        signature.set_public_key(_public_key);
        signature.set_sign_data(_sign_data);
        signature.set_encryption_type(self.encryption_type);
        Ok(signature)
    }

    pub fn from(s: Signature) -> Self {
        Self {
            public_key: hex::encode(s.get_public_key()),
            sign_data: hex::encode(s.get_sign_data()),
            encryption_type: s.get_encryption_type().to_string(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKeyRaw {
    #[serde(rename = "priv")]
    pub priv_field: String,
    #[serde(rename = "encryption_type")]
    pub encryption_type: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct TxHash {
    pub hash: String,
}
impl TxHash {
    pub fn new(_hash: String) -> Self {
        Self { hash: _hash }
    }
}
