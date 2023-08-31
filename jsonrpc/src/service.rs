use utils::general::self_chain_id;

use crate::errors::JsonRpcError;

#[derive(Clone)]
pub(crate) struct JsonRpcService {
    pub chain_id: String,
    batch_size_limit: u16,
    page_size_limit: u16,
}

impl JsonRpcService {
    pub fn new(config: &configure::JsonRpcConfig) -> Self {
        Self {
            chain_id: self_chain_id(),
            batch_size_limit: config.batch_size_limit,
            page_size_limit: config.batch_size_limit,
        }
    }

    pub fn check_batch_size_limit(&self, size: usize) -> Result<(), JsonRpcError> {
        self.check_size_limit("batch size", self.batch_size_limit, size)
    }

    pub fn check_page_size_limit(&self, size: usize) -> Result<(), JsonRpcError> {
        self.check_size_limit("page size", self.page_size_limit, size)
    }

    fn check_size_limit(&self, name: &str, limit: u16, size: usize) -> Result<(), JsonRpcError> {
        if size > limit as usize {
            Err(JsonRpcError::invalid_request_with_msg(format!(
                "{} = {}, exceed limit {}",
                name, size, limit
            )))
        } else {
            Ok(())
        }
    }
}
