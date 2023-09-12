


use clap::{Parser, ValueEnum};
use anyhow::Result;

#[derive(Debug, Parser)]
pub struct Command {
    ///sign data with private key
    #[arg(value_name = "private key", long, short)]
    private_key:Option<String>,
    ///sign data with keystore
    #[arg(value_name = "keystore", long, short)]
    keystore:Option<String>,
    ///message
    #[arg(value_name = "messages")]
    message:String,
    ///algorithm type
    #[clap(name = "algorithm", long, short)]
    algorithm:Option<String>,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum ALG {
    Ed25519,
    Secp256k1,
    Sm2,
}

impl Command {
    pub fn run(&self)->Result<()> {
        if let Some(prikey) = &self.private_key{
            if let Some(alg) = &self.algorithm{
                println!("sign data with private key:{}:{}:{}",alg,prikey,self.message)
            };  
        }

        if let Some(keystore) = &self.keystore{
            println!("sign data with keystore:{}:{}",keystore,self.message)
        }
        Ok(())
    }
}
