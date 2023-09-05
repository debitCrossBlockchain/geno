/*
 * @Author: your name
 * @Date: 2021-12-08 09:08:12
 * @LastEditTime: 2021-12-21 08:25:53
 * @LastEditors: Please set LastEditors
 * @Description: 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
 * @FilePath: /chain-concordium/tx-pool/src/core_mempool/transaction.rs
 */
// Copyright (c) The  Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::shared_mempool::account_address::AccountAddress;
use parking_lot::RwLock;
use protobuf::Message;
use types::TransactionSignRaw;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, ops::Deref, rc::Rc, sync::Arc, time::Duration};

use utils::timing::Timestamp;
#[derive(Clone)]
pub struct MempoolTransaction {
    pub txn: TransactionSignRaw,
    // System expiration time of the transaction. It should be removed from mempool by that time.
    pub expiration_time: Duration,
    pub state: TxState,
    pub sequence_info: SequenceInfo,
}

impl MempoolTransaction {
    pub(crate) fn new(
        txn: TransactionSignRaw,
        expiration_time: Duration,
        state: TxState,
        account_seqno: u64,
    ) -> Self {
        let nonce = txn.tx.nonce();
        MempoolTransaction {
            txn,
            expiration_time,
            state,
            sequence_info: SequenceInfo {
                transaction_sequence_number: nonce,
                account_sequence_number: account_seqno,
            },
        }
    }

    pub(crate) fn get_state(&self) -> TxState {
        self.state.clone()
    }

    pub(crate) fn get_sequence_info(&self) -> SequenceInfo {
        self.sequence_info.clone()
    }

    pub(crate) fn get_sequence_number(&self) -> u64 {
        self.txn.tx.nonce()
    }
    pub(crate) fn get_sender(&self) -> &str {
        self.txn.tx.sender()
    }
    pub(crate) fn get_gas_price(&self) -> u128 {
        self.txn.tx.gas_price()
    }

    pub(crate) fn get_hash(&self) -> &[u8] {
        self.txn.tx.hash()
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
    pub transaction_sequence_number: u64,
    pub account_sequence_number: u64,
}
