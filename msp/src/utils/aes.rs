use crate::bytes_to_hex_str;
use crate::encrypt::aes_cbc;

pub fn crypto_hex(input: String, key: String) -> String {
    let mut iv: [u8; 16] = [0; 16];
    let cypher = aes_cbc::encrypt(input.as_ref(), key.as_ref(), iv.as_ref())
        .ok()
        .unwrap();
    let cypher_text = cypher.as_slice();
    let cypher_text_string = bytes_to_hex_str(cypher_text);
    cypher_text_string
}
