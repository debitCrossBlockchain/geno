use clap::Parser;
use anyhow::Result;

#[derive(Debug, Parser)]
pub struct Command {

    show_config:bool,
}

impl Command {
    pub fn run(&self)->Result<()> {
        println!("run config cmd!");
        Ok(())
    }
}