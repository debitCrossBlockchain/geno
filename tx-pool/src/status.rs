use anyhow::Result;
use std::{convert::TryFrom, fmt};

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

    UnknownStatus = 7,
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
            7 => Ok(StatusCode::UnknownStatus),
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
