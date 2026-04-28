use thiserror::Error;

#[derive(Debug, Error)]
pub enum RollupError {
    #[error("account {id} not found")]
    AccountNotFound { id: u32 },

    #[error("insufficient balance: have {available}, need {requested}")]
    InsufficientBalance { available: u64, requested: u64 },

    #[error("invalid signature")]    
    InvalidSignature,

    #[error("state root mismatch")]    
    StateRootMismatch,
}