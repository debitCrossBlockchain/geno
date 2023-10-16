use msp::bytes_to_hex_str;
use once_cell::sync::Lazy;
use utils::general::hash_crypto_byte;

pub static SYSTEM_CONTRACT_ADDRESS: Lazy<Vec<String>> = Lazy::new(|| {
    let mut vec = Vec::new();
    for i in 1..=50 {
        if let Some(addr) = generate_contract_address("sysaddress", i) {
            vec.push(addr);
        }
    }
    vec
});

pub fn generate_contract_address(address: &str, i: i32) -> Option<String> {
    let raw_key = format!("{}-{}", address, i);
    let raw_key_hash = hash_crypto_byte(raw_key.as_bytes());
    let private_hex = bytes_to_hex_str(raw_key_hash.as_slice());

    match msp::signing::create_private_key("eddsa_ed25519", private_hex.as_ref()) {
        Ok(p) => return Some(p.get_address()),
        Err(_err) => return None,
    };
}

pub fn is_system_contract(address: &String) -> bool {
    SYSTEM_CONTRACT_ADDRESS.contains(address)
}

pub fn get_system_address(index: usize) -> Option<String> {
    if let Some(addr) = SYSTEM_CONTRACT_ADDRESS.get(index) {
        return Some(addr.clone());
    }
    None
}

pub fn initialize_syscontract_address() {
    let _ = SYSTEM_CONTRACT_ADDRESS.len();
}
