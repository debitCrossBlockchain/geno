use protobuf::Message;
use protos::ledger::TransactionSignBrodcast;
use std::time::Instant;
use utils::general::{hash_crypto_byte, node_address};
use utils::timing::duration_since_epoch;
pub struct TxPoolUtils {}

impl TxPoolUtils {
    pub fn generate_timestamp() -> String {
        let now = duration_since_epoch();
        let v = now.as_nanos();
        v.to_string()
    }

    pub fn generate_batch_hash(broadcast: &mut TransactionSignBrodcast) -> String {
        broadcast.mut_batchid().clear();
        let hash = msp::bytes_to_hex_str(
            hash_crypto_byte(broadcast.write_to_bytes().unwrap().as_slice()).as_ref(),
        );
        hash[0..8].to_string()
    }

    pub fn generate_node_id() -> String {
        node_address()[0..8].to_string()
    }

    pub fn generate_batch_id(broadcast: &mut TransactionSignBrodcast) {
        let bench_id = Self::generate_timestamp()
            + &Self::generate_batch_hash(broadcast)
            + &Self::generate_node_id();

        broadcast.set_batchid(bench_id);
    }

    pub fn get_timestamp(batchId: &str) -> String {
        return batchId[0..8].to_string();
    }

    pub fn get_node_id(batchId: &str) -> String {
        return batchId[8..16].to_string();
    }

    pub fn get_batch_hash(batchId: &str) -> String {
        return batchId[16..].to_string();
    }
}
