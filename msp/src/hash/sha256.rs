/*
 * @Author: your name
 * @Date: 2022-02-18 02:39:54
 * @LastEditTime: 2022-02-18 06:27:30
 * @LastEditors: your name
 * @Description: 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
 */
use crate::bytes_to_hex_str;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use hash_db::Hasher;
use keccak_hasher::KeccakHasher;

/// Hashes the given bytes with SHA-256
pub fn hash_sha256(bytes: &[u8]) -> Vec<u8> {
    let out = KeccakHasher::hash(bytes);
    Vec::from(out)
}

/// Verifies that the SHA-256 hash of the given content matches the given hash
pub fn verify_sha256(content: &[u8], content_hash: &[u8]) -> bool {
    let computed_sha256 = hash_sha256(&content);
    if computed_sha256.as_slice() != content_hash {
        false
    } else {
        true
    }
}
// /// Hashes the given bytes with SHA-256
// pub fn hash_sha2561(bytes: &[u8]) -> Vec<u8> {
//     let mut sha = Sha256::new();
//     sha.input(bytes);
//     let mut bytes = Vec::new();
//     let hash: &mut [u8] = &mut [0; 32];
//     sha.result(hash);
//     bytes.extend(hash.iter());
//     Vec::from(bytes_to_hex_str(bytes.as_slice()))
// }
//
// /// Verifies that the SHA-256 hash of the given content matches the given hash
// pub fn verify_sha2561(content: &[u8], content_hash: &[u8]) -> bool {
//     let computed_sha256 = hash_sha256(&content);
//     if computed_sha256.as_slice() != content_hash {
//         false
//     } else {
//         true
//     }
// }
#[cfg(test)]
mod tests {
    use super::*;

    /// Nodes must be able to verify SHA-256 hashes to properly validate consensus messages from
    /// other peers, especially those that are used in consensus seals. This allows the network to
    /// verify the origin of the messages and prevents a malicious node from forging messages.
    ///
    /// This test will verify that the `verify_sha512` function properly verifies a SHA-256 hash.
    #[test]
    fn test_sha256_verification() {
        let bytes = b"abc";
        let correct_hash = [
            186, 120, 22, 191, 143, 1, 207, 234, 65, 65, 64, 222, 93, 174, 34, 35, 176, 3, 97, 163,
            150, 23, 122, 156, 180, 16, 255, 97, 242, 0, 21, 173,
        ];
        let incorrect_hash = [
            186, 121, 22, 191, 143, 1, 207, 234, 65, 65, 64, 222, 93, 174, 34, 35, 176, 3, 97, 163,
            150, 23, 122, 156, 180, 16, 255, 97, 242, 0, 21, 173,
        ];

        assert_eq!(
            verify_sha256(bytes, &Vec::from(bytes_to_hex_str(&correct_hash))),
            true
        );
        assert_eq!(verify_sha256(bytes, &incorrect_hash), false);
    }
    #[test]
    fn test_sha256() {
        let bytes = b"abc";
        let a = hash_sha256(bytes);
        let a_string = String::from_utf8(a).unwrap();
        // let b = hash_sha2561(bytes);
        // let b_string = String::from_utf8(b).unwrap();
        println!("hash - {}", a_string);
        // println!("-{}",b_string);
    }
}
