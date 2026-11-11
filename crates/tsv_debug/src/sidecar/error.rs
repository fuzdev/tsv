//! Error types for sidecar communication

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// The install command the install errors name, run from the checkout's root.
const INSTALL_HINT: &str = "Install the sidecar's packages from the checkout's root: npm ci --prefix crates/tsv_debug/sidecar";

/// Errors that can occur when communicating with the sidecar
#[derive(Debug, Error)]
pub enum SidecarError {
    /// The sidecar's package directory, or its committed lock, could not be read
    #[error("cannot read the sidecar package at {}: {source}", .dir.display())]
    PackageUnreadable {
        dir: PathBuf,
        #[source]
        source: io::Error,
    },

    /// The sidecar's packages are not installed (npm's hidden lockfile is absent)
    #[error(
        "the sidecar's packages are not installed in {}: run `npm ci --prefix {}`",
        .dir.join("node_modules").display(),
        .dir.display()
    )]
    NotInstalled { dir: PathBuf },

    /// The installed packages differ from the committed lock (a lock changed since the install)
    #[error(
        "the sidecar's installed packages in {} differ from {}: run `npm ci --prefix {}`",
        .dir.join("node_modules").display(),
        .dir.join("package-lock.json").display(),
        .dir.display()
    )]
    InstallStale { dir: PathBuf },

    /// No `node` on `PATH`
    #[error("node not found on PATH: the sidecar runs on Node")]
    NodeNotFound,

    /// The `node` found on `PATH` printed no Node version when asked
    #[error(
        "{} reported no Node version: `-p process.versions.node` gave {output:?}",
        .program.display()
    )]
    NodeVersionUnknown { program: PathBuf, output: String },

    /// The `node` found on `PATH` is below the floor the sidecar's `package.json` states
    #[error(
        "{} is Node {version}, below the floor `{floor}` that `engines.node` in {} states",
        .program.display(),
        .manifest.display()
    )]
    NodeTooOld {
        program: PathBuf,
        version: String,
        floor: String,
        manifest: PathBuf,
    },

    /// The `node` found on `PATH` ran the sidecar under another runtime, which refused it
    #[error("{} is not Node: the sidecar refused to run under it", .program.display())]
    NotNode { program: PathBuf },

    /// Failed to spawn the Node process
    #[error("failed to spawn node: {0}")]
    ProcessSpawn(#[source] io::Error),

    /// Failed to get stdio pipe
    #[error("failed to get {pipe} pipe from the sidecar process")]
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
    #[error("sidecar process crashed")]
    Crashed,

    /// Actor shutdown
    #[error("sidecar actor shut down")]
    ActorShutdown,

    /// Request timed out
    #[error("sidecar timed out after {seconds}s")]
    Timeout { seconds: u64 },
}

impl SidecarError {
    /// Get a hint for how to fix this error
    #[must_use]
    pub fn hint(&self) -> &'static str {
        match self {
            Self::PackageUnreadable { .. } => {
                "tsv_debug runs the sidecar in place from the source tree it was built from (crates/tsv_debug/sidecar)"
            }
            Self::NotInstalled { .. } | Self::InstallStale { .. } => INSTALL_HINT,
            Self::NodeNotFound => {
                "Install Node, at or above the floor `engines.node` states in crates/tsv_debug/sidecar/package.json, and put it on PATH"
            }
            Self::NodeVersionUnknown { .. } => "Check that the `node` first on PATH is Node",
            Self::NodeTooOld { .. } => "Put a Node at or above the sidecar's floor first on PATH",
            Self::NotNode { .. } => {
                "Put a real Node first on PATH: with no Node installed, `deno task` puts its own `node`, which runs Deno, there"
            }
            Self::ProcessSpawn(_) => "Check that the `node` on PATH can be executed",
            Self::Crashed => "This may be a bug in the sidecar script",
            Self::Timeout { .. } => "The sidecar may be stuck starting up or processing a request",
            Self::EmptyOutput => {
                "Transient under load — re-run; if it persists, check the sidecar (tsv_debug check)"
            }
            _ => "",
        }
    }
}
