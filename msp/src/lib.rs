pub mod encrypt;
pub mod hash;
pub mod keystore;
pub mod rand;
pub mod signing;
pub mod utils;

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
pub use signing::{bytes_to_hex_str, hex_str_to_bytes};
use std::sync::Arc;

lazy_static! {
    pub static ref HashInstanceRef: Arc<hash::Hash> = Arc::new(hash::Hash::default());
}

pub fn hash_zero() -> Vec<u8> {
    hash::Hash::zero()
}
