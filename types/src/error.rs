use thiserror::Error;

#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BlockExecutionError {
    #[error("InternalError({})", error)]
    InternalError { error: String },

    #[error("TransactionParamError({})", error)]
    TransactionParamError { error: String },

    #[error("VmError({})", error)]
    VmError { error: String },

    #[error("StateConvertError({})", error)]
    StateConvertError { error: String },

    #[error("NotExistError({})", error)]
    NotExistError { error: String },
}

#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    #[error("InternalError({})", error)]
    InternalError { error: String },

    #[error("AddressConvert({})", error)]
    AddressConvertError { error: String },

    #[error("ValueConvertError({})", error)]
    ValueConvertError { error: String },

    #[error("VmStateError({})", error)]
    StateError { error: String },

    #[error("VmDatabaseError({})", error)]
    DatabaseError { error: String },

    #[error("VmStorageError({})", error)]
    StorageError { error: String },

    #[error("VM execute transaction ({hash:?}): {message}")]
    VMExecuteError { hash: String, message: String },
}
