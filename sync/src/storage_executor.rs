

use anyhow::bail;
use executor::block_executor::BlockExecutor;
use ledger_store::LedgerStorage;
use protos::ledger::{Ledger, LedgerHeader};
use utils::TransactionSign;


pub trait StorageExecutorInterface{

    fn execute_verify_block(&self, block: Ledger,)->anyhow::Result<()>;

    fn get_block_height(&self)->anyhow::Result<Option<u64>> {
        LedgerStorage::load_max_block_height()
    }

    fn get_block(&self, block_num: u64) -> anyhow::Result<Option<Ledger>>{
        let header = match LedgerStorage::load_ledger_header_by_seq(block_num){
            Ok(Some(v)) => v,
            Ok(None) => return Ok(None) ,
            Err(e) => bail!(e),
        };

        let txs_list = match LedgerStorage::load_ledger_tx_list(block_num){
            Ok(Some(v)) => v,
            Ok(None) => return Ok(None) ,
            Err(e) => bail!(e),
        };

        let txs:Vec<TransactionSign> = txs_list
        .get_entry()
        .iter()
        .map(|hash|{
            let tx = match LedgerStorage::load_tx(std::str::from_utf8(hash).unwrap()){
                Ok(Some(v)) => v.get_transaction_sign().to_owned(),
                Ok(None) => TransactionSign::new(),
                Err(e) => TransactionSign::new(),
            };
            tx.to_owned()
        })
        .collect();

        let mut block = Ledger::new();
        block.set_header(header);
        block.set_transaction_signs(txs.into());

        return Ok(Some(block))
    }

} 

pub struct StoreageExecutor{
    block_executor: BlockExecutor,

}

impl StoreageExecutor{
    pub fn new(block_executor: BlockExecutor,)->Self{
        Self{
            block_executor
        }
    }

    fn execute_verify_block(&self, block: Ledger) ->anyhow::Result<()> {
        match self.block_executor.execute_verify_block(block){
            Ok(_) => Ok(()),
            Err(e) => bail!(e),
        }
    }
}

impl StorageExecutorInterface for StoreageExecutor{
    fn execute_verify_block(&self, block: Ledger,)->anyhow::Result<()> {
        self.block_executor.execute_verify_block(block)
    }

}
