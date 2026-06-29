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
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::NamedTempFile;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, Command};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, timeout};

/// Timeout for reading a response from the sidecar (30 seconds)
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Embedded sidecar script
const SIDECAR_SCRIPT: &str = include_str!("sidecar.ts");

/// Deno config for import map (ensures acorn-typescript uses same acorn instance)
const DENO_CONFIG: &str = r#"{"imports":{"acorn":"npm:acorn@8.16.0"}}"#;

/// Request ID counter
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Process-lifetime runtime that owns every sidecar's background tasks (the
/// actor event loop plus the stdout/stderr reader tasks) and the child-process
/// I/O registrations.
///
/// The sidecar pool is a process-global `static` (see `mod.rs`), but
/// `tokio::spawn` binds a task to whatever runtime is current at spawn time.
/// The production `tsv_debug` binary runs a single long-lived `#[tokio::main]`
/// runtime, so that is harmless there — but the test suite runs many
/// `#[tokio::test]`s, each with its own short-lived runtime. Binding the pool's
/// tasks to whichever test runtime first initialized the pool meant the tasks
/// were aborted the moment that test finished its runtime, so every later test
/// pulled a now-dead actor from the pool and got [`DenoError::ActorShutdown`].
/// Pinning the tasks (and the child-process I/O) to this dedicated runtime
/// decouples the pool from any caller's runtime: it lives for the whole process
/// regardless of which runtime first touched it.
fn sidecar_runtime() -> &'static Runtime {
    static SIDECAR_RT: OnceLock<Runtime> = OnceLock::new();
    SIDECAR_RT.get_or_init(|| {
        // One worker thread is plenty: the sidecars are I/O-bound and each
        // multiplexes its own requests; this thread only drives the actor loops
        // and the stdout/stderr readers.
        #[allow(clippy::expect_used)]
        // runtime build fails only on catastrophic OS resource exhaustion, with no recovery path
        Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("tsv-deno-sidecar")
            .build()
            .expect("failed to build the deno sidecar runtime")
    })
}

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

        // Bind the child process I/O and all three background tasks below to the
        // process-lifetime sidecar runtime rather than whatever runtime is
        // current here. This guard must stay live through the final
        // `tokio::spawn` (it restores the previous context on drop), so it is a
        // named binding — `let _` would drop it immediately. See
        // `sidecar_runtime`.
        let _rt_guard = sidecar_runtime().enter();

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
            // Without this, prettier-plugin-svelte silently emits the whole
            // <script>/<style> block verbatim when the embedded formatter
            // throws — fake "prettier-stable" output that poisons fixture
            // baselines and comparisons. PRETTIER_DEBUG makes the plugin and
            // prettier-core rethrow, so the failure surfaces as a tool error.
            .env("PRETTIER_DEBUG", "1")
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

        // Spawn dedicated stdout reader task.
        // read_line is NOT cancel-safe, so we must not use it inside tokio::select!.
        // This task reads complete lines and sends them via a cancel-safe channel.
        let (line_tx, line_rx) = mpsc::channel::<String>(64);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        if line_tx.send(line).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("[deno] stdout read error: {e}");
                        break;
                    }
                }
            }
        });

        // Create channel for requests
        let (tx, rx) = mpsc::channel(256);

        // Spawn actor task
        let actor_state = ActorState {
            child,
            stdin: BufWriter::new(stdin),
            line_rx,
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
        // Best-effort shutdown signal, spawned on the dedicated sidecar runtime
        // so the drop doesn't depend on an ambient runtime being current (in
        // practice the pool is a process-lifetime static that never drops).
        let tx = self.tx.clone();
        sidecar_runtime().spawn(async move {
            let _ = tx.send(ActorCommand::Shutdown).await;
        });
    }
}

/// Internal actor state
struct ActorState {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    line_rx: mpsc::Receiver<String>,
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
        // Receive from the dedicated reader task (cancel-safe, unlike read_line)
        let line = match timeout(READ_TIMEOUT, self.line_rx.recv()).await {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(false), // EOF - reader task ended
            Err(_) => {
                return Err(DenoError::Timeout {
                    seconds: READ_TIMEOUT.as_secs(),
                });
            }
        };

        // Skip empty lines and non-JSON output (defensive against stdout noise from npm packages)
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(true);
        }
        if !trimmed.starts_with('{') {
            // npm packages sometimes write warnings to stdout - log and continue
            eprintln!("[deno] Unexpected stdout: {trimmed}");
            return Ok(true);
        }

        let response: WireResponse = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                // Log the actual content that failed to parse for debugging
                eprintln!("[deno] Failed to parse response: {e}");
                eprintln!("[deno] Raw content ({} bytes): {trimmed:?}", trimmed.len());
                return Err(DenoError::ResponseParse(e));
            }
        };

        // id: -1 means the sidecar couldn't parse our request (log and continue)
        if response.id < 0 {
            eprintln!(
                "[deno] Malformed request error: {}",
                response.error.as_deref().unwrap_or("unknown")
            );
            return Ok(true);
        }

        // Find and complete the pending request
        #[allow(clippy::cast_sign_loss)]
        if let Some(tx) = self.pending.remove(&(response.id as u64)) {
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

            // Read responses from sidecar (only when requests are pending)
            result = state.read_response(), if !state.pending.is_empty() => {
                match result {
                    Ok(true) => {} // Response handled
                    Ok(false) => {
                        eprintln!("deno sidecar process exited unexpectedly ({} requests pending)", state.pending.len());
                        state.fail_all_pending(|| DenoError::SidecarCrashed);
                        break;
                    }
                    Err(DenoError::Timeout { .. }) => {
                        eprintln!("deno sidecar timed out ({} requests pending)", state.pending.len());
                        state.fail_all_pending(|| DenoError::Timeout { seconds: READ_TIMEOUT.as_secs() });
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error reading from deno sidecar: {e} ({} requests pending)", state.pending.len());
                        state.fail_all_pending(|| DenoError::SidecarCrashed);
                        break;
                    }
                }
            }
        }
    }
}
