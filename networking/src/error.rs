use std::net::AddrParseError;
use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkingError {
    #[error("IO Error {0:?} for {1:}")]
    IoError(#[source] std::io::Error, String),
    #[error("JSON Error {0:?} for {1:}")]
    UtfError(#[source] FromUtf8Error, String),
    #[error("JSON Error {0:?} for {1:}")]
    JsonError(#[source] serde_json::error::Error, String),
    #[error("Disconnected")]
    Disconnected,
    #[error("Invalid address {0:?} for {1:}")]
    InvalidAddress(#[source] AddrParseError, String),
}
