use serde::{Deserialize, Serialize};
use serde_json::Value;
use tx_pool::types::{TxPoolStatus, TxPoolStatusCode, TxPoolValidationStatusCode};

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

    // Mempool errors
    MempoolInvalidSeqNumber = -200,
    MempoolIsFull = -201,
    MempoolTooManyTransactions = -202,
    MempoolInvalidUpdate = -203,
    MempoolValidationError = -204,
    MempoolIsPending = -205,
    MempoolUnknownError = -206,

    ValidationError = -300,
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

    pub fn invalid_param(index: usize, name: &str, err_msg: &str) -> Self {
        Self {
            code: RpcErrorCode::InvalidParams as i16,
            message: format!("Invalid param {}(params[{}]) {}", name, index, err_msg),
            data: None,
        }
    }

    pub fn invalid_parameter(name: &str, err_msg: &str) -> Self {
        Self {
            code: RpcErrorCode::InvalidParams as i16,
            message: format!("Invalid param, {}: {}", name, err_msg),
            data: None,
        }
    }

    pub fn invalid_params_size(msg: String) -> Self {
        Self {
            code: RpcErrorCode::InvalidParams as i16,
            message: format!("Invalid params size: {}", msg),
            data: None,
        }
    }

    pub fn invalid_address(address: &str) -> Self {
        Self {
            code: RpcErrorCode::InvalidParams as i16,
            message: format!("Invalid address {}", address),
            data: None,
        }
    }

    pub fn data_not_found(string: String) -> Self {
        Self {
            code: RpcErrorCode::DataNotFound as i16,
            message: string,
            data: None,
        }
    }

    pub fn mempool_error(error: TxPoolStatus) -> anyhow::Result<Self> {
        let code = match error.code {
            TxPoolStatusCode::InvalidSeqNumber => RpcErrorCode::MempoolInvalidSeqNumber,
            TxPoolStatusCode::IsFull => RpcErrorCode::MempoolIsFull,
            TxPoolStatusCode::TooManyTransactions => RpcErrorCode::MempoolTooManyTransactions,
            TxPoolStatusCode::InvalidUpdate => RpcErrorCode::MempoolInvalidUpdate,
            TxPoolStatusCode::ValidationError => RpcErrorCode::MempoolValidationError,
            TxPoolStatusCode::UnknownStatus => RpcErrorCode::MempoolUnknownError,
            TxPoolStatusCode::Accepted => return Err(anyhow::format_err!("mempool no error")),
        };

        Ok(Self {
            code: code as i16,
            message: format!("Mempool error: {:?}", error.message),
            data: None,
        })
    }

    pub fn validation_status(error: TxPoolValidationStatusCode) -> Self {
        let code = RpcErrorCode::ValidationError;
        Self {
            code: code as i16,
            message: format!("Validation error: {:?}", error),
            data: Some(error as i16),
        }
    }
}
