use anyhow::bail;
use protos::ledger::{Ledger, LedgerHeader};
use types::SignedTransaction;
use utils::{signature::verify_sign, TransactionSign};

pub trait Verify {
    fn verify_tx(&self) -> anyhow::Result<bool> {
        bail!("verify tx fail!")
    }
    fn verify_pre_hash(&self, pre_hash: &[u8]) -> anyhow::Result<bool> {
        bail!("verify pre hash fail!")
    }
    fn verify_transactions_hash(&self, hash: &[u8]) -> anyhow::Result<bool> {
        bail!("verify transactions hash fail!")
    }
    fn verify_receips_hash(&self, hash: &[u8]) -> anyhow::Result<bool> {
        bail!("verify receips hash fail!")
    }
    fn verify_state_hash(&self, state_hash: &[u8]) -> anyhow::Result<bool> {
        bail!("verify state hash fail!")
    }
    fn verify_block_hash(&self, block_hash: &[u8]) -> anyhow::Result<bool> {
        bail!("verify block hash fail!")
    }
}

impl Verify for TransactionSign {
    fn verify_tx(&self) -> anyhow::Result<bool> {
        if let Some(signature) = self.get_signatures().get(0) {
            let tx = match SignedTransaction::try_from(self.clone()) {
                Ok(v) => v,
                Err(e) => {
                    bail!("{}", e);
                }
            };
            verify_sign(signature, tx.hash())
        } else {
            bail!("signature");
        }
    }
}

impl Verify for LedgerHeader {
    fn verify_pre_hash(&self, pre_hash: &[u8]) -> anyhow::Result<bool> {
        if self.get_previous_hash().cmp(pre_hash).is_ne() {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn verify_state_hash(&self, state_hash: &[u8]) -> anyhow::Result<bool> {
        if self.get_state_hash().cmp(state_hash).is_ne() {
            Ok(false)
        } else {
            Ok(true)
        }
    }
    fn verify_transactions_hash(&self, hash: &[u8]) -> anyhow::Result<bool> {
        if self.get_transactions_hash().cmp(hash).is_ne() {
            Ok(false)
        } else {
            Ok(true)
        }
    }
    fn verify_receips_hash(&self, hash: &[u8]) -> anyhow::Result<bool> {
        if self.get_receips_hash().cmp(hash).is_ne() {
            Ok(false)
        } else {
            Ok(true)
        }
    }
    fn verify_block_hash(&self, block_hash: &[u8]) -> anyhow::Result<bool> {
        if self.get_hash().cmp(block_hash).is_ne() {
            Ok(false)
        } else {
            Ok(true)
        }
    }
}

impl Verify for Ledger {}
