pub mod aes_cbc;
pub mod jni;
pub mod sm4;

use crypto;
use crypto::symmetriccipher;

pub fn encrypt_aes(
    data: &[u8],
    key: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    aes_cbc::encrypt(data, key, iv)
}

pub fn decrypt_aes(
    encrypt_data: &[u8],
    key: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    aes_cbc::decrypt(encrypt_data, key, iv)
}

pub fn encrypt_sm4(
    data: &[u8],
    key: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    sm4::encrypt(data, key, iv)
}

pub fn decrypt_sm4(
    encrypt_data: &[u8],
    key: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    sm4::decrypt(encrypt_data, key, iv)
}
