//! Error types for Deno sidecar communication

use std::io;
use thiserror::Error;

/// Errors that can occur when communicating with the Deno sidecar
#[derive(Debug, Error)]
pub enum DenoError {
    /// Failed to create tempfile for sidecar script
    #[error("failed to create tempfile for sidecar script: {0}")]
    TempfileCreate(#[source] io::Error),

    /// Failed to write sidecar script to tempfile
    #[error("failed to write sidecar script: {0}")]
    ScriptWrite(#[source] io::Error),

    /// Failed to spawn Deno process
    #[error("failed to spawn deno: {0}")]
    ProcessSpawn(#[source] io::Error),

    /// Deno not found in PATH
    #[error("deno not found in PATH - install from https://deno.land")]
    DenoNotFound,

    /// Failed to get stdio pipe
    #[error("failed to get {pipe} pipe from deno process")]
    PipeMissing { pipe: &'static str },

    /// Communication error with sidecar
    #[error("communication error: {0}")]
    Communication(#[source] io::Error),

    /// Failed to parse response from sidecar
    #[error("failed to parse sidecar response: {0}")]
    ResponseParse(#[source] serde_json::Error),

    /// Sidecar returned an error
    #[error("tool error: {message}")]
    ToolError { message: String },

    /// Sidecar response missing output
    #[error("sidecar response missing output")]
    MissingOutput,

    /// Prettier returned empty output for non-empty input — the under-load
    /// miss, never a real format result
    #[error("prettier returned empty output for non-empty input (sidecar miss)")]
    EmptyOutput,

    /// Sidecar process crashed
    #[error("deno sidecar process crashed")]
    SidecarCrashed,

    /// Actor shutdown
    #[error("deno actor shut down")]
    ActorShutdown,

    /// Request timed out
    #[error("deno sidecar timed out after {seconds}s")]
    Timeout { seconds: u64 },
}

impl DenoError {
    /// Get a hint for how to fix this error
    #[must_use]
    pub fn hint(&self) -> &'static str {
        match self {
            Self::DenoNotFound => "Install Deno: curl -fsSL https://deno.land/install.sh | sh",
            Self::ProcessSpawn(_) => "Check that 'deno' is in your PATH",
            Self::SidecarCrashed => "This may be a bug in the sidecar script",
            Self::Timeout { .. } => "The sidecar may be stuck processing a request",
            Self::EmptyOutput => {
                "Transient under load — re-run; if it persists, check the sidecar (tsv_debug check)"
            }
            _ => "",
        }
    }
}
