use configure::{Consensus, GenesisBlock, CONFIGURE_INSTANCE_REF};

pub const MILLI_UNITS_PER_SEC: i64 = 1000;

pub const LEDGER_VERSION: u64 = 1000;
pub const NETWORK_VERSION: u64 = 1000;

pub const GENESIS_TIMESTAMP_USECS: i64 = 0;
pub const GENESIS_HEIGHT: u64 = 0;

pub const BFT_PREVIOUS_PROOF: &str = "bft_previous_proof";
pub const BFT_CURRENT_PROOF: &str = "bft_current_proof";
pub const BFT_CONSENSUS_VALUE_HASH: &str = "bft_consensus_value hash";
pub const BFT_TX_HASH_LIST: &str = "bft_tx_hash_list";

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

pub fn hash_zero() -> Vec<u8> {
    msp::hash_zero()
}

pub fn verify_hash(content: &[u8], hash: &[u8]) -> bool {
    msp::HashInstanceRef.verify_hash(content, hash)
}

// ===========================================================================

pub fn self_chain_hub() -> String {
    CONFIGURE_INSTANCE_REF.chain_hub.clone()
}

pub fn self_chain_id() -> String {
    CONFIGURE_INSTANCE_REF.chain_id.clone()
}

pub fn node_private_key() -> String {
    CONFIGURE_INSTANCE_REF.node_private_key.clone()
}

pub fn node_address() -> String {
    CONFIGURE_INSTANCE_REF.node_address.clone()
}

pub fn consensus_config() -> Consensus {
    CONFIGURE_INSTANCE_REF.consensus.clone()
}

pub fn genesis_block_config() -> GenesisBlock {
    CONFIGURE_INSTANCE_REF.genesis_block.clone()
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

pub fn u64_2_vector(value: u64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

pub fn vector_2_u64(data: Vec<u8>) -> u64 {
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&data);
    u64::from_be_bytes(arr)
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
