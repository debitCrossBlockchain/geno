use msp::bytes_to_hex_str;
use protos::ledger::LedgerHeader;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct LedgerView {
    pub sequence: u64,
    pub hash: String,
    pub previous_hash: String,
    pub state_hash: String,
    pub transactions_hash: String,
    pub receips_hash: String,
    pub timestamp: i64,
    pub version: u64,
    pub tx_count: u64,
    pub total_tx_count: u64,
    pub validators_hash: String,
    pub fees_hash: String,
    pub proposer: String,
    pub hub_id: String,
    pub chain_id: String,
}

impl LedgerView {
    pub fn new(header: LedgerHeader) -> LedgerView {
        LedgerView {
            sequence: header.get_height(),
            hash: bytes_to_hex_str(header.get_hash()),
            previous_hash: bytes_to_hex_str(header.get_previous_hash()),
            state_hash: bytes_to_hex_str(header.get_state_hash()),
            transactions_hash: bytes_to_hex_str(header.get_hash()),
            receips_hash: bytes_to_hex_str(header.get_hash()),
            timestamp: header.get_timestamp(),
            version: header.get_version(),
            tx_count: header.get_tx_count(),
            total_tx_count: header.get_total_tx_count(),
            validators_hash: bytes_to_hex_str(header.get_validators_hash()),
            fees_hash: bytes_to_hex_str(header.get_fees_hash()),
            proposer: header.get_proposer().to_string(),
            hub_id: header.get_hub_id().to_string(),
            chain_id: header.get_chain_id().to_string(),
        }
    }
}
