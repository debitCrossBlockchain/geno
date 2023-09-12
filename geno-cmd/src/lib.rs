
pub mod cli;
pub mod config;
pub mod account;
pub mod chain;
pub mod node;

extern crate clap;
extern crate anyhow;


pub fn run() {
    if let Err(err) = cli::run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
    std::process::exit(0);
}