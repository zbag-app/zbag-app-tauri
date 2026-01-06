#![allow(dead_code)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Wallet not found: {0}")]
    WalletNotFound(String),

    #[error("Invalid password")]
    InvalidPassword,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, TuiError>;
