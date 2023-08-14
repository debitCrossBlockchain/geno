use configure::{ConfigureInstanceRef, Consensus, Fees, GenesisBlock, Ledger};
use protos::{
    consensus,
    ledger::{BftValue, TransactionSign, TransactionSignBrodcast},
};

pub const CONSENSUS_PREFIX: &str = "consensus";

pub const TRANSACTION_PREFIX: &str = "tx";
pub const LEDGER_TRANSACTION_PREFIX: &str = "lg_tx";
pub const CONSENSUS_VALUE_PREFIX: &str = "consensus_value";
pub const ACCOUNT_PREFIX: &str = "acc";
pub const METADATA_PREFIX: &str = "meta";
pub static mut CHAIN_ID: &str = "0";
pub static mut CHAIN_HUB: &str = "0";
pub const KEY_LEDGER_SEQ_PREFIX: &str = "ledger_max_seq";
pub const VALIDATORS_PREFIX: &str = "validators";
pub const KEY_GENE_ACCOUNT_PREFIX: &str = "genesis_account";
pub const LAST_PROOF: &str = "last_proof";
pub const PROOF_PREFIX: &str = "proof";
pub const CONTRACT_STATE_PREFIX: &str = "contract_state";
pub const CONTRACT_SCHEMA_PREFIX: &str = "contract_schema";
pub const CONTRACT_META_PREFIX: &str = "contract_meta";
pub const ACCOUNT_META_PREFIX: &str = "account_meta";
pub const CDI_INFO_PREFIX: &str = "cdi_info";
pub const LAST_TX_HASHS: &str = "last_tx_hashs";
pub const PROPOSAL_KEY: &str = "proposal";

pub const LEDGER_VERSION: u64 = 1000;
pub const NETWORK_VERSION: u64 = 1000;

pub const HANDLE_BUF_LEN: usize = 16 * 64;
pub const P2P_LIMIT_SIZE: u32 = 20 * 1024 * 1024;
pub const BYTES_PER_KILO: u32 = 1024;
pub const KILO_PER_MEGA: u32 = 1024;
pub const BYTES_PER_MEGA: u32 = 2 * BYTES_PER_KILO * KILO_PER_MEGA;
pub const TRANSACTION_LIMIT_SIZE: u32 = BYTES_PER_MEGA;
pub const TXSET_LIMIT_SIZE: u32 = 16 * BYTES_PER_MEGA;

pub const MAX_OPERATIONS_NUM_PER_TRANSACTION: usize = 100;
pub const LAST_TX_HASHS_LIMIT: usize = 100;
pub const PEER_DB_COUNT: usize = 5000;
pub const MILLI_UNITS_PER_SEC: i64 = 1000;
pub const MICRO_UNITS_PER_MILLI: i64 = 1000;
pub const NANO_UNITS_PER_MICRO: i64 = 1000;
pub const MICRO_UNITS_PER_SEC: i64 = MICRO_UNITS_PER_MILLI * MILLI_UNITS_PER_SEC;
pub const NANO_UNITS_PER_SEC: i64 = NANO_UNITS_PER_MICRO * MICRO_UNITS_PER_SEC;

pub const TX_EXECUTE_TIME_OUT: i64 = MICRO_UNITS_PER_SEC * 2;
pub const BLOCK_EXECUTE_TIME_OUT: i64 = 5 * MICRO_UNITS_PER_SEC;

pub const TRIE_KEY_MAX_LEN: usize = 63;
pub const METADATA_KEY_MAXSIZE: usize = TRIE_KEY_MAX_LEN;
pub const METADATA_MAX_VALUE_SIZE: usize = 256 * BYTES_PER_KILO as usize;

pub static mut LEDGER_INTERVAL: i64 = 10;

pub const NODE_VOLIDATORE: u64 = 0;
pub const NODE_CANDIDATE_ADD: u64 = 1;
pub const NODE_CANDIDATE_DEL: u64 = 2;

pub const VC_MAX_LEN: usize = 2000;
pub const VC_OBJECT_MAX_LEN: usize = 1000;

