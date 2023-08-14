pub mod hash;
pub mod signing;
pub mod rand;
pub mod encrypt;
pub mod keystore;
pub mod utils;

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
use std::sync::Arc;
pub use signing::{hex_str_to_bytes,bytes_to_hex_str};


lazy_static! {
    pub static ref HashInstanceRef: Arc<hash::Hash> = Arc::new(hash::Hash::default());
}