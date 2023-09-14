
use ledger_store::LedgerStorage;
use configure::CONFIGURE_INSTANCE_REF;
use clap::{Parser, ValueEnum};
use anyhow::Result;


#[derive(Debug, Parser)]
pub struct Command {

    #[arg(value_enum)]
    subcmd:Option<SubCmd>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum SubCmd {
    /// the info of the chain
    Info,
    /// the Genesis Block
    Genesis,
}

impl Command {
    pub fn run(&self)->Result<()> {
        match self.subcmd {
            Some(SubCmd::Info) => {
                println!("geno id: {:?} hub:{:?} version:{:?}", 
                CONFIGURE_INSTANCE_REF.chain_id,
                CONFIGURE_INSTANCE_REF.chain_hub,
                CONFIGURE_INSTANCE_REF.key_version);
            },
            Some(SubCmd::Genesis) => {
                match LedgerStorage::load_ledger_header_by_seq(0){
                    Ok(Some(v)) => {
                        println!("genesis block: {:?}", v)
                    },
                    Ok(None) => println!("genesis not found"),
                    Err(e) => return Err(e),
                };
            },
            _ =>(),
        };
        Ok(())
    }
}