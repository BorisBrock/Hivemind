use serde::Serialize;

use crate::models::TicketStatus;

/// Errors returned to the frontend. Serialized as `{ kind, message? | from/to }`.
#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    #[error("database error: {message}")]
    Database { message: String },
    #[error("not found")]
    NotFound,
    #[error("invalid input: {message}")]
    Invalid { message: String },
    #[error("cannot move ticket from {from:?} to {to:?}")]
    InvalidTransition { from: TicketStatus, to: TicketStatus },
    #[error("the ticket can only be edited while in planning")]
    NotEditable,
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            e => AppError::Database { message: e.to_string() },
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Database { message: e.to_string() }
    }
}

pub type AppResult<T> = Result<T, AppError>;

pub fn invalid(message: impl Into<String>) -> AppError {
    AppError::Invalid { message: message.into() }
}
