


use clap::{Parser, ValueEnum};
use anyhow::Result;

#[derive(Debug, Parser)]
pub struct Command {
    ///password, 
    #[arg(value_name = "password")]
    password:String,
    ///create keystore from private key
    #[arg(value_name = "private key", long)]
    private_key:Option<String>,
    ///keystore json
    #[arg(value_name = "keystore", long="get-privatekey")]
    keystore:Option<String>,
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

        if let Some(alg) = &self.algorithm{
            if let Some(priky) = &self.private_key{
                println!("create keystore from private key:{}:{}:{}",alg,priky,self.password)
            }else{
                println!("create keystore:{}:{}",alg,self.password)
            }
        };

        //get private key
        if let Some(kyeystore) = &self.keystore {
            println!("get key form keystore:{}:{}",self.password,kyeystore)
        }
        Ok(())
    }
}