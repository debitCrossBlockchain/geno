use crate::{account, chain, config, node};
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "geno", author, version = "0.1", long_version = "0.0.1", about = "geno cmd", long_about =None)]
pub struct Cmd {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "chain")]
    Chain(chain::Command),
    #[command(name = "account")]
    Account(account::Command),
    #[command(name = "node")]
    Node(node::Command),
    #[command(name = "config")]
    Config(config::Command),
}

impl Cmd {
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Node(cmd) => cmd.run(),
            Commands::Chain(cmd) => cmd.run(),
            Commands::Account(cmd) => cmd.run(),
            Commands::Config(cmd) => cmd.run(),
        }
    }
}

#[inline]
pub fn run() -> Result<()> {
    match Cmd::try_parse() {
        Ok(cmd) => cmd.run(),
        Err(e) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cmd() {
        todo!()
    }
}
