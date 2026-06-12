//! Error types for debug utilities

use crate::deno::DenoError;
use thiserror::Error;

/// Errors from debug command execution
#[derive(Debug, Error)]
pub enum DebugError {
    /// Deno sidecar error
    #[error("deno: {0}")]
    Deno(#[from] DenoError),

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

impl DebugError {
    /// Get hint for this error, if any.
    ///
    /// Returns the hint from the underlying [`DenoError`] for
    /// `Deno` variants, empty string otherwise.
    #[must_use]
    pub fn hint(&self) -> &str {
        match self {
            Self::Deno(e) => e.hint(),
            _ => "",
        }
    }
}

/// Result type alias for debug operations
pub type Result<T> = std::result::Result<T, DebugError>;