pub const SIGN_DATA_MAX_LEN: usize = 1000;
pub const SIGN_SIGN_MAX_LEN: usize = 1000;
pub const SIGN_MESSAGE_MAX_LEN: usize = 1000;

pub const ALLOCATE_REWARD_CONTRACT_ADDRESS: &str = "";

pub const DEPLOYMENT_VALIDATOR_CONTRACT_NAME: &str = "validator-vote";
pub const DEPLOYMENT_VALIDATOR_CONTRACT_ADDRESS: &str =
    "did:gdt:0xf785c9e3980bd232fa27c7ee450e2a910b934308";

pub fn compose_prefix_str(prefix: &str, value: &str) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();
    result.extend_from_slice(prefix.as_bytes());
    result.extend_from_slice(value.as_bytes());
    return result;
}

pub fn compose_prefix_u64(prefix: &str, value: u64) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();
    result.extend_from_slice(prefix.as_bytes());
    result.extend_from_slice(value.to_string().as_bytes());
    return result;
}

pub fn compose_prefix_bytes(prefix: &str, value: &[u8]) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();
    result.extend_from_slice(prefix.as_bytes());
    result.extend_from_slice(value);
    return result;
}

pub fn compose_metadata_key(prefix: &str, address: &str, inner_key: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    v.extend_from_slice(prefix.as_bytes());
    v.extend_from_slice(address.as_bytes());
    v.extend_from_slice(inner_key);
    let result = hash_crypto_byte(v.as_slice());
    return result;
}

pub fn vc_overflow(key: &str) -> bool {
    if key.len() > VC_MAX_LEN {
        return true;
    }
    false
}
pub fn vc_object_overflow(key: &str) -> bool {
    if key.len() > VC_OBJECT_MAX_LEN {
        return true;
    }
    false
}
pub fn sign_data_overflow(key: &str) -> bool {
    if key.len() > SIGN_DATA_MAX_LEN {
        return true;
    }
    false
}
pub fn sign_sign_overflow(key: &str) -> bool {
    if key.len() > SIGN_SIGN_MAX_LEN {
        return true;
    }
    false
}
pub fn sign_message_overflow(key: &str) -> bool {
    if key.len() > SIGN_MESSAGE_MAX_LEN {
        return true;
    }
    false
}
pub fn hash_crypto(bytes: &[u8]) -> Vec<u8> {
    let out = msp::HashInstanceRef.hash(bytes);
    Vec::from(msp::bytes_to_hex_str(out.as_ref()))
}

pub fn hash_bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

pub fn hash_crypto_byte(bytes: &[u8]) -> Vec<u8> {
    msp::HashInstanceRef.hash(bytes)
}

pub fn verify_hash(content: &[u8], hash: &[u8]) -> bool {
    msp::HashInstanceRef.verify_hash(content, hash)
}

// ===========================================================================

pub fn self_chain_hub() -> String {
    ConfigureInstanceRef.chain_hub.clone()
}

pub fn self_chain_id() -> String {
    ConfigureInstanceRef.chain_id.clone()
}

pub fn node_private_key() -> String {
    ConfigureInstanceRef.node_private_key.clone()
}

pub fn node_address() -> String {
    ConfigureInstanceRef.address.clone()
}

pub fn consensus_config() -> Consensus {
    ConfigureInstanceRef.consensus.clone()
}

pub fn ledger_config() -> Ledger {
    ConfigureInstanceRef.ledger.clone()
}

pub fn genesis_block_config() -> GenesisBlock {
    ConfigureInstanceRef.genesis_block.clone()
}

pub fn fees_config() -> Fees {
    ConfigureInstanceRef.fees.clone()
}

pub fn address_filter_prefix(address: &str) -> String {
    if let Some(i) = address.rfind(":") {
        let new_address = address[i + 1..].to_string();
        return new_address;
    }
    address.to_string()
}

pub fn address_add_prefix(prefix: &str, address: &str) -> String {
    format!("{}:{}", prefix, address)
}

pub fn address_prefix_filter_0x() -> String {
    let prefix = msp::signing::ADDRESS_PREFIX;
    prefix[0..prefix.len() - 3].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_crypto_test() {
        let h1 = hash_crypto_byte("hello".as_bytes());
        println!("{}", h1.len());
    }
}
