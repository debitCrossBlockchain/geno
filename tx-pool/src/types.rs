use crate::pool::Pool;
use crate::status::Status;
use anyhow::Result;
use futures::{
    channel::{mpsc, mpsc::UnboundedSender, oneshot},
    future::Future,
    task::{Context, Poll},
};
use parking_lot::RwLock;
use std::{collections::HashMap, fmt, sync::Arc};
use types::TransactionSignRaw;

use crate::verify_pool::*;
use msp::signing::{create_context, create_public_key_by_bytes};

pub trait Validation: Send + Sync + Clone {
    /// Validate a txn from client
    fn validate(&self, _txn: &TransactionSignRaw) -> Result<ValidatorResult>;
}

#[derive(Clone)]
pub struct Validator;

impl Validator {
    pub fn new() -> Validator {
        Validator {}
    }
}

impl Validation for Validator {
    fn validate(&self, txn: &TransactionSignRaw) -> Result<ValidatorResult> {
        let txn_sender = txn.signatures.clone();
        for signature in txn_sender {
            // if already verify in jsonrpc,skip this verify
            if verify_pool_exist(txn.hash()) {
                continue;
            }
            let ctx = create_context(signature.get_encryption_type()).unwrap();

            let pub_key = create_public_key_by_bytes(
                signature.get_encryption_type(),
                signature.get_public_key(),
            );
            if pub_key.is_err() {
                return Ok(ValidatorResult::new(Some(StatusCode::INVALID_SIGNATURE), 0));
            }
            let result = ctx.verify(signature.get_sign_data(), txn.hash(), &*pub_key.unwrap());
            if result.is_err() {
                return Ok(ValidatorResult::new(Some(StatusCode::INVALID_SIGNATURE), 0));
            }
            if !result.unwrap() {
                return Ok(ValidatorResult::new(Some(StatusCode::INVALID_SIGNATURE), 0));
            }

            // insert tx verify pool
            verify_pool_set(txn.hash());
        }

        Ok(ValidatorResult::new(None, txn.gas_price()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatorResult {
    /// Result of the validation: `None` if the transaction was successfully validated
    /// or `Some(DiscardedVMStatus)` if the transaction should be discarded.
    status: Option<VMStatus>,

    /// Score for ranking the transaction priority (e.g., based on the gas price).
    /// Only used when the status is `None`. Higher values indicate a higher priority.
    score: u128,
}

#[repr(u64)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum StatusCode {
    // The status of a transaction as determined by the prologue.
    // Validation Errors: 0-999
    // We don't want the default value to be valid
    UNKNOWN_VALIDATION_STATUS = 0,
    // The transaction has a bad signature
    INVALID_SIGNATURE = 1,
    // Bad account authentication key
    INVALID_AUTH_KEY = 2,
    // Sequence number is too old
    SEQUENCE_NUMBER_TOO_OLD = 3,
    // Sequence number is too new
    SEQUENCE_NUMBER_TOO_NEW = 4,
    // Insufficient balance to pay minimum transaction fee
    INSUFFICIENT_BALANCE_FOR_TRANSACTION_FEE = 5,
    // The transaction has expired
    TRANSACTION_EXPIRED = 6,
    // The sending account does not exist
    SENDING_ACCOUNT_DOES_NOT_EXIST = 7,

    CDI_ERROR = 8,

    RESOURCE_DOES_NOT_EXIST = 4003,
    // this is std::u64::MAX, but we can't pattern match on that, so put the hardcoded value in
    UNKNOWN_STATUS = 18446744073709551615,
}

pub type VMStatus = StatusCode;

impl ValidatorResult {
    pub fn new(vm_status: Option<VMStatus>, score: u128) -> Self {
        Self {
            status: vm_status,
            score,
        }
    }

    pub fn status(&self) -> Option<VMStatus> {
        self.status
    }

    pub fn score(&self) -> u128 {
        self.score
    }
}


/// Struct that owns all dependencies required by shared mempool routines.
#[derive(Clone)]
pub(crate) struct Shared<V>
where
    V: Validation + 'static,
{
    pub mempool: Arc<RwLock<Pool>>,
    pub config: configure::TxPoolConfig,
    pub validator: Arc<RwLock<V>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Notification {
    PeerStateChange,
    NewTransactions,
    ACK,
    Broadcast,
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

/// Response sent from mempool to consensus.
pub enum ConsensusResponse {
    /// Block to submit to consensus
    GetBlockResponse(Vec<TransactionSignRaw>),
    CommitResponse(),
}

pub type SubmissionStatus = (Status, Option<VMStatus>);
pub type SubmissionStatusBundle = (TransactionSignRaw, SubmissionStatus);
pub type ClientSender = mpsc::UnboundedSender<(
    TransactionSignRaw,
    oneshot::Sender<Result<SubmissionStatus>>,
)>;
pub type ClientReceiver = mpsc::UnboundedReceiver<(
    TransactionSignRaw,
    oneshot::Sender<Result<SubmissionStatus>>,
)>;
pub type CommitNotificationSender = mpsc::Sender<CommitNotification>;
pub type CommitNotificationReceiver = mpsc::Receiver<CommitNotification>;
pub type BroadCastTxSender = mpsc::UnboundedSender<Vec<TransactionSignRaw>>;
pub type BroadCastTxReceiver = mpsc::UnboundedReceiver<Vec<TransactionSignRaw>>;

pub fn get_account_nonce_banace(_account_address: &str) -> Result<(u64, u64)> {
    // for i in 0..3 {
    //     let last_state = { LastLedgerStateRef.read().get() };
    //     if let Some((nonce, balance)) =
    //         state::reading_trie_get_nonce_banace(account_address, &last_state.get_tire_hash())
    //     {
    //         return Ok((nonce, balance));
    //     }
    // }
    Err(anyhow::anyhow!("get_account_nonce_banace failed"))
}