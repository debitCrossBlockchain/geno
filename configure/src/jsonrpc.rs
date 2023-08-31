use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize, Debug)]
pub struct JsonRpcConfig {
    pub address: SocketAddr,
    pub batch_size_limit: u16,
    pub page_size_limit: u16,
    pub content_length_limit: usize,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

pub const DEFAULT_JSON_RPC_ADDRESS: &str = "0.0.0.0";
pub const DEFAULT_JSON_RPC_PORT: u16 = 8080;
pub const DEFAULT_BATCH_SIZE_LIMIT: u16 = 20;
pub const DEFAULT_PAGE_SIZE_LIMIT: u16 = 1000;
pub const DEFAULT_CONTENT_LENGTH_LIMIT: usize = 32 * 1024;

impl Clone for JsonRpcConfig {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            batch_size_limit: self.batch_size_limit,
            page_size_limit: self.page_size_limit,
            content_length_limit: self.content_length_limit,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

impl Default for JsonRpcConfig {
    fn default() -> JsonRpcConfig {
        JsonRpcConfig {
            address: format!("{}:{}", DEFAULT_JSON_RPC_ADDRESS, DEFAULT_JSON_RPC_PORT)
                .parse()
                .unwrap(),
            batch_size_limit: DEFAULT_BATCH_SIZE_LIMIT,
            page_size_limit: DEFAULT_PAGE_SIZE_LIMIT,
            content_length_limit: DEFAULT_CONTENT_LENGTH_LIMIT,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}
