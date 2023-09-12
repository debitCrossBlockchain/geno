

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
            Some(SubCmd::Info) => println!("show Info"),
            Some(SubCmd::Genesis) => println!("show Genesis"),
            _ =>(),
        };
        Ok(())
    }
}