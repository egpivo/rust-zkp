use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use serde::Serialize;
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

#[derive(Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: String,
}

impl IntoResponse for RollupError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            RollupError::AccountNotFound { .. } => (StatusCode::NOT_FOUND, "ACCOUNT_NOT_FOUND"),
            RollupError::InsufficientBalance { .. } => (StatusCode::BAD_REQUEST, "INSUFFICIENT_BALANCE"),
            RollupError::InvalidSignature => (StatusCode::UNAUTHORIZED, "INVALID_SIGNATURE"),
            RollupError::StateRootMismatch => (StatusCode::CONFLICT, "STATE_ROOT_MISMATCH"),
        };

        let body = Json(ErrorResponse {
            code,
            message: self.to_string(),
        });

        (status, body).into_response()
    }
}