
pub mod cli;
pub mod config;
pub mod account;
pub mod chain;
pub mod node;
pub mod argument;

extern crate clap;
extern crate anyhow;
extern crate ledger_store;
extern crate configure;
extern crate msp;
extern crate ron;
extern crate serde_json;
#[macro_use]
extern crate serde;

pub fn run() {
    if let Err(err) = cli::run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
    //std::process::exit(0);
}