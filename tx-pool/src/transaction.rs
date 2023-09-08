use serde::{Deserialize, Serialize};
use std::time::Duration;
use types::TransactionSignRaw;

#[derive(Clone)]
pub struct PoolTransaction {
    pub txn: TransactionSignRaw,
    // System expiration time of the transaction. It should be removed from mempool by that time.
    pub expiration_time: Duration,
    pub state: TxState,
    pub sequence_info: SequenceInfo,
}

impl PoolTransaction {
    pub(crate) fn new(
        txn: TransactionSignRaw,
        expiration_time: Duration,
        state: TxState,
        account_seqno: u64,
    ) -> Self {
        let nonce = txn.nonce();
        PoolTransaction {
            txn,
            expiration_time,
            state,
            sequence_info: SequenceInfo {
                seq: nonce,
                account_seq: account_seqno,
            },
        }
    }

    pub(crate) fn get_state(&self) -> TxState {
        self.state.clone()
    }

    pub(crate) fn get_sequence_info(&self) -> SequenceInfo {
        self.sequence_info.clone()
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

    pub(crate) fn get_tx(&self) -> TransactionSignRaw {
        self.txn.clone()
    }

    pub(crate) fn get_expiration_time(&self) -> Duration {
        self.expiration_time.clone()
    }

    pub(crate) fn is_contract(&self) -> bool {
        // if let Some(k) = self.txn.get_transaction().get_tx_type().get(0) {
        //     if k.get_ktype() == protos::ledger::Kind_KindType::CREATE_ACCOUNT {
        //         if k.get_create_account().has_contract() {
        //             if k.get_create_account().get_contract().get_payload().len() > 0 {
        //                 return true;
        //             }
        //         }
        //     }
        //     if k.get_ktype() == protos::ledger::Kind_KindType::PAY_COIN {
        //         if k.get_pay_coin().has_input() {
        //             return true;
        //         }
        //     }
        // }
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
pub struct SequenceInfo {
    pub seq: u64,
    pub account_seq: u64,
}
