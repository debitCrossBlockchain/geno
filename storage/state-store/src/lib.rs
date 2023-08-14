use protos::ledger::Account;
use state::{AccountFrame, TrieHash, TrieReader};
use storage_db::{MemWriteBatch, WriteBatchTrait, STORAGE_INSTANCE_REF};
use utils::{general::compose_prefix_bytes, parse::ProtocolParser};

pub const CODE_HASH_PREFIX: &str = "codehash";
pub struct StateStorage;

impl StateStorage {
    pub fn load_account(
        address: &str,
        root_hash: TrieHash,
    ) -> anyhow::Result<Option<AccountFrame>> {
        let state_db = STORAGE_INSTANCE_REF.account_db();
        let reader = TrieReader::new(state_db, Some(root_hash));
        match reader.get(address.as_bytes()) {
            Ok(result) => {
                if let Some(value) = result {
                    match ProtocolParser::deserialize::<Account>(&value) {
                        Ok(account) => return Ok(Some(AccountFrame::from_account_raw(account))),
                        Err(e) => return Err(e),
                    }
                } else {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        }
    }

    pub fn load_codehash_address_map(code_hash: &[u8]) -> anyhow::Result<Option<String>> {
        let key = compose_prefix_bytes(CODE_HASH_PREFIX, code_hash);
        match STORAGE_INSTANCE_REF.account_db().lock().get(&key) {
            Ok(result) => {
                if let Some(value) = result {
                    match String::from_utf8(value) {
                        Ok(address) => return Ok(Some(address)),
                        Err(e) => {
                            return Err(anyhow::anyhow!(
                                "address convert from bytes to string error"
                            ));
                        }
                    }
                } else {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        }
    }

    pub fn store_codehash_address_map(code_hash: &[u8], address: &str, batch: &mut MemWriteBatch) {
        let key = compose_prefix_bytes(CODE_HASH_PREFIX, code_hash);
        batch.set(&key, address.as_bytes());
    }

    pub fn commit(batch: MemWriteBatch) -> anyhow::Result<()> {
        STORAGE_INSTANCE_REF.account_db().lock().write_batch(batch)
    }
}
