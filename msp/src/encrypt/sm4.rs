use crate::signing::check_hex;
use crate::{bytes_to_hex_str, hex_str_to_bytes};
use anyhow::Error;
use crypto::buffer::{BufferResult, ReadBuffer, WriteBuffer};
use crypto::symmetriccipher::SymmetricCipherError::InvalidLength;
use crypto::{aes, blockmodes, buffer, symmetriccipher};
use libsm::sm4::cipher_mode::{CipherMode, Sm4CipherMode};

pub fn encrypt(
    data: &[u8],
    key: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    let cmode = Sm4CipherMode::new(&key, CipherMode::Cbc);
    let ct = cmode.encrypt(&data[..], &iv);
    if ct.is_err() {
        return Err(InvalidLength);
    }
    Ok(ct.unwrap())
}

pub fn decrypt(
    encrypted_data: &[u8],
    key: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, symmetriccipher::SymmetricCipherError> {
    let cmode = Sm4CipherMode::new(&key, CipherMode::Cbc);
    let new_pt = cmode.decrypt(&encrypted_data[..], &iv);
    if new_pt.is_err() {
        return Err(InvalidLength);
    }
    Ok(new_pt.unwrap())
}
pub fn encrypt_for_special_key(encrypted_data: String) -> Result<String, anyhow::Error> {
    let key = hex::decode("F29253377076E2A341FEE9F452D1C951").unwrap();
    let data = encrypted_data.as_bytes();
    let iv = hex::decode("fedcba0987654321fedcba0987654321").unwrap();
    let re = encrypt(&*data, &*key, &*iv);
    if re.is_err() {
        return Err(anyhow::format_err!("encrypt ERR"));
    }
    let re = re.unwrap();
    let str = bytes_to_hex_str(&*re);
    Ok(str)
}
pub fn decrypt_for_special_key(encrypted_data: String) -> Result<String, anyhow::Error> {
    let key = hex::decode("F29253377076E2A341FEE9F452D1C951").unwrap();
    if !check_hex(&*encrypted_data) {
        return Err(anyhow::format_err!("data err"));
    }
    let data = hex_str_to_bytes(&*encrypted_data);
    if data.is_err() {
        return Err(anyhow::format_err!("hex_str_to_bytes ERR"));
    }
    let data = data.unwrap();
    let iv = hex::decode("fedcba0987654321fedcba0987654321").unwrap();
    let re = decrypt(&*data, &*key, &*iv);
    if re.is_err() {
        return Err(anyhow::format_err!("encrypt ERR"));
    }
    let re = re.unwrap();
    let str = String::from_utf8(re).unwrap();
    Ok(str)
}

#[test]
pub fn test() {
    let data = "eyJuIjpbNzk4MDcxOTU1LDI0OTcxMDQ3OCw0MTQ1MzcwNDk1LDM1OTQ1NTIxMTcsMTk1NDE5MzgyNiwyMDQ5MTg5Njk2LDIwOTc4ODgwNzQsNDIxNTQ4NTMxMSwzNTQ5Mzg1Nzc5LDI5OTgxMzE5NjYsMTIxMzU3MzE4NCwxODY4MzA1MjAsMzI3OTEzMDg1MCwzMjEzOTczODY4LDE5ODYyNzMwMywzMTQ5NzE1NjI2XSwiZSI6WzY1NTM3XSwiZCI6WzQyMDcyNTE5MjEsMzQxNDk1MzgyNCwzNzU2NTQ1Mzc5LDc0MjY2NzU2Miw4MjE1NDUyMzksMjcwMzE0MjEwNywxNDgwMjExMzYzLDMzODQzMDUzNDEsOTE1NjcxMjU3LDE5MTM2NTIwNDUsMzY1NTA4NzQ0MCwxODMxMzM1MTI3LDE3MDI4NDM0MzQsMzE3NTExNjkwNywyODY2ODg2MzI1LDIxMDg0NDUyMTddLCJwcmltZXMiOltbOTEzOTY5MDgxLDI5MTIzNDIwMzMsMzM0OTA1MDE4LDI2MjgzNTk0OTcsMTExNDY2ODMyLDMxNTM2NTE1NTIsMTI5Nzk4NTM5MiwzNjE0NzY0OTY2XSxbNzEzNTU1MTE1LDIzNjQ5MzUwMDQsMzk0NzIxMTI3OSw0MTY5MzY4MjAsNDAyOTkxMTMzMSwxODY1MDA4NjMyLDMzNzE0ODExNzYsMzc0MjQwODA3Nl1dfQ==";
    let result = encrypt_for_special_key(data.to_string()).unwrap();

    println!("加密 {}", result);

    let re = decrypt_for_special_key(result).unwrap();

    println!("解密 {}", re);
}
