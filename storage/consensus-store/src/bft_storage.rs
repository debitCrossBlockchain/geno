use protobuf::{Message, RepeatedField};
use storage_db::{MemWriteBatch, WriteBatchTrait, STORAGE_INSTANCE_REF};

pub const CONSENSUS_PREFIX: &str = "consensus";
pub const VIEW_ACTIVE: &str = "bft_view_active";
pub const SEQUENCE_NAME: &str = "bft_sequence";
pub const VIEW_NUMBER_NAME: &str = "bft_view_number";
pub struct BftStorage {}

impl BftStorage {
    fn store_value_string(name: &str, value: &str) {
        let mut str_name = String::from(CONSENSUS_PREFIX);
        str_name.push_str("_");
        str_name.push_str(name);
        let _ = STORAGE_INSTANCE_REF
            .key_value_db()
            .lock()
            .put(str_name.as_bytes().to_vec(), value.as_bytes().to_vec());
    }

    fn store_value_i64(name: &str, value: i64) {
        BftStorage::store_value_string(name, value.to_string().as_str());
    }

    fn store_value(name: &str, value: &Vec<u8>) {
        let mut str_name = String::from(CONSENSUS_PREFIX);
        str_name.push_str("_");
        str_name.push_str(name);
        let _ = STORAGE_INSTANCE_REF
            .key_value_db()
            .lock()
            .put(str_name.as_bytes().to_vec(), value.clone());
    }

    fn load_value_string(name: &str) -> anyhow::Result<Option<String>> {
        let mut str_name = String::from(CONSENSUS_PREFIX);
        str_name.push_str("_");
        str_name.push_str(name);
        match STORAGE_INSTANCE_REF
            .key_value_db()
            .lock()
            .get(str_name.as_bytes())
        {
            Ok(value) => match value {
                Some(value) => {
                    if let Ok(s) = String::from_utf8(value) {
                        return Ok(Some(s));
                    } else {
                        return Err(anyhow::anyhow!("value is not utf8"));
                    }
                }
                None => Ok(None),
            },
            Err(e) => Err(e),
        }
    }

    fn load_value_i64(name: &str) -> anyhow::Result<Option<i64>> {
        if let Some(str_value) = BftStorage::load_value_string(name)? {
            match str_value.parse::<i64>() {
                Ok(value) => return Ok(Some(value)),
                Err(e) => return Err(anyhow::anyhow!("parse int error:{}", e)),
            }
        }
        Ok(None)
    }

    fn load_value(name: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let mut str_name = String::from(CONSENSUS_PREFIX);
        str_name.push_str("_");
        str_name.push_str(name);
        STORAGE_INSTANCE_REF
            .key_value_db()
            .lock()
            .get(name.as_bytes())
    }

    fn del_value(name: &str) -> anyhow::Result<()> {
        let mut str_name = String::from(CONSENSUS_PREFIX);
        str_name.push_str("_");
        str_name.push_str(name);
        STORAGE_INSTANCE_REF
            .key_value_db()
            .lock()
            .delete(str_name.as_bytes().to_vec())
    }

    pub fn load_view_number() -> Option<i64> {
        Self::load_value_i64(VIEW_NUMBER_NAME).expect("load view number error")
    }

    pub fn store_view_number(view_number: i64) {
        Self::store_value_i64(VIEW_NUMBER_NAME, view_number);
    }

    pub fn clear_status() {
        let _ = BftStorage::del_value(VIEW_NUMBER_NAME);
    }
}
