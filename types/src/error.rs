use thiserror::Error;

#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BlockExecutionError {
    #[error("Internal error: {:?}", error)]
    InternalError { error: String },

    #[error("TransactionParamError error: {:?}", error)]
    TransactionParamError { error: String },

    #[error("VmError error: {:?}", error)]
    VmEexecError { error: String },

    #[error("StateConvertError error: {:?}", error)]
    StateConvertError { error: String },
}

#[allow(missing_docs)]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    #[error("Internal error: {:?}", error)]
    InternalError { error: String },

    #[error("AddressConvert error: {:?}", error)]
    AddressConvertError { error: String },

    #[error("ValueConvertError error: {:?}", error)]
    ValueConvertError { error: String },

    #[error("VmStateError error: {:?}", error)]
    StateError { error: String },

    #[error("VmDatabaseError error: {:?}", error)]
    DatabaseError { error: String },

    #[error("VmStorageError error: {:?}", error)]
    StorageError { error: String },

    #[error("VM execute transaction ({hash:?}): {message}")]
    VMExecuteError { hash: String, message: String },
}
