
use anyhow::{bail, Ok};
use protobuf::Message;
use protos::ledger::{Ledger, LedgerHeader};
use utils::{TransactionSign, general::hash_crypto_byte, signature::verify_sign};


pub trait Verify {
    fn verify_tx(&self)->anyhow::Result<bool>{
        bail!("verify tx fail!") 
    }
    fn verify_pre_hash(&self, pre_hash:&[u8])->anyhow::Result<bool>{
        bail!("verify pre hash fail!") 
    }
    fn verify_validators_hash(&self, validators_hash:&[u8])->anyhow::Result<bool>{
        bail!("verify validators hash fail!") 
    }
    fn verify_state_hash(&self, state_hash:&[u8])->anyhow::Result<bool>{
        bail!("verify state hash fail!") 
    }
    fn verify_block_hash(&self, block_hash:&[u8])->anyhow::Result<bool>{
        bail!("verify block hash fail!") 
    }
}

impl Verify for TransactionSign {
    fn verify_tx(&self) -> anyhow::Result<bool> {
        let signature = if self.get_signatures().len()>0{
            self.get_signatures().get(0).unwrap()
        }else{
            bail!("signature ")
        };

        let txhash = hash_crypto_byte(self.write_to_bytes().unwrap().as_slice());

        verify_sign(signature, &txhash)
    }
}

impl Verify for LedgerHeader{
    fn verify_pre_hash(&self, pre_hash:&[u8])->anyhow::Result<bool>{
        if self.get_previous_hash().cmp(pre_hash).is_ne(){
            Ok(false) 
        }else {
            Ok(true)
        }
    }
    fn verify_validators_hash(&self, validators_hash:&[u8])->anyhow::Result<bool>{
        if self.get_validators_hash().cmp(validators_hash).is_ne(){
            Ok(false) 
        }else {
            Ok(true)
        }
    }
    fn verify_state_hash(&self, state_hash:&[u8])->anyhow::Result<bool>{
        if self.get_state_hash().cmp(state_hash).is_ne(){
            Ok(false) 
        }else {
            Ok(true)
        }
    }
    fn verify_block_hash(&self, block_hash:&[u8])->anyhow::Result<bool>{
        if self.get_hash().cmp(block_hash).is_ne(){
            Ok(false) 
        }else {
            Ok(true)
        }
    }
}

impl Verify for Ledger{

}