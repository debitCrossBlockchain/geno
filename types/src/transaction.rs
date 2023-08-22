use protobuf::Message;
use protos::common::Signature;
use protos::ledger::*;
use utils::general::hash_crypto_byte;
pub struct TransactionRaw {
    tx_hash: Vec<u8>,
    source: String,
    nonce: u64,
    value: u128,
    to: String,
    payload: Vec<u8>,
    gas_price: u128,
    gas_limit: u64,
    hub_id: String,
    chain_id: String,
}

impl TryFrom<Transaction> for TransactionRaw {
    type Error = anyhow::Error;
    fn try_from(tx: Transaction) -> anyhow::Result<Self> {
        Ok(Self {
            tx_hash: hash_crypto_byte(tx.write_to_bytes().unwrap().as_slice()),
            source: tx.get_source().to_string(),
            nonce: tx.get_nonce(),
            value: u128::from_str_radix(tx.get_value(), 10)?,
            to: tx.get_to().to_string(),
            payload: tx.get_payload().to_vec(),
            gas_price: u128::from_str_radix(tx.get_gas_price(), 10)?,
            gas_limit: tx.get_gas_limit(),
            hub_id: tx.get_hub_id().to_string(),
            chain_id: tx.get_chain_id().to_string(),
        })
    }
}

impl TransactionRaw {
    pub fn hash(&self) -> &[u8] {
        &self.tx_hash
    }

    pub fn hash_hex(&self) -> String {
        hex::encode(&self.tx_hash)
    }

    pub fn sender(&self) -> &str {
        &self.source
    }

    pub fn to(&self) -> &str {
        &self.to
    }

    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    pub fn value(&self) -> u128 {
        self.value
    }

    pub fn gas_price(&self) -> u128 {
        self.gas_price
    }

    pub fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }

    pub fn hub_id(&self) -> &str {
        &self.hub_id
    }

    pub fn code(&self) -> &[u8] {
        if self.to.is_empty() {
            &self.payload
        } else {
            &[]
        }
    }

    pub fn input(&self) -> &[u8] {
        if !self.to.is_empty() {
            &self.payload
        } else {
            &[]
        }
    }
}

pub struct TransactionSignRaw {
    pub tx: TransactionRaw,
    pub signatures: Vec<Signature>,
}

impl TryFrom<TransactionSign> for TransactionSignRaw {
    type Error = anyhow::Error;

    fn try_from(tx_sign: TransactionSign) -> anyhow::Result<Self> {
        Ok(Self {
            tx: TransactionRaw::try_from(tx_sign.get_transaction().clone())?,
            signatures: tx_sign
                .get_signatures()
                .into_iter()
                .map(|s| s.clone())
                .collect(),
        })
    }
}
