
mod signdata;
mod keystore;

use clap::{Parser, ValueEnum, Subcommand};
use signdata::Command as SigndataCommand; 
use keystore::Command as KeystoreCommand;
use msp::{signing, utils};
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

    #[arg(value_enum, skip)]
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
            println!("create account {:?}", password);
            let priv_key = signing::create_secret_key("eddsa_ed25519").unwrap();
            let public_key = priv_key.get_pubkey();
            let private_key = priv_key.as_hex();
            let public_address = priv_key.get_address();
            let private_key_aes = utils::aes::crypto_hex(
                private_key.parse().unwrap(),
                utils::get_data_secure_key(),
            );
            let sign_type = priv_key.get_algorithm_name().to_string();
            println!("Creating account address:{}", public_address);
            
            #[derive(Serialize, Deserialize, Debug)]
            pub struct Data {
                public_key: String,
                private_key: String,
                address: String,
                private_key_aes: String,
                sign_type: String,
            }

            let result = Data {
                public_key,
                private_key,
                address: public_address,
                private_key_aes,
                sign_type,
            };
            let result_json = serde_json::to_string(&result).unwrap();
            println!("{}", result_json);
        }

        if let Some(address) = self.show_account.as_deref() {
            println!("show account {:?}", address)
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
