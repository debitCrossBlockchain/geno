use serde::{Deserialize, Serialize};
use serde_json::Value;

#[repr(i16)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum RpcErrorCode {
    NoError = 0,
    InternalError = -1,

    InvalidRequest = -100,
    MethodNotFound = -101,
    InvalidParams = -103,
    InvalidFormat = -104,
    DataNotFound = -105,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i16,
    pub message: String,
    pub data: Option<i16>,
}

impl std::error::Error for JsonRpcError {}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<serde_json::error::Error> for JsonRpcError {
    fn from(err: serde_json::error::Error) -> Self {
        JsonRpcError::internal_error(err.to_string())
    }
}

impl From<anyhow::Error> for JsonRpcError {
    fn from(err: anyhow::Error) -> Self {
        JsonRpcError::internal_error(err.to_string())
    }
}

impl JsonRpcError {
    pub fn serialize(self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    pub fn internal_error(message: String) -> Self {
        Self {
            code: RpcErrorCode::InternalError as i16,
            message: format!("Server error: {}", message),
            data: None,
        }
    }

    pub fn invalid_request_with_msg(msg: String) -> Self {
        Self {
            code: RpcErrorCode::InvalidRequest as i16,
            message: format!("Invalid Request: {}", msg),
            data: None,
        }
    }

    pub fn invalid_jsonrpc_format() -> Self {
        Self {
            code: RpcErrorCode::InvalidFormat as i16,
            message: "Invalid jsonrpc request format".to_string(),
            data: None,
        }
    }

    pub fn method_not_found() -> Self {
        Self {
            code: RpcErrorCode::MethodNotFound as i16,
            message: "Method not found".to_string(),
            data: None,
        }
    }

    pub fn no_error() -> Self {
        Self {
            code: RpcErrorCode::NoError as i16,
            message: format!("Success"),
            data: None,
        }
    }
}
