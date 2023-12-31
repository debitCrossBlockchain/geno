use ledger_store::LedgerStorage;
use protos::{common::ValidatorSet, consensus::BftProof, ledger::Account};
use state::{AccountFrame, TrieHash, TrieReader};
use storage_db::{MemWriteBatch, WriteBatchTrait, STORAGE_INSTANCE_REF};
use utils::{
    general::{compose_prefix_bytes, compose_prefix_str, compose_prefix_u64, hash_crypto_byte},
    parse::ProtocolParser,
};

pub const CODE_HASH_PREFIX: &str = "codehash";
pub const VALIDATORS_PREFIX: &str = "vs";
pub const LAST_PROOF: &str = "last_proof";
pub const PROOF_PREFIX: &str = "proof";
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
                        Ok(account) => return Ok(Some(AccountFrame::try_from(account)?)),
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
        batch.set(key, address.as_bytes().to_vec());
    }

    pub fn store_last_proof(batch: &mut MemWriteBatch, proof: &BftProof) {
        batch.set(
            LAST_PROOF.as_bytes().to_vec(),
            ProtocolParser::serialize::<BftProof>(proof),
        );
    }

    pub fn store_proof(batch: &mut MemWriteBatch, height: u64, proof: &BftProof) {
        let key = compose_prefix_u64(PROOF_PREFIX, height);
        batch.set(key, ProtocolParser::serialize::<BftProof>(proof));
    }

    pub fn load_last_proof() -> anyhow::Result<Option<BftProof>> {
        let result = STORAGE_INSTANCE_REF
            .account_db()
            .lock()
            .get(LAST_PROOF.as_bytes())?;

        if let Some(value) = result {
            let proof = ProtocolParser::deserialize::<BftProof>(&value)?;
            Ok(Some(proof))
        } else {
            Ok(None)
        }
    }

    pub fn load_proof(height: u64) -> anyhow::Result<Option<BftProof>> {
        let key = compose_prefix_u64(PROOF_PREFIX, height);
        let result = STORAGE_INSTANCE_REF.account_db().lock().get(&key)?;

        if let Some(value) = result {
            let proof = ProtocolParser::deserialize::<BftProof>(&value)?;
            Ok(Some(proof))
        } else {
            Ok(None)
        }
    }

    pub fn commit(batch: MemWriteBatch) -> anyhow::Result<()> {
        STORAGE_INSTANCE_REF.account_db().lock().write_batch(batch)
    }
}
