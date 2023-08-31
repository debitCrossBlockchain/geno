use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: String,
    #[serde(rename = "method")]
    pub method: String,
    #[serde(rename = "params")]
    pub params: Vec<serde_json::Value>,
    #[serde(rename = "id")]
    pub id: u64,
}
