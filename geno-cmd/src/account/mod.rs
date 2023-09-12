
mod signdata;
mod keystore;

use clap::{Parser, ValueEnum, Subcommand};
use signdata::Command as SigndataCommand; 
use keystore::Command as KeystoreCommand;
use anyhow::Result;


#[derive(Debug, Parser)]
pub struct Command {

    ///create a new account, rember your salt(password)
    #[clap(name = "create", long, short, value_name = "salt(password)")]
    create:Option<String>,

    ///unlock the account, 
    #[clap(name = "unlock", skip)]
    unlock:Option<String>,

    ///show the info of account,
    #[clap(name = "show", long, short,value_name = "address")]
    show_account:Option<String>,

    ///delete account
    #[clap(name = "delete", skip)]
    delete:Option<String>,

    #[arg(value_enum)]
    subcommand:Option<SubCmd>,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum SubCmd {
    /// list accout in the node
    #[clap(name = "list")]
    List,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "sign-data")]
    SignData(SigndataCommand),
    #[command(name = "keystore",about = "by default will create a new keystore")]
    KeyStore(KeystoreCommand),
}

impl Command {
    pub fn run(&self)->Result<()> {
        if let Some(password) = self.create.as_deref() {
            println!("create account {:?}", password)
        }

        if let Some(address) = self.show_account.as_deref() {
            println!("show account {:?}", address)
        }

        if let Some(address) = self.delete.as_deref() {
            println!("delete account {:?}", address)
        }

        match self.subcommand {
            Some(SubCmd::List) => println!("list account"),
            _ =>(),
        };

        match &self.command{
            Commands::SignData(cmd) => cmd.run(),
            Commands::KeyStore(cmd) => cmd.run(),
        }
    }
}
