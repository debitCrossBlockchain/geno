
use std::{fs::OpenOptions, io::Write};
use msp::{signing, keystore::KeyStore};
use clap::Parser;
use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use crate::argument::ALG;

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
    algorithm:Option<ALG>,
}

impl Command {
    pub fn run(&self)->Result<()> {

        if let Some(alg) = &self.algorithm{
            if let Some(priky) = &self.private_key{
                println!("create keystore from private key:{:?}:{}:{}",alg,priky,self.password);
                create_keystore_from_private_key(priky, (*alg).into(), &self.password)?
            }else{
                println!("create keystore:{:?}:{}",alg,self.password);
                create_keystore((*alg).into(), &self.password)?
            }
        };

        //get private key
        if let Some(kyeystore) = &self.keystore {
            println!("get key form keystore:{}:{}",self.password,kyeystore);
            get_private_key_from_keystore(kyeystore, &self.password)?
        }
        Ok(())
    }
}

fn create_keystore(algorithm_name:&str, password:&str)->Result<()> {

        if password.len() != 6 {
            bail!("Invalid password, please enter a password with a length of 6")
        }
        let new_priv_key = signing::create_secret_key(&algorithm_name).unwrap();
        let mut key_store = KeyStore::new();
        let ret =
            KeyStore::generate(&password, &mut key_store, &new_priv_key, &*algorithm_name);
        let mut file = OpenOptions::new()
            .append(true)
            .read(true)
            .write(true)
            .create(true)
            .open("key_store.txt")
            .expect("open failed");
        file.write("\n".as_ref());
        ron::ser::to_writer(file, &key_store).unwrap();
        let key_store_json = serde_json::to_string(&key_store).unwrap();
        if ret {
            println!("{:?}", key_store_json);
            return Ok(());
        } else {
            bail!("create keystore failed")
        }
}


fn create_keystore_from_private_key(private_key:&str, algorithm_name:&str, password:&str)->Result<()> {
        if password.len() != 6 {
            bail!("Invalid password, please enter a password with a length of 6")
        }
        let new_priv_key = signing::create_private_key(&algorithm_name, private_key).unwrap();
        let mut key_store = KeyStore::new();
        let ret =
            KeyStore::generate(&password, &mut key_store, &new_priv_key, &*algorithm_name);
        let key_store_json = serde_json::to_string(&key_store).unwrap();
        if ret {
            println!("{:?}", key_store_json);
            return Ok(());
        } else {
            bail!("create keystore failed");
        }
}


fn get_private_key_from_keystore(key_store_json:&str, password:&str)->Result<()> {
        if password.len() != 6 {
            bail!("Invalid password, please enter a password with a length of 6")
        }
        let mut private_key = String::new();
        let key_store: KeyStore = serde_json::from_str(key_store_json).unwrap();
        let ret = KeyStore::from(key_store, password, &mut private_key);
        if ret {
            println!("{}", private_key);
            return Ok(());
        } else {
            bail!("get private key failed");
        }
}

