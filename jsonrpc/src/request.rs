use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::errors::JsonRpcError;

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

impl JsonRpcRequest {
    pub fn get_param(&self, index: usize) -> Value {
        self.get_param_with_default(index, Value::Null)
    }

    fn get_param_with_default(&self, index: usize, default: Value) -> Value {
        if self.params.len() > index {
            return self.params[index].clone();
        }
        default
    }

    fn try_parse_param<T>(&self, index: usize, name: &str) -> Result<T, JsonRpcError>
    where
        T: TryFrom<String>,
    {
        let raw_str: String = self.parse_param(index, name)?;
        Ok(T::try_from(raw_str).map_err(|_| JsonRpcError::invalid_param(index, name, ""))?)
    }

    pub fn parse_param<T>(&self, index: usize, name: &str) -> Result<T, JsonRpcError>
    where
        T: DeserializeOwned,
    {
        Ok(serde_json::from_value(self.get_param(index))
            .map_err(|_| JsonRpcError::invalid_param(index, name, ""))?)
    }
}
