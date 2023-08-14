pub mod aes;
use sha256::{digest};
pub fn get_strong_rand_bytes(out:&str) -> String {
    digest(out)
}
pub fn get_data_secure_key() -> String {
    let key = String::from("HCPwz!H1Y3jaJ*|qw8K<eo7>Qih)rPq0");
    key
}
