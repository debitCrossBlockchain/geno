use protobuf::Message;
use protos::ledger::LedgerHeader;
use storage_db::{MemWriteBatch, STORAGE_INSTANCE_REF};
use utils::{general::compose_prefix_u64, parse::ProtocolParser};

pub const LEDGER_PREFIX: &str = "lg";
pub struct LedgerStorage;

impl LedgerStorage {
    pub fn load_ledger_header(seq: u64) -> anyhow::Result<Option<LedgerHeader>> {
        match STORAGE_INSTANCE_REF
            .ledger_db()
            .lock()
            .get(&compose_prefix_u64(LEDGER_PREFIX, seq))
        {
            Ok(result) => {
                if let Some(value) = result {
                    match ProtocolParser::deserialize::<LedgerHeader>(&value) {
                        Ok(header) => Ok(Some(header)),
                        Err(e) => Err(e),
                    }
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(e),
        }
    }

    pub fn store_ledger_header(header: LedgerHeader) -> anyhow::Result<()> {
        STORAGE_INSTANCE_REF.ledger_db().lock().put(
            compose_prefix_u64(LEDGER_PREFIX, header.get_height()),
            header.write_to_bytes().unwrap(),
        )
    }

    pub fn commit(batch: MemWriteBatch) -> anyhow::Result<()> {
        STORAGE_INSTANCE_REF.account_db().lock().write_batch(batch)
    }
}
