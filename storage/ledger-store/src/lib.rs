use std::collections::HashMap;

use anyhow::Ok;
use protobuf::Message;
use protos::{
    common::EntryList,
    ledger::{LedgerHeader, TransactionSignStore},
};
use storage_db::{MemWriteBatch, WriteBatchTrait, STORAGE_INSTANCE_REF};
use utils::{
    general::{compose_prefix_str, compose_prefix_u64, hash_crypto_byte},
    parse::ProtocolParser,
};

pub const KEY_LEDGER_SEQ_PREFIX: &str = "ledger_max_seq";
pub const LEDGER_PREFIX: &str = "lg";
pub const LEDGER_HASH_PREFIX: &str = "lg_hash";
pub const TRANSACTION_PREFIX: &str = "tx";
pub const TRANSACTION_HASH_LIST_PREFIX: &str = "tx_lst";

pub struct LedgerStorage;

impl LedgerStorage {
    pub fn load_ledger_header_by_seq(seq: u64) -> anyhow::Result<Option<LedgerHeader>> {
        let result = STORAGE_INSTANCE_REF
            .ledger_db()
            .lock()
            .get(&compose_prefix_u64(LEDGER_PREFIX, seq))?;

        if let Some(value) = result {
            let header = ProtocolParser::deserialize::<LedgerHeader>(&value)?;
            Ok(Some(header))
        } else {
            Ok(None)
        }
    }

    pub fn load_ledger_header_by_hash(hash: &str) -> anyhow::Result<Option<LedgerHeader>> {
        let hash_key = compose_prefix_str(LEDGER_HASH_PREFIX, hash);
        let result = STORAGE_INSTANCE_REF.ledger_db().lock().get(&hash_key)?;
        let seq = if let Some(value) = result {
            utils::general::vector_2_u64(value)
        } else {
            return Ok(None);
        };

        Self::load_ledger_header_by_seq(seq)
    }

    pub fn load_ledger_tx_list(seq: u64) -> anyhow::Result<Option<EntryList>> {
        let result = STORAGE_INSTANCE_REF
            .ledger_db()
            .lock()
            .get(&compose_prefix_u64(TRANSACTION_HASH_LIST_PREFIX, seq))?;

        if let Some(value) = result {
            let list = ProtocolParser::deserialize::<EntryList>(&value)?;
            Ok(Some(list))
        } else {
            Ok(None)
        }
    }

    pub fn load_tx(hash: &str) -> anyhow::Result<Option<TransactionSignStore>> {
        let result = STORAGE_INSTANCE_REF
            .ledger_db()
            .lock()
            .get(&compose_prefix_str(TRANSACTION_PREFIX, hash))?;

        if let Some(value) = result {
            let tx = ProtocolParser::deserialize::<TransactionSignStore>(&value)?;
            Ok(Some(tx))
        } else {
            Ok(None)
        }
    }

    pub fn load_max_block_height() -> anyhow::Result<Option<u64>> {
        let result = STORAGE_INSTANCE_REF
            .ledger_db()
            .lock()
            .get(KEY_LEDGER_SEQ_PREFIX.as_bytes())?;

        if let Some(value) = result {
            let height = utils::general::vector_2_u64(value);
            Ok(Some(height))
        } else {
            Ok(None)
        }
    }

    pub fn store_max_block_height(batch: &mut MemWriteBatch, height: u64) {
        batch.set(
            KEY_LEDGER_SEQ_PREFIX.as_bytes().to_vec(),
            utils::general::u64_2_vector(height),
        );
    }

    pub fn store_ledger_header(batch: &mut MemWriteBatch, header: &LedgerHeader) {
        batch.set(
            compose_prefix_u64(LEDGER_PREFIX, header.get_height()),
            header.write_to_bytes().unwrap(),
        );

        // store ledger hash : ledger height
        let key = compose_prefix_str(LEDGER_HASH_PREFIX, &hex::encode(header.get_hash()));
        let value = utils::general::u64_2_vector(header.get_height());
        batch.set(key, value);
    }

    pub fn store_ledger_tx_list(
        batch: &mut MemWriteBatch,
        header: &LedgerHeader,
        txs: &HashMap<Vec<u8>, TransactionSignStore>,
    ) {
        let mut tx_hash_list = EntryList::new();
        for (tx_hash, tx) in txs.iter() {
            tx_hash_list.mut_entry().push(tx_hash.clone());
        }
        batch.set(
            compose_prefix_u64(TRANSACTION_HASH_LIST_PREFIX, header.get_height()),
            ProtocolParser::serialize::<EntryList>(&tx_hash_list),
        );
    }

    pub fn store_ledger_tx(
        batch: &mut MemWriteBatch,
        txs: &HashMap<Vec<u8>, TransactionSignStore>,
    ) {
        for (tx_hash, tx) in txs.iter() {
            let key = compose_prefix_str(TRANSACTION_PREFIX, &hex::encode(tx_hash));
            batch.set(key, ProtocolParser::serialize::<TransactionSignStore>(tx));
        }
    }

    pub fn store_ledger(
        batch: &mut MemWriteBatch,
        header: &LedgerHeader,
        txs: &HashMap<Vec<u8>, TransactionSignStore>,
    ) {
        Self::store_ledger_header(batch, header);
        Self::store_ledger_tx_list(batch, header, txs);
        Self::store_ledger_tx(batch, txs);
    }

    pub fn commit(batch: MemWriteBatch) -> anyhow::Result<()> {
        STORAGE_INSTANCE_REF.ledger_db().lock().write_batch(batch)
    }
}
