use protobuf::{Message, RepeatedField};
use protos::common::Signature;
use protos::ledger::*;
use utils::general::hash_crypto_byte;
#[derive(Clone, Default)]
pub struct SignedTransaction {
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
    reserves: Option<ExtendedData>,
    tx_type: TransactionType,
    pub signatures: Vec<Signature>,
    pub source_type: TransactionSign_SourceType,
}

impl SignedTransaction {
    pub fn convert_into(&self) -> TransactionSign {
        let mut tx_sig = TransactionSign::new();
        let mut tx = Transaction::new();
        tx.set_tx_type(self.tx_type());
        tx.set_source(self.sender().to_string());
        tx.set_nonce(self.nonce());
        tx.set_to(self.to().to_string());
        tx.set_value(self.value().to_string());
        tx.set_payload(self.payload.clone());
        tx.set_gas_limit(self.gas_limit());
        tx.set_gas_price(self.gas_price().to_string());
        tx.set_chain_id(self.chain_id().to_string());
        tx.set_hub_id(self.hub_id().to_string());
        if let Some(reserves) = &self.reserves {
            tx.set_reserves(reserves.clone());
        }

        tx_sig.set_transaction(tx);
        tx_sig.set_signatures(RepeatedField::from(self.signatures.clone()));
        tx_sig.set_source_type(self.source_type);
        tx_sig
    }

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

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn reserves(&self) -> &Option<ExtendedData> {
        &self.reserves
    }

    pub fn tx_type(&self) -> TransactionType {
        self.tx_type
    }
}

impl TryFrom<TransactionSign> for SignedTransaction {
    type Error = anyhow::Error;

    fn try_from(tx_sign: TransactionSign) -> anyhow::Result<Self> {
        let tx = tx_sign.get_transaction().clone();
        let reserves = if tx.has_reserves() {
            Some(tx.get_reserves().clone())
        } else {
            None
        };
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
            reserves,
            signatures: tx_sign
                .get_signatures()
                .into_iter()
                .map(|s| s.clone())
                .collect(),
            source_type: tx_sign.get_source_type(),
            tx_type: tx.get_tx_type(),
        })
    }
}

#[cfg(test)]
mod tests {
    use utils::proto2json::proto_to_json;

    use super::*;

    #[test]
    fn convert_test() {
        let mut transaction_sign = TransactionSign::default();
        let mut proto_tx = Transaction::default();

        proto_tx.set_tx_type(TransactionType::EVM_GENO);
        proto_tx.set_source("source".to_string());
        proto_tx.set_nonce(1);
        proto_tx.set_to("to".to_string());
        proto_tx.set_value("12345".to_string());
        proto_tx.set_payload("payload".as_bytes().to_vec());
        proto_tx.set_gas_limit(1000);
        proto_tx.set_gas_price("1".to_string());
        proto_tx.set_chain_id("2024".to_string());
        proto_tx.set_hub_id("hub_id".to_string());
        transaction_sign.set_transaction(proto_tx);

        let sign_transaction = match SignedTransaction::try_from(transaction_sign) {
            Ok(value) => value,
            Err(e) => panic!("{}", e),
        };
        println!("sign_transaction {}", sign_transaction.hash_hex());

        let transaction_sign2 = sign_transaction.convert_into();

        let sign_transaction2 = match SignedTransaction::try_from(transaction_sign2) {
            Ok(value) => value,
            Err(e) => panic!("{}", e),
        };
        println!("sign_transaction2 {}", sign_transaction2.hash_hex());
    }

    #[test]
    fn proto_to_json_test() {
        let mut header = LedgerHeader::default();
        header.set_height(1);
        header.set_previous_hash(vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        ]);
        let mut kv = protos::common::KeyValuePair::default();
        kv.set_key("BFT_CONSENSUS_VALUE_HASH".to_string());
        kv.set_value(vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        ]);
        header.mut_extended_data().mut_extra_data().push(kv);

        let john = proto_to_json(&header);
        // println!("{}", john);
        println!("{}", john.to_string());
    }
}
