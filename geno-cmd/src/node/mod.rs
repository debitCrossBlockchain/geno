

use clap::{Parser, ValueEnum};
use anyhow::Result;

#[derive(Debug, Parser)]
pub struct Command {

    ///get txn 
    #[clap(name = "transaction", long, short, value_name = "hash")]
    get_txn_by_hash:Option<String>,

    ///get block by number 
    #[clap(name = "block", long, short, value_name = "number")]
    get_block_by_number:Option<u128>,

    ///get block by hash, 
    #[clap(name = "block2", long, value_name = "hash")]
    get_block_by_hash:Option<String>,

    #[arg(value_enum)]
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
            println!("get txn {:?}", txhash)
        }

        if let Some(number) = self.get_block_by_number {
            println!("get block {:?}", number)
        }

        if let Some(blockhash) = self.get_block_by_hash.as_deref() {
            println!("get block2 {:?}", blockhash)
        }

        match self.subcmd {
            Some(SubCmd::Id) => println!("show node id"),
            Some(SubCmd::Owner) => println!("show node owner"),
            Some(SubCmd::BlockNumber) => println!("show block number"),
            _ =>(),
        };
        Ok(())
    }
}