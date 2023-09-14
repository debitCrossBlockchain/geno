
use crate::argument::ALG;
use msp::{signing, keystore::KeyStore};
use clap::{Parser, ValueEnum, builder::PossibleValuesParser};
use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};

#[derive(Debug, Parser)]
pub struct Command {
    ///sign data with private key
    #[arg(group = "private_key_types", value_name = "private key", long, short)]
    private_key:Option<String>,
    ///sign data with keystore
    #[arg(group = "private_key_types", value_name = "keystore", long, short)]
    keystore:Option<Vec<String>>,
    ///message
    #[arg(value_name = "messages", long, short)]
    message:String,
    ///algorithm type
    #[clap(name = "algorithm", long, short)]
    algorithm:ALG,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SignatureResult {
    data: String,
    public_key: String,
    sign_data: String,
}

impl Command {
    pub fn run(&self)->Result<()> {
        if let Some(prikey) = &self.private_key{
            println!("sign data with private key:{}:{}",prikey,self.message);
            sign_data(prikey,&self.message,self.algorithm.into())?
        }

        if let Some(keystore) = &self.keystore{
            println!("sign data with keystore:{:?}:{}",keystore,self.message);
            if keystore.len() == 2{
                sign_data_with_keystore(&keystore[0], &keystore[1], &self.message, self.algorithm.into())?
            }else{
                bail!("keystore argument error!")
            }
        }
        Ok(())
    }
}

fn sign_data(node_priv_key:&str, blob_data:&str, algorithm_name:&str)->Result<()>  {
        let priv_key = signing::create_private_key(&algorithm_name, &node_priv_key).unwrap();
        let private_key = priv_key.get_pubkey();
        // let signature = priv_key.sign(blob_data.as_ref());
        let signature = priv_key.sign_data(blob_data.as_ref(), algorithm_name);
        let result = SignatureResult {
            data: blob_data.to_string(),
            public_key: private_key,
            sign_data: signature,
        };
        let result_json = serde_json::to_string(&result).unwrap();
        println!("{}", result_json);
        Ok(())
}

fn sign_data_with_keystore(key_store_json:&str, password:&str, blob_data:&str, algorithm_name:&str)->Result<()>  {

        if password.len() != 6 {
            bail!("Invalid password, please enter a password with a length of 6")
        }

        let mut node_priv_key = String::new();
        let key_store: KeyStore = ron::from_str(key_store_json).unwrap();
        let ret = KeyStore::from(key_store, password, &mut node_priv_key);
        if ret {
            println!("{}", key_store_json);
        } else {
            bail!("error");
        }

        let priv_key = signing::create_private_key(&algorithm_name, &node_priv_key).unwrap();
        let private_key = priv_key.get_pubkey();
        let signature = priv_key.sign_data(blob_data.as_ref(), algorithm_name);
        let result = SignatureResult {
            data: blob_data.to_string(),
            public_key: private_key,
            sign_data: signature,
        };
        let result_json = serde_json::to_string(&result).unwrap();
        println!("{}", result_json);
        Ok(())
}

