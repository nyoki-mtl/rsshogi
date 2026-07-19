use crate::board::SfenError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KifError {
    #[error("failed to parse SFEN: {0}")]
    Sfen(#[from] SfenError),
    #[error("invalid KIF line: {0}")]
    InvalidLine(String),
    #[error("invalid KIF move: {0}")]
    InvalidMove(String),
    #[error("illegal move at index {index}")]
    IllegalMove { index: usize },
    #[error("missing KIF result")]
    MissingResult,
}

#[derive(Debug, Error)]
pub enum Ki2Error {
    #[error("failed to parse SFEN: {0}")]
    Sfen(#[from] SfenError),
    #[error("invalid KI2 line: {0}")]
    InvalidLine(String),
    #[error("invalid KI2 move: {0}")]
    InvalidMove(String),
    #[error("illegal move at index {index}")]
    IllegalMove { index: usize },
}
