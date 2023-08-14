/*
 * @Author: your name
 * @Date: 2022-02-18 02:39:54
 * @LastEditTime: 2022-02-18 06:27:41
 * @LastEditors: your name
 * @Description: 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
 */
use crate::{bytes_to_hex_str, hex_str_to_bytes};
use libsm::sm3::hash::Sm3Hash;

/// Hashes the given bytes with sm3
pub fn hash_sm3(bytes: &[u8]) -> Vec<u8> {
    let mut sm3 = Sm3Hash::new(bytes);
    let out = sm3.get_hash();
    Vec::from(out)
}

/// Verifies that the sm3 hash of the given content matches the given hash
pub fn verify_sm3(content: &[u8], content_hash: &[u8]) -> bool {
    let computed_sha256 = hash_sm3(&content);
    if computed_sha256.as_slice() != content_hash {
        false
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nodes must be able to verify sm3 hashes to properly validate consensus messages from
    /// other peers, especially those that are used in consensus seals. This allows the network to
    /// verify the origin of the messages and prevents a malicious node from forging messages.
    ///
    /// This test will verify that the `verify_sm3` function properly verifies a sm3 hash.
    #[test]
    fn test_sm3_verification() {
        let bytes = b"123";
        let hash = hash_sm3(bytes);
        let hash_string = String::from_utf8(hash).unwrap();
        // let hash_string = bytes_to_hex_str(hash.as_slice());
        println!("hash : {:}", hash_string);

        //https://aks.jd.com/tools/sec/
        let net_query = "6e0f9e14344c5406a0cf5a3b4dfb665f87f4a771a31f7edbb5c72874a32b2957";
        let net_query_hash = String::from_utf8(Vec::from(net_query)).unwrap();
        // let net_query_hash = hex_str_to_bytes(net_query).unwrap();
        assert_eq!(verify_sm3(bytes, net_query_hash.as_ref()), true);

        //wrong hash test
        let net_query_err = "8f30b2c2ab4c30470692874e3408a8dfb15c02b6b569a9215a3e29e8c9ccd093";
        let net_query_hash_err = String::from_utf8(Vec::from(net_query_err)).unwrap();
        // let net_query_hash_err = hex_str_to_bytes(net_query_err).unwrap();
        assert_eq!(verify_sm3(bytes, net_query_hash_err.as_ref()), false);
    }
}
