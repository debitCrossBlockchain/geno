use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Data_back_config {
    pub is_open: bool,
    pub key_vaule_max_open_files: u64,
    pub ledger_db_path: String,
}

impl Clone for Data_back_config {
    fn clone(&self) -> Self {
        Self {
            is_open: self.is_open.clone(),
            key_vaule_max_open_files: self.key_vaule_max_open_files,
            ledger_db_path: self.ledger_db_path.clone(),
        }
    }
}