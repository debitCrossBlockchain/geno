use serde::{Deserialize, Serialize};
use std::time::Duration;
use types::SignedTransaction;

#[derive(Clone)]
pub struct PoolTransaction {
    pub txn: SignedTransaction,
    // System expiration time of the transaction. It should be removed from mempool by that time.
    pub expiration_time: Duration,
    pub state: TxState,
    pub seq_info: SeqInfo,
}

impl PoolTransaction {
    pub(crate) fn new(
        txn: SignedTransaction,
        expiration_time: Duration,
        state: TxState,
        account_seq: u64,
    ) -> Self {
        let nonce = txn.nonce();
        PoolTransaction {
            txn,
            expiration_time,
            state,
            seq_info: SeqInfo {
                tx_seq: nonce,
                account_seq,
            },
        }
    }

    pub(crate) fn get_state(&self) -> TxState {
        self.state.clone()
    }

    pub(crate) fn get_seq_info(&self) -> SeqInfo {
        self.seq_info.clone()
    }

    pub(crate) fn get_seq(&self) -> u64 {
        self.txn.nonce()
    }
    pub(crate) fn get_sender(&self) -> &str {
        self.txn.sender()
    }
    pub(crate) fn get_gas_price(&self) -> u128 {
        self.txn.gas_price()
    }

    pub(crate) fn get_hash(&self) -> &[u8] {
        self.txn.hash()
    }

    pub(crate) fn get_tx(&self) -> SignedTransaction {
        self.txn.clone()
    }

    pub(crate) fn get_expiration_time(&self) -> Duration {
        self.expiration_time.clone()
    }

    pub(crate) fn is_contract(&self) -> bool {
        if !self.txn.to().is_empty() && !self.txn.input().is_empty() {
            return true;
        }
        false
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Hash, Serialize)]
pub enum TimelineState {
    // The transaction is ready for broadcast.
    // Associated integer represents it's position in the log of such transactions.
    Ready(u64),
    // Transaction is not yet ready for broadcast, but it might change in a future.
    NotReady,
    // Transaction will never be qualified for broadcasting.
    // Currently we don't broadcast transactions originated on other peers.
    NonQualified,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Hash, Serialize)]
pub enum TxState {
    // in consensus
    Pending,
    // sended by network
    Sended,
    // in pool and sorted
    Ready,
    // not sroted
    NotReady,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SeqInfo {
    pub tx_seq: u64,
    pub account_seq: u64,
}
