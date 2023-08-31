use crate::errors::JsonRpcError;
use serde::{Deserialize, Serialize};

pub const HEADER_CHAIN_ID: &str = "CHAIN-ID";

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub chain_id: String,
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn new(chain_id: String) -> Self {
        Self {
            chain_id,
            jsonrpc: "2.0".to_string(),
            id: None,
            result: None,
            error: None,
        }
    }
}
