use revm::primitives::{AccountInfo, B160, B256, U256};
use state::AccountFrame;
use std::str::FromStr;
use types::error::VmError;
use utils::general::{address_add_prefix, address_filter_prefix};
pub const ADDRESS_PREFIX: &str = "did:gdt:0x";
pub struct AddressConverter;

impl AddressConverter {
    pub fn to_evm_address(address: &str) -> std::result::Result<B160, VmError> {
        match B160::from_str(&address_filter_prefix(address)) {
            Ok(address) => Ok(address),
            Err(e) => Err(VmError::AddressConvertError {
                error: e.to_string(),
            }),
        }
    }

    pub fn from_evm_address(address: B160) -> String {
        address_add_prefix(ADDRESS_PREFIX, &address.to_string())
    }
}

pub struct StorageConverter;
impl StorageConverter {
    pub fn to_evm_storage(value: &[u8]) -> std::result::Result<U256, VmError> {
        if let Some(u_value) = U256::try_from_le_slice(value) {
            Ok(u_value)
        } else {
            return Err(VmError::ValueConvertError {
                error: "key value to u256 error".to_string(),
            });
        }
    }

    pub fn from_evm_storage(value: U256) -> Vec<u8> {
        value.as_le_slice().to_vec()
    }
}

pub fn u256_2_u128(value: U256) -> u128 {
    let b256: B256 = value.into();
    let u256: primitive_types::U256 = b256.into();
    u256.as_u128()
}

pub fn u128_2_u256(value: u128) -> U256 {
    U256::from(value)
}
