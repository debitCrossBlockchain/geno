use crate::pool::Pool;
use anyhow::Result;
use futures::channel::{mpsc, oneshot};
use parking_lot::RwLock;
use std::{collections::HashMap, convert::TryFrom, fmt, sync::Arc};
use types::SignedTransaction;

use crate::verify_pool::*;
use msp::signing::{create_context, create_public_key_by_bytes};

/// A `Status` is represented as a required status code that is semantic coupled with an optional sub status and message.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Status {
    /// insertion status code
    pub code: StatusCode,
    /// optional message
    pub message: String,
}

impl Status {
    pub fn new(code: StatusCode) -> Self {
        Self {
            code,
            message: "".to_string(),
        }
    }

    /// Adds a message to the  status.
    pub fn with_message(mut self, message: String) -> Self {
        self.message = message;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
// #[cfg_attr(any(test, feature = "fuzzing"), derive(Arbitrary))]
#[repr(u64)]
pub enum StatusCode {
    // Transaction was accepted by
    Accepted = 0,
    // Sequence number is old, etc.
    InvalidSeqNumber = 1,
    //  is full (reached max global capacity)
    IsFull = 2,
    // Account reached max capacity per account
    TooManyTransactions = 3,
    // Invalid update. Only gas price increase is allowed
    InvalidUpdate = 4,
    // transaction didn't pass vm_validation
    VmError = 5,

    Pending = 6,

    // The transaction has a bad signature
    InvalidSignature = 7,
    // Bad account authentication key
    InvalidAuthKey = 8,
    // Sequence number is too old
    SeqTooOld = 9,
    // Sequence number is too new
    SeqTooNew = 10,
    // Insufficient balance to pay minimum transaction fee
    InsufficientBalanceFee = 11,
    // The transaction has expired
    TransactionExpired = 12,
    // The sending account does not exist
    AccountDoesNotExist = 13,
    ResourceDoesNotExist = 14,
    UnknownStatus = 15,
}

impl TryFrom<u64> for StatusCode {
    type Error = &'static str;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(StatusCode::Accepted),
            1 => Ok(StatusCode::InvalidSeqNumber),
            2 => Ok(StatusCode::IsFull),
            3 => Ok(StatusCode::TooManyTransactions),
            4 => Ok(StatusCode::InvalidUpdate),
            5 => Ok(StatusCode::VmError),
            6 => Ok(StatusCode::Pending),
            7 => Ok(StatusCode::InvalidSignature),
            8 => Ok(StatusCode::InvalidAuthKey),
            9 => Ok(StatusCode::SeqTooOld),
            10 => Ok(StatusCode::SeqTooNew),
            11 => Ok(StatusCode::InsufficientBalanceFee),
            12 => Ok(StatusCode::TransactionExpired),
            13 => Ok(StatusCode::AccountDoesNotExist),
            14 => Ok(StatusCode::ResourceDoesNotExist),
            15 => Ok(StatusCode::UnknownStatus),
            _ => Err("invalid StatusCode"),
        }
    }
}

impl From<StatusCode> for u64 {
    fn from(status: StatusCode) -> u64 {
        status as u64
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait Validation: Send + Sync + Clone {
    /// Validate a txn from client
    fn validate(&self, _txn: &SignedTransaction) -> Result<ValidatorResult>;
}

#[derive(Clone)]
pub struct Validator;

impl Validator {
    pub fn new() -> Validator {
        Validator {}
    }
}

impl Validation for Validator {
    fn validate(&self, txn: &SignedTransaction) -> Result<ValidatorResult> {
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
                return Ok(ValidatorResult::new(Some(StatusCode::InvalidSignature), 0));
            }
            let result = ctx.verify(signature.get_sign_data(), txn.hash(), &*pub_key.unwrap());
            if result.is_err() {
                return Ok(ValidatorResult::new(Some(StatusCode::InvalidSignature), 0));
            }
            if !result.unwrap() {
                return Ok(ValidatorResult::new(Some(StatusCode::InvalidSignature), 0));
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
    /// or `Some(DiscardedStatusCode)` if the transaction should be discarded.
    status: Option<StatusCode>,

    /// Score for ranking the transaction priority (e.g., based on the gas price).
    /// Only used when the status is `None`. Higher values indicate a higher priority.
    score: u128,
}

impl ValidatorResult {
    pub fn new(vm_status: Option<StatusCode>, score: u128) -> Self {
        Self {
            status: vm_status,
            score,
        }
    }

    pub fn status(&self) -> Option<StatusCode> {
        self.status
    }

    pub fn score(&self) -> u128 {
        self.score
    }
}

/// Struct that owns all dependencies required by shared pool routines.
#[derive(Clone)]
pub(crate) struct Shared<V>
where
    V: Validation + 'static,
{
    pub pool: Arc<RwLock<Pool>>,
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
pub struct Committed {
    pub sender: String,
    pub max_seq: u64,
    pub seqs: Vec<u64>,
}
/// Notification from state sync to pool of commit event.
/// This notifies pool to remove committed txns.
pub struct CommitNotification {
    pub transactions: HashMap<String, Committed>,
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

/// Message sent from consensus to pool.
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

/// Response sent from pool to consensus.
pub enum ConsensusResponse {
    /// Block to submit to consensus
    GetBlockResponse(Vec<SignedTransaction>),
    CommitResponse(),
}

pub type SubmissionStatus = (Status, Option<StatusCode>);
pub type SubmissionStatusBundle = (SignedTransaction, SubmissionStatus);
pub type ClientSender =
    mpsc::UnboundedSender<(SignedTransaction, oneshot::Sender<Result<SubmissionStatus>>)>;
pub type ClientReceiver =
    mpsc::UnboundedReceiver<(SignedTransaction, oneshot::Sender<Result<SubmissionStatus>>)>;
pub type CommitNotificationSender = mpsc::Sender<CommitNotification>;
pub type CommitNotificationReceiver = mpsc::Receiver<CommitNotification>;
pub type BroadcastSender = mpsc::UnboundedSender<Vec<SignedTransaction>>;
pub type BroadcastReceiver = mpsc::UnboundedReceiver<Vec<SignedTransaction>>;

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
