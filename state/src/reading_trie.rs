use storage_db::STORAGE_INSTANCE_REF;

use crate::{AccountFrame, TrieHash, TrieReader};

pub struct ReadingTrie {
    pub reader: TrieReader,
}

impl Default for ReadingTrie {
    fn default() -> Self {
        let state_db = STORAGE_INSTANCE_REF.account_db();
        let root_hash = TrieHash::default();
        ReadingTrie {
            reader: TrieReader::new(state_db, Some(root_hash)),
        }
    }
}

impl ReadingTrie {
    pub fn is_change(&self, hash: &TrieHash) -> bool {
        self.reader.root != *hash
    }

    pub fn get_mut(
        &mut self,
        root_hash: &TrieHash,
        address: &str,
    ) -> anyhow::Result<Option<AccountFrame>> {
        let state_db = STORAGE_INSTANCE_REF.account_db();
        self.reader = TrieReader::new(state_db, Some(root_hash.clone()));
        self.get(address)
    }

    pub fn get(&self, address: &str) -> anyhow::Result<Option<AccountFrame>> {
        match self.reader.get(address.as_bytes()) {
            Ok(value) => {
                if let Some(data) = value {
                    match AccountFrame::deserialize(&data) {
                        Ok(account) => {
                            return Ok(Some(account));
                        }
                        Err(e) => return Err(anyhow::anyhow!(e)),
                    }
                } else {
                    return Ok(None);
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
}
