use serde::Deserialize;
#[derive(Deserialize, Debug)]
pub struct Db {
    pub db_type: String,
    pub key_vaule_max_open_files: u64,
    pub key_value_db_path: String,
    pub ledger_db_path: String,
    pub account_db_path: String,
}

impl Clone for Db {
    fn clone(&self) -> Self {
        Self {
            db_type: self.db_type.clone(),
            key_vaule_max_open_files: self.key_vaule_max_open_files,
            key_value_db_path: self.key_value_db_path.clone(),
            ledger_db_path: self.ledger_db_path.clone(),
            account_db_path: self.account_db_path.clone(),
        }
    }
}
