//! Deno sidecar actor
//!
//! Manages a long-running Deno process that handles JS tool requests.
//! Communication is via JSON-lines over stdio.

use super::error::DenoError;
use super::protocol::{WireRequest, WireResponse};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::NamedTempFile;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};

/// Embedded sidecar script
const SIDECAR_SCRIPT: &str = include_str!("sidecar.ts");

/// Deno config for import map (ensures acorn-typescript uses same acorn instance)
const DENO_CONFIG: &str = r#"{"imports":{"acorn":"npm:acorn@8.15.0"}}"#;

/// Request ID counter
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Internal request to the actor
struct ActorRequest {
    id: u64,
    tool: String,
    content: String,
    options: Option<Value>,
    response_tx: oneshot::Sender<Result<Value, DenoError>>,
}

/// Commands sent to the actor task
enum ActorCommand {
    Request(ActorRequest),
    Shutdown,
}

/// Handle to communicate with the Deno actor
#[derive(Debug)]
pub struct DenoActor {
    tx: mpsc::Sender<ActorCommand>,
}

impl DenoActor {
    /// Spawn a new Deno sidecar actor
    ///
    /// This starts the Deno process and background task for handling requests.
    pub fn spawn() -> Result<Self, DenoError> {
        // Write embedded script to tempfile
        let mut script_file = NamedTempFile::new().map_err(DenoError::TempfileCreate)?;
        script_file
            .write_all(SIDECAR_SCRIPT.as_bytes())
            .map_err(DenoError::ScriptWrite)?;

        // Write deno.json config for import map (ensures acorn version alignment)
        let mut config_file = NamedTempFile::new().map_err(DenoError::TempfileCreate)?;
        config_file
            .write_all(DENO_CONFIG.as_bytes())
            .map_err(DenoError::ScriptWrite)?;

        // Spawn Deno process
        let mut child = Command::new("deno")
            .args([
                "run",
                "--allow-read",
                "--allow-env",
                "--allow-sys=cpus",
                "--quiet",
            ])
            .arg(format!("--config={}", config_file.path().display()))
            .arg(script_file.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    DenoError::DenoNotFound
                } else {
                    DenoError::ProcessSpawn(e)
                }
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or(DenoError::PipeMissing { pipe: "stdin" })?;
        let stdout = child
            .stdout
            .take()
            .ok_or(DenoError::PipeMissing { pipe: "stdout" })?;
        let stderr = child
            .stderr
            .take()
            .ok_or(DenoError::PipeMissing { pipe: "stderr" })?;

        // Spawn task to log stderr
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("[deno] {line}");
            }
        });

        // Create channel for requests
        let (tx, rx) = mpsc::channel(256);

        // Spawn actor task
        let actor_state = ActorState {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            pending: HashMap::new(),
            _script_file: script_file, // Keep alive for process lifetime
            _config_file: config_file, // Keep alive for process lifetime
        };
        tokio::spawn(run_actor(actor_state, rx));

        Ok(Self { tx })
    }

    /// Call a tool on the Deno sidecar
    pub async fn call(
        &self,
        tool: &str,
        content: &str,
        options: Option<Value>,
    ) -> Result<Value, DenoError> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();

        let request = ActorRequest {
            id,
            tool: tool.to_string(),
            content: content.to_string(),
            options,
            response_tx,
        };

        self.tx
            .send(ActorCommand::Request(request))
            .await
            .map_err(|_| DenoError::ActorShutdown)?;

        response_rx.await.map_err(|_| DenoError::ActorShutdown)?
    }
}

impl Drop for DenoActor {
    fn drop(&mut self) {
        // Best-effort shutdown signal
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(ActorCommand::Shutdown).await;
        });
    }
}

/// Internal actor state
struct ActorState {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    pending: HashMap<u64, oneshot::Sender<Result<Value, DenoError>>>,
    _script_file: NamedTempFile,
    _config_file: NamedTempFile,
}

impl ActorState {
    /// Send a request to the sidecar
    async fn send_request(&mut self, req: ActorRequest) -> Result<(), DenoError> {
        let wire_req = WireRequest {
            id: req.id,
            tool: req.tool,
            content: req.content,
            options: req.options,
        };

        // Store the response channel
        self.pending.insert(req.id, req.response_tx);

        // Serialize and send
        let json = serde_json::to_string(&wire_req).map_err(|e| {
            DenoError::Communication(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
        self.stdin
            .write_all(json.as_bytes())
            .await
            .map_err(DenoError::Communication)?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(DenoError::Communication)?;
        self.stdin.flush().await.map_err(DenoError::Communication)?;

        Ok(())
    }

    /// Read and dispatch a response from the sidecar
    ///
    /// Returns false on EOF (sidecar crashed)
    async fn read_response(&mut self) -> Result<bool, DenoError> {
        let mut line = String::new();
        let bytes_read = self
            .stdout
            .read_line(&mut line)
            .await
            .map_err(DenoError::Communication)?;

        if bytes_read == 0 {
            return Ok(false); // EOF
        }

        let response: WireResponse =
            serde_json::from_str(&line).map_err(DenoError::ResponseParse)?;

        // Find and complete the pending request
        if let Some(tx) = self.pending.remove(&response.id) {
            let result = if response.ok {
                response.output.ok_or(DenoError::MissingOutput)
            } else {
                Err(DenoError::ToolError {
                    message: response
                        .error
                        .unwrap_or_else(|| "Unknown error".to_string()),
                })
            };
            let _ = tx.send(result);
        }

        Ok(true)
    }

    /// Fail all pending requests
    fn fail_all_pending(&mut self, error_fn: impl Fn() -> DenoError) {
        for (_, tx) in self.pending.drain() {
            let _ = tx.send(Err(error_fn()));
        }
    }
}

impl Drop for ActorState {
    fn drop(&mut self) {
        // Kill the child process
        #[allow(clippy::let_underscore_must_use)]
        let _ = self.child.start_kill();
    }
}

/// Run the actor event loop
async fn run_actor(mut state: ActorState, mut rx: mpsc::Receiver<ActorCommand>) {
    loop {
        tokio::select! {
            biased;

            // Prioritize incoming commands
            cmd = rx.recv() => {
                match cmd {
                    Some(ActorCommand::Request(req)) => {
                        if let Err(e) = state.send_request(req).await {
                            eprintln!("Failed to send request to deno: {e}");
                            break;
                        }
                    }
                    Some(ActorCommand::Shutdown) | None => {
                        state.fail_all_pending(|| DenoError::ActorShutdown);
                        break;
                    }
                }
            }

            // Read responses from sidecar
            result = state.read_response() => {
                match result {
                    Ok(true) => {} // Response handled
                    Ok(false) => {
                        eprintln!("deno sidecar process exited unexpectedly");
                        state.fail_all_pending(|| DenoError::SidecarCrashed);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error reading from deno sidecar: {e}");
                        state.fail_all_pending(|| DenoError::SidecarCrashed);
                        break;
                    }
                }
            }
        }
    }
}
