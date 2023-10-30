use anyhow::bail;
use executor::block_executor::BlockExecutor;
use ledger_store::LedgerStorage;
use msp::bytes_to_hex_str;
use protos::{
    consensus::{BftProof, TxHashList},
    ledger::{Ledger, LedgerHeader},
};
use state_store::StateStorage;
use utils::{general::hash_bytes_to_string, parse::ProtocolParser, TransactionSign};

pub trait StorageExecutorInterface {
    fn execute_verify_block(&self, block: Ledger) -> anyhow::Result<()>;

    fn get_block_height(&self) -> anyhow::Result<Option<u64>> {
        LedgerStorage::load_max_block_height()
    }

    fn get_block(&self, block_num: u64) -> anyhow::Result<Option<Ledger>> {
        let header = match LedgerStorage::load_ledger_header_by_seq(block_num) {
            Ok(Some(v)) => v,
            Ok(None) => return Ok(None),
            Err(e) => bail!(e),
        };

        let txs_list = match LedgerStorage::load_ledger_tx_list(block_num) {
            Ok(Some(v)) => v,
            Ok(None) => return Ok(None),
            Err(e) => bail!(e),
        };

        let txs: Vec<TransactionSign> = txs_list
            .get_entry()
            .iter()
            .map(|hash| {
                let tx = match LedgerStorage::load_tx(&bytes_to_hex_str(hash)) {
                    Ok(Some(v)) => v.get_transaction_sign().to_owned(),
                    Ok(None) => TransactionSign::new(),
                    Err(e) => TransactionSign::new(),
                };
                tx.to_owned()
            })
            .collect();
        let tx_hash_list = if txs.len() > 0 {
            let mut proto_hash_list = TxHashList::default();
            proto_hash_list.set_hash_set(protobuf::RepeatedField::from(txs_list.entry.to_vec()));
            Some(ProtocolParser::serialize::<TxHashList>(&proto_hash_list))
        } else {
            None
        };

        let previous_proof_data = match StateStorage::load_proof(block_num - 1) {
            Ok(Some(v)) => Some(ProtocolParser::serialize::<BftProof>(&v)),
            Ok(None) => None,
            Err(e) => bail!(e),
        };

        let mut block = BlockExecutor::initialize_new_block(
            header.get_height(),
            Vec::from(header.get_previous_hash()),
            header.get_timestamp(),
            header.get_version(),
            header.get_tx_count(),
            header.get_total_tx_count(),
            header.get_proposer().to_string(),
            previous_proof_data,
            tx_hash_list,
        );
        block.set_header(header);
        block.set_transaction_signs(txs.into());

        return Ok(Some(block));
    }
}

pub struct StoreageExecutor {
    block_executor: BlockExecutor,
}

impl StoreageExecutor {
    pub fn new(block_executor: BlockExecutor) -> Self {
        Self { block_executor }
    }

    fn execute_verify_block(&self, block: Ledger) -> anyhow::Result<()> {
        match self.block_executor.execute_verify_block(block) {
            Ok(_) => Ok(()),
            Err(e) => bail!(e),
        }
    }
}

impl StorageExecutorInterface for StoreageExecutor {
    fn execute_verify_block(&self, block: Ledger) -> anyhow::Result<()> {
        self.block_executor.execute_verify_block(block)
    }
}
