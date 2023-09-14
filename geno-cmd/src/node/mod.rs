
use ledger_store::LedgerStorage;
use configure::CONFIGURE_INSTANCE_REF;
use clap::{Parser, ValueEnum};
use anyhow::Result;

#[derive(Debug, Parser)]
pub struct Command {

    ///get txn 
    #[clap(name = "transaction", long, short, value_name = "hash", group = "get info")]
    get_txn_by_hash:Option<String>,

    ///get block by number 
    #[clap(name = "block", long, short, value_name = "number", group = "get info")]
    get_block_by_number:Option<u64>,

    ///get block by hash, 
    #[clap(name = "block2", long, value_name = "hash", group = "get info")]
    get_block_by_hash:Option<String>,

    #[arg(value_enum, group = "get info")]
    subcmd:Option<SubCmd>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum SubCmd {
    /// the id of the node
    Id,
    /// the owner of the node
    Owner,
    /// get block number
    BlockNumber,
}

impl Command {
    pub fn run(&self)->Result<()> {
        if let Some(txhash) = self.get_txn_by_hash.as_deref() {
            println!("get txn {:?}", txhash);
            match LedgerStorage::load_tx(&txhash){
                Ok(Some(v)) => {
                    println!("transaction: {:?}", v)
                },
                Ok(None) => println!("transaction not found"),
                Err(e) => return Err(e),
            };
        }

        if let Some(number) = self.get_block_by_number {
            println!("get block {:?}", number);
            match LedgerStorage::load_ledger_header_by_seq(number){
                Ok(Some(v)) => {
                    println!("block: {:?}", v)
                },
                Ok(None) => println!("block not found"),
                Err(e) => return Err(e),
            };
        }

        if let Some(blockhash) = self.get_block_by_hash.as_deref() {
            println!("get block2 {:?}", blockhash);
            match LedgerStorage::load_ledger_header_by_hash(blockhash){
                Ok(Some(v)) => {
                    println!("block: {:?}", v)
                },
                Ok(None) => println!("block not found"),
                Err(e) => return Err(e),
            };
        }

        match self.subcmd {
            Some(SubCmd::Id) => {
                println!("node address: {:?} network:{:?}", 
                CONFIGURE_INSTANCE_REF.node_address, 
                CONFIGURE_INSTANCE_REF.network_id);
            },
            Some(SubCmd::Owner) => println!("show node owner"),
            Some(SubCmd::BlockNumber) => {
                match LedgerStorage::load_max_block_height(){
                    Ok(Some(v)) => {
                        println!("block height: {:?}", v)
                    },
                    Ok(None) => println!("block height not found"),
                    Err(e) => return Err(e),
                };
            },
            _ =>(),
        };
        Ok(())
    }
}