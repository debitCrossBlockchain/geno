use revm::primitives::{AccountInfo, B160, B256, U256};
use state::AccountFrame;
use std::str::FromStr;
use types::error::VmError;
use utils::general::{address_add_prefix, address_filter_prefix};
pub const ADDRESS_PREFIX: &str = "did:gdt";
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
        address_add_prefix(ADDRESS_PREFIX, &format!("{address:?}"))
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

pub fn u256_into<T: TryFrom<U256>>(value: U256) -> std::result::Result<T, VmError> {
    match T::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => {
            return Err(VmError::ValueConvertError {
                error: "key value to u256 error".to_string(),
            });
        }
    }
}

pub fn u128_2_u256(value: u128) -> U256 {
    U256::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::primitives::U256;

    #[test]
    fn to_evm_address_test() {
        let addr = "did:gdt:0xf6b02a2d47b84e845b7e3623355f041bcb36daf1";
        match AddressConverter::to_evm_address(addr) {
            Ok(a) => {
                //let addr = format!("0x:{:02x}", a);
                let addr = format!("{a:?}");
                let addr2 = format!("{a:#}");
                println!("{:?}", addr);
                println!("{:?}", addr2);
            }
            Err(e) => println!("error {}", e.to_string()),
        };
    }

    #[test]
    fn u256_into_u64_test() {
        //AddressConverter::from_evm_address(value);
        let value = U256::from(10000000);
        let data: u64 = match u256_into(value) {
            Ok(a) => a,
            Err(e) => panic!("error {}", e.to_string()),
        };
        assert!(data == 10000000);
    }

    #[test]
    fn u256_into_u128_test() {
        //AddressConverter::from_evm_address(value);
        let value = U256::from(90000000);
        let data: u128 = match u256_into(value) {
            Ok(a) => a,
            Err(e) => panic!("error {}", e.to_string()),
        };
        assert!(data == 90000000);
    }
}
