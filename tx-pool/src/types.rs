use crate::CoreMempool;
use crate::status::Status;
use crate::tx_pool_config::TxPoolConfig;
use crate::tx_validator::{DiscardedVMStatus, TransactionValidation};
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
pub(crate) struct Shared<V>
where
    V: TransactionValidation + 'static,
{
    pub mempool: Arc<RwLock<CoreMempool>>,
    pub config: configure::TxPoolConfig,
    // pub network_senders: HashMap<NodeNetworkId, MempoolNetworkSender>,
    pub validator: Arc<RwLock<V>>,
    // pub peer_manager: Arc<PeerManager>,
    // pub subscribers: Vec<UnboundedSender<SharedMempoolNotification>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Notification {
    PeerStateChange,
    NewTransactions,
    ACK,
    Broadcast,
}

pub(crate) fn subscribers(
    event: Notification,
    subscribers: &[UnboundedSender<Notification>],
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
pub struct CommitNotification {
    pub transactions: HashMap<String, CommittedTransaction>,
    pub count: u64,
}

#[derive(Debug)]
pub struct CommitResponse {
    pub success: bool,
    /// The error message if `success` is false.
    pub error_message: Option<String>,
}

impl CommitResponse {
    // Returns a new CommitResponse without an error.
    pub fn success() -> Self {
        CommitResponse {
            success: true,
            error_message: None,
        }
    }

    // Returns a new CommitResponse holding the given error message.
    pub fn error(error_message: String) -> Self {
        CommitResponse {
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
pub enum ConsensusRequest {
    /// Request to pull block to submit to consensus.
    GetBlockRequest(
        // max block size
        u64,
        // max contract size
        u64,
        // transactions to exclude from the requested block
        Vec<TransactionSummary>,
        // callback to respond to
        oneshot::Sender<Result<ConsensusResponse>>,
    ),
    /// Notifications about *rejected* committed txns.
    RejectNotification(
        // rejected transactions from consensus
        Vec<TransactionSummary>,
        // callback to respond to
        oneshot::Sender<Result<ConsensusResponse>>,
    ),
}

impl fmt::Display for ConsensusRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let payload = match self {
            ConsensusRequest::GetBlockRequest(
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
            ConsensusRequest::RejectNotification(rejected_txns, _) => {
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
pub enum ConsensusResponse {
    /// Block to submit to consensus
    GetBlockResponse(Vec<TransactionSignRaw>),
    CommitResponse(),
}

pub type SubmissionStatus = (Status, Option<DiscardedVMStatus>);

pub type SubmissionStatusBundle = (TransactionSignRaw, SubmissionStatus);

pub type ClientSender = mpsc::UnboundedSender<(TransactionSignRaw, oneshot::Sender<Result<SubmissionStatus>>)>;
pub type ClientReceiver = mpsc::UnboundedReceiver<(TransactionSignRaw, oneshot::Sender<Result<SubmissionStatus>>)>;
pub type CommitNotificationSender = mpsc::Sender<CommitNotification>;
pub type CommitNotificationReceiver = mpsc::Receiver<CommitNotification>;
pub type BroadCastTxSender = mpsc::UnboundedSender<Vec<TransactionSignRaw>>;
pub type BroadCastTxReceiver = mpsc::UnboundedReceiver<Vec<TransactionSignRaw>>;
