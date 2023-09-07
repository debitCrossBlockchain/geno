use crate::tx_verify_pool::*;
use msp::signing::{create_context, create_public_key_by_bytes, Context};
use protobuf::Message;
use serde::{de, ser, Deserialize, Serialize};
use types::TransactionSignRaw;
use std::{convert::TryFrom, fmt};
use anyhow::{Error, Result};


pub static VALIDATION_STATUS_MIN_CODE: u64 = 0;

/// The maximum status code for validation statuses
pub static VALIDATION_STATUS_MAX_CODE: u64 = 999;

/// The minimum status code for verification statuses
pub static VERIFICATION_STATUS_MIN_CODE: u64 = 1000;

/// The maximum status code for verification statuses
pub static VERIFICATION_STATUS_MAX_CODE: u64 = 1999;

/// The minimum status code for invariant violation statuses
pub static INVARIANT_VIOLATION_STATUS_MIN_CODE: u64 = 2000;

/// The maximum status code for invariant violation statuses
pub static INVARIANT_VIOLATION_STATUS_MAX_CODE: u64 = 2999;

/// The minimum status code for deserialization statuses
pub static DESERIALIZATION_STATUS_MIN_CODE: u64 = 3000;

/// The maximum status code for deserialization statuses
pub static DESERIALIZATION_STATUS_MAX_CODE: u64 = 3999;

/// The minimum status code for runtime statuses
pub static EXECUTION_STATUS_MIN_CODE: u64 = 4000;

/// The maximum status code for runtim statuses
pub static EXECUTION_STATUS_MAX_CODE: u64 = 4999;

pub trait TransactionValidation: Send + Sync + Clone {
    /// Validate a txn from client
    fn validate_transaction(&self, _txn: &TransactionSignRaw) -> Result<VMValidatorResult>;

    //fn restart(&mut self, config: OnChainConfigPayload) -> Result<()>;
}

#[derive(Clone)]
pub struct TxValidator {}

impl TxValidator {
    pub fn new() -> TxValidator {
        TxValidator {}
    }
}

impl TransactionValidation for TxValidator {
    fn validate_transaction(&self, txn: &TransactionSignRaw) -> Result<VMValidatorResult> {
        let txn_sender = txn.signatures.clone();
        for signature in txn_sender {
            // if already verify in jsonrpc,skip this verify
            if tx_verify_pool_exist(txn.tx.hash()) {
                continue;
            }
            let ctx = create_context(signature.get_encryption_type()).unwrap();

            let pub_key = create_public_key_by_bytes(
                signature.get_encryption_type(),
                signature.get_public_key(),
            );
            if pub_key.is_err() {
                return Ok(VMValidatorResult::new(
                    Some(StatusCode::INVALID_SIGNATURE),
                    0,
                ));
            }
            let result = ctx.verify(signature.get_sign_data(), txn.tx.hash(), &*pub_key.unwrap());
            if result.is_err() {
                return Ok(VMValidatorResult::new(
                    Some(StatusCode::INVALID_SIGNATURE),
                    0,
                ));
            }
            if !result.unwrap() {
                return Ok(VMValidatorResult::new(
                    Some(StatusCode::INVALID_SIGNATURE),
                    0,
                ));
            }

            // insert tx verify pool
            tx_verify_pool_set(txn.tx.hash());
        }

        Ok(VMValidatorResult::new(None, txn.tx.gas_price()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VMValidatorResult {
    /// Result of the validation: `None` if the transaction was successfully validated
    /// or `Some(DiscardedVMStatus)` if the transaction should be discarded.
    status: Option<DiscardedVMStatus>,

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

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum StatusType {
    Validation,
    Verification,
    InvariantViolation,
    Deserialization,
    Execution,
    Unknown,
}

impl fmt::Display for StatusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            StatusType::Validation => "Validation",
            StatusType::Verification => "Verification",
            StatusType::InvariantViolation => "Invariant violation",
            StatusType::Deserialization => "Deserialization",
            StatusType::Execution => "Execution",
            StatusType::Unknown => "Unknown",
        };
        write!(f, "{}", string)
    }
}

impl StatusCode {
    /// Return the status type for this status code
    pub fn status_type(self) -> StatusType {
        let major_status_number: u64 = self.into();
        if major_status_number >= VALIDATION_STATUS_MIN_CODE
            && major_status_number <= VALIDATION_STATUS_MAX_CODE
        {
            return StatusType::Validation;
        }

        if major_status_number >= VERIFICATION_STATUS_MIN_CODE
            && major_status_number <= VERIFICATION_STATUS_MAX_CODE
        {
            return StatusType::Verification;
        }

        StatusType::Unknown
    }
}

impl ser::Serialize for StatusCode {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_u64((*self).into())
    }
}

impl<'de> de::Deserialize<'de> for StatusCode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct StatusCodeVisitor;
        impl<'de> de::Visitor<'de> for StatusCodeVisitor {
            type Value = StatusCode;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("StatusCode as u64")
            }

            // fn visit_u64<E>(self, v: u64) -> std::result::Result<StatusCode, E>
            //     where
            //         E: de::Error,
            // {
            //     Ok(StatusCode::try_from(v).unwrap_or(StatusCode::UNKNOWN_STATUS))
            // }
        }

        deserializer.deserialize_u64(StatusCodeVisitor)
    }
}

impl From<StatusCode> for u64 {
    fn from(status: StatusCode) -> u64 {
        status as u64
    }
}

pub type DiscardedVMStatus = StatusCode;

impl VMValidatorResult {
    pub fn new(vm_status: Option<DiscardedVMStatus>, score: u128) -> Self {
        // debug_assert!(
        //     match vm_status {
        //         None => true,
        //         Some(status) =>
        //             status.status_type() == StatusType::Unknown
        //                 || status.status_type() == StatusType::Validation
        //                 || status.status_type() == StatusType::InvariantViolation,
        //     },
        //     "Unexpected discarded status: {:?}",
        //     vm_status
        // );
        Self {
            status: vm_status,
            score,
        }
    }

    pub fn status(&self) -> Option<DiscardedVMStatus> {
        self.status
    }

    pub fn score(&self) -> u128 {
        self.score
    }
}

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
