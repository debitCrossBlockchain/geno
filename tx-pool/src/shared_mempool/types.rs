// Copyright (c) The  Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Objects used by/related to shared mempool

use crate::core_mempool::CoreMempool;
use crate::shared_mempool::account_address::AccountAddress;
use crate::shared_mempool::mempool_status::MempoolStatus;
use crate::shared_mempool::temp_db::DbReader;
use crate::shared_mempool::tx_pool_config::TxPoolConfig;
use crate::shared_mempool::tx_validator::{DiscardedVMStatus, TransactionValidation};
use anyhow::Result;
use futures::{
    channel::{mpsc, mpsc::UnboundedSender, oneshot},
    future::Future,
    task::{Context, Poll},
};
use parking_lot::{Mutex, Once, RawRwLock, RwLock};
use protos::ledger::TransactionSign;
use types::TransactionSignRaw;
use std::{collections::HashMap, fmt, pin::Pin, sync::Arc, task::Waker, time::Instant};
use tokio::runtime::Handle;
/// Struct that owns all dependencies required by shared mempool routines.
#[derive(Clone)]
pub(crate) struct SharedMempool<V>
where
    V: TransactionValidation + 'static,
{
    pub mempool: Arc<RwLock<CoreMempool>>,
    pub config: configure::TxPoolConfig,
    // pub network_senders: HashMap<NodeNetworkId, MempoolNetworkSender>,
    pub db: Arc<dyn DbReader>,
    pub validator: Arc<RwLock<V>>,
    // pub peer_manager: Arc<PeerManager>,
    // pub subscribers: Vec<UnboundedSender<SharedMempoolNotification>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SharedMempoolNotification {
    PeerStateChange,
    NewTransactions,
    ACK,
    Broadcast,
}

pub(crate) fn notify_subscribers(
    event: SharedMempoolNotification,
    subscribers: &[UnboundedSender<SharedMempoolNotification>],
) {
    for subscriber in subscribers {
        let _ = subscriber.unbounded_send(event);
    }
}

#[derive(Clone)]
pub struct CommittedTransaction {
    pub sender: String,
    pub max_seq: u64,
    pub seqs: Vec<u64>,
}
/// Notification from state sync to mempool of commit event.
/// This notifies mempool to remove committed txns.
pub struct MempoolCommitNotification {
    pub transactions: HashMap<String, CommittedTransaction>,
    pub count: u64,
}

#[derive(Debug)]
pub struct MempoolCommitResponse {
    pub success: bool,
    /// The error message if `success` is false.
    pub error_message: Option<String>,
}

impl MempoolCommitResponse {
    // Returns a new MempoolCommitResponse without an error.
    pub fn success() -> Self {
        MempoolCommitResponse {
            success: true,
            error_message: None,
        }
    }

    // Returns a new MempoolCommitResponse holding the given error message.
    pub fn error(error_message: String) -> Self {
        MempoolCommitResponse {
            success: false,
            error_message: Some(error_message),
        }
    }
}

#[derive(Clone)]
pub struct TransactionSummary {
    pub sender: String,
    pub sequence_number: u64,
}

impl fmt::Display for TransactionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.sender, self.sequence_number,)
    }
}

/// Message sent from consensus to mempool.
pub enum MempoolConsensusRequest {
    /// Request to pull block to submit to consensus.
    GetBlockRequest(
        // max block size
        u64,
        // max contract size
        u64,
        // transactions to exclude from the requested block
        Vec<TransactionSummary>,
        // callback to respond to
        oneshot::Sender<Result<MempoolConsensusResponse>>,
    ),
    /// Notifications about *rejected* committed txns.
    RejectNotification(
        // rejected transactions from consensus
        Vec<TransactionSummary>,
        // callback to respond to
        oneshot::Sender<Result<MempoolConsensusResponse>>,
    ),
}

impl fmt::Display for MempoolConsensusRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let payload = match self {
            MempoolConsensusRequest::GetBlockRequest(
                block_size,
                contact_size,
                excluded_txns,
                _,
            ) => {
                let mut txns_str = "".to_string();
                for tx in excluded_txns.iter() {
                    txns_str += &format!("{} ", tx);
                }
                format!(
                    "GetBlockRequest [block_size: {}, excluded_txns: {}]",
                    block_size, txns_str
                )
            }
            MempoolConsensusRequest::RejectNotification(rejected_txns, _) => {
                let mut txns_str = "".to_string();
                for tx in rejected_txns.iter() {
                    txns_str += &format!("{} ", tx);
                }
                format!("RejectNotification [rejected_txns: {}]", txns_str)
            }
        };
        write!(f, "{}", payload)
    }
}

/// Response sent from mempool to consensus.
pub enum MempoolConsensusResponse {
    /// Block to submit to consensus
    GetBlockResponse(Vec<TransactionSignRaw>),
    CommitResponse(),
}

pub type SubmissionStatus = (MempoolStatus, Option<DiscardedVMStatus>);

pub type SubmissionStatusBundle = (TransactionSignRaw, SubmissionStatus);

pub type MempoolClientSender =
    mpsc::UnboundedSender<(TransactionSignRaw, oneshot::Sender<Result<SubmissionStatus>>)>;

pub type MempoolClientReceiver =
    mpsc::UnboundedReceiver<(TransactionSignRaw, oneshot::Sender<Result<SubmissionStatus>>)>;

pub type MempoolConsensusSender = mpsc::Sender<MempoolConsensusRequest>;
pub type MempoolConsensusReceiver = mpsc::Receiver<MempoolConsensusRequest>;

pub type MempoolCommitNotificationSender = mpsc::Sender<MempoolCommitNotification>;
pub type MempoolCommitNotificationReceiver = mpsc::Receiver<MempoolCommitNotification>;

pub type MempoolBroadCastTxSender = mpsc::UnboundedSender<Vec<TransactionSignRaw>>;
pub type MempoolBroadCastTxReceiver = mpsc::UnboundedReceiver<Vec<TransactionSignRaw>>;