//! Error types for debug utilities

use thiserror::Error;

/// Errors from debug command execution
#[derive(Debug, Error)]
pub enum DebugError {
    /// Fuz daemon client error
    #[error("fuz daemon: {0}")]
    Client(#[from] fuz_client::ClientError),

    /// IO error (file/process operations)
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// Command execution failed
    #[error("{0}")]
    Command(String),
}

/// Result type alias for debug operations
pub type Result<T> = std::result::Result<T, DebugError>;
