
use std::net::SocketAddr;
use std::str::FromStr;
use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct JsonRpcConfig {
    pub address: SocketAddr,
    pub batch_size_limit: u16,
    pub page_size_limit: u16,
    pub content_length_limit: usize,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

pub const DEFAULT_JSON_RPC_ADDRESS: &str = "10.119.10.244";
pub const DEFAULT_JSON_RPC_PORT: u16 = 8080;
pub const DEFAULT_BATCH_SIZE_LIMIT: u16 = 20;
pub const DEFAULT_PAGE_SIZE_LIMIT: u16 = 1000;
pub const DEFAULT_CONTENT_LENGTH_LIMIT: usize = 32 * 1024; // 32kb

impl Clone for JsonRpcConfig{
    fn clone(&self) -> Self {
        Self{
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
//
// impl JsonRpcConfig {
//     pub fn randomize_ports(&mut self) {
//         self.address.set_port(utils::get_available_port());
//     }
// }


#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleType {
    Validator,
    FullNode,
}

impl RoleType {
    pub fn is_validator(self) -> bool {
        self == RoleType::Validator
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RoleType::Validator => "validator",
            RoleType::FullNode => "full_node",
        }
    }
}

#[derive(Debug, Error)]
pub struct ParseRoleError(String);

impl std::fmt::Display for ParseRoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl FromStr for RoleType {
    type Err = ParseRoleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "validator" => Ok(RoleType::Validator),
            "full_node" => Ok(RoleType::FullNode),
            _ => Err(ParseRoleError(s.to_string())),
        }
    }
}