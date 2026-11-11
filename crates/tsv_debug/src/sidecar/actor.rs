//! Sidecar actor
//!
//! Manages a long-running Node process that handles JS tool requests.
//! Communication is via JSON-lines over stdio.

use super::error::SidecarError;
use super::protocol::{WireRequest, WireResponse};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, Command};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, timeout};

/// Timeout for reading a response from a running sidecar.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for the sidecar's FIRST response, which also waits on node starting
/// and loading the sidecar's module graph (prettier, svelte, acorn) — seconds
/// on an idle machine, far longer when it launches beside a saturated test run.
/// Every later response is held to [`READ_TIMEOUT`], so a stuck request still
/// fails fast.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(120);

/// How long `node -p process.versions.node` gets to answer before a spawn: a real
/// Node answers in milliseconds, and a `node` that never does fails the spawn
/// rather than hanging it.
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a sidecar whose stdout has closed gets to report its exit status.
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);

/// The exit code of a sidecar that refused the runtime it was started under
/// (`NOT_NODE_EXIT` in `sidecar.ts`): the `node` found on `PATH` is another
/// runtime's (Deno's or Bun's).
const NOT_NODE_EXIT: i32 = 78;

/// The sidecar's source, read by the tests that pin it against its Rust twins.
#[cfg(test)]
pub(crate) const SIDECAR_SCRIPT: &str = include_str!("../../sidecar/sidecar.ts");

/// The sidecar's package directory as the build recorded it: the sidecar runs in
/// place from the source tree this crate was built from, with the packages
/// `npm ci --prefix crates/tsv_debug/sidecar` installed there.
fn sidecar_dir_recorded() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("sidecar")
}

/// The sidecar's package directory, canonical (see [`canonical_dir`]).
fn sidecar_dir() -> Result<PathBuf, SidecarError> {
    canonical_dir(&sidecar_dir_recorded())
}

/// `dir` resolved, its symlinks followed. Node's permission model refuses a
/// grant, or a script, reached through a symlink, so the grant and the script
/// path are both spelled from this one resolved path.
fn canonical_dir(dir: &Path) -> Result<PathBuf, SidecarError> {
    let canonical =
        std::fs::canonicalize(dir).map_err(|source| SidecarError::PackageUnreadable {
            dir: dir.to_path_buf(),
            source,
        })?;
    Ok(strip_verbatim_prefix(canonical))
}

/// `path` without the `\\?\` verbatim prefix `canonicalize` gives every path on
/// Windows (`\\?\C:\…`, and `\\?\UNC\server\share\…` for a share, which becomes
/// `\\server\share\…`). Node's permission grants are matched against the
/// path Node resolves itself, which it doesn't spell verbatim (unverified on
/// Windows). No other platform's canonical path begins with the prefix.
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path;
    };
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{share}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\")
        && rest.as_bytes().get(1) == Some(&b':')
    {
        PathBuf::from(rest)
    } else {
        path
    }
}

/// The `packages` of an npm lockfile, without the root project's own `""` entry —
/// the entries the committed `package-lock.json` and npm's hidden lockfile
/// (`node_modules/.package-lock.json`, which `npm ci` writes for the tree it
/// installed and which has no root entry) share.
fn lock_packages(text: &str) -> Option<serde_json::Map<String, Value>> {
    let mut lock: Value = serde_json::from_str(text).ok()?;
    let mut packages = lock.get_mut("packages")?.as_object_mut()?.clone();
    packages.remove("");
    Some(packages)
}

/// Refuses a sidecar whose installed packages aren't the committed lock's.
///
/// `npm ci` installs exactly the lock, but only when it runs: a lock changed
/// since (a pin bump pulled, a branch switched) leaves the old tree in place,
/// and the sidecar would grade every fixture against an oracle no file in the
/// repo names. npm's hidden lockfile records the tree it last installed, so its
/// entries are compared with the committed lock's, and each package's installed
/// `package.json` with its lock entry's `version`, which catches a package
/// directory removed or its version edited since. An edit to any other file
/// inside the install passes.
fn check_install(dir: &Path) -> Result<(), SidecarError> {
    let read_error = |source| SidecarError::PackageUnreadable {
        dir: dir.to_path_buf(),
        source,
    };
    let committed = std::fs::read_to_string(dir.join("package-lock.json")).map_err(read_error)?;
    let committed = lock_packages(&committed).ok_or_else(|| {
        read_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "package-lock.json has no `packages` object",
        ))
    })?;
    let installed = match std::fs::read_to_string(dir.join("node_modules/.package-lock.json")) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SidecarError::NotInstalled {
                dir: dir.to_path_buf(),
            });
        }
        Err(e) => return Err(read_error(e)),
    };
    let stale = || SidecarError::InstallStale {
        dir: dir.to_path_buf(),
    };
    if lock_packages(&installed).as_ref() != Some(&committed) {
        return Err(stale());
    }
    for (key, entry) in &committed {
        let locked = entry.get("version").and_then(Value::as_str);
        if locked.is_none() || installed_version(&dir.join(key)).as_deref() != locked {
            return Err(stale());
        }
    }
    Ok(())
}

/// The `version` in the `package.json` of the installed package at `package_dir`,
/// or `None` when it can't be read.
fn installed_version(package_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(package_dir.join("package.json")).ok()?;
    let manifest: Value = serde_json::from_str(&text).ok()?;
    Some(manifest.get("version")?.as_str()?.to_owned())
}

/// A Node version's `major.minor.patch`.
type NodeVersion = (u32, u32, u32);

/// The Node floor the sidecar's `package.json` states in `engines.node`, as
/// written there and parsed. Its one form is `>=major.minor[.patch]`.
struct NodeFloor {
    range: String,
    version: NodeVersion,
}

/// Reads the sidecar's Node floor from its `package.json`, the one place it's
/// stated (`npm ci` only warns below it).
fn node_floor(dir: &Path) -> Result<NodeFloor, SidecarError> {
    let manifest = dir.join("package.json");
    let read_error = |source| SidecarError::PackageUnreadable {
        dir: dir.to_path_buf(),
        source,
    };
    let text = std::fs::read_to_string(&manifest).map_err(read_error)?;
    let range = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|m| Some(m.get("engines")?.get("node")?.as_str()?.to_owned()));
    let floor = range.and_then(|range| {
        let version = parse_node_floor(&range)?;
        Some(NodeFloor { range, version })
    });
    floor.ok_or_else(|| {
        read_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "package.json's `engines.node` is not a floor of the form `>=major.minor[.patch]`",
        ))
    })
}

/// `>=major.minor[.patch]` as a version, a missing patch `0`.
fn parse_node_floor(range: &str) -> Option<NodeVersion> {
    let mut parts = range.strip_prefix(">=")?.split('.');
    let major = parse_version_part(parts.next()?)?;
    let minor = parse_version_part(parts.next()?)?;
    let patch = parts.next().map_or(Some(0), parse_version_part)?;
    parts.next().is_none().then_some((major, minor, patch))
}

/// `process.versions.node` (`major.minor.patch`, a pre-release suffix after `-`
/// dropped) as a version.
fn parse_node_version(text: &str) -> Option<NodeVersion> {
    let release = text.split_once('-').map_or(text, |(release, _)| release);
    let mut parts = release.split('.');
    let version = (
        parse_version_part(parts.next()?)?,
        parse_version_part(parts.next()?)?,
        parse_version_part(parts.next()?)?,
    );
    parts.next().is_none().then_some(version)
}

/// One decimal part of a version: ASCII digits only.
fn parse_version_part(part: &str) -> Option<u32> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

/// Refuses a `program` whose Node is below the floor the sidecar's `package.json`
/// states, before the sidecar starts: a Node too old for `--permission` or for
/// stripping the script's types would otherwise fail at startup as a crash, and
/// one new enough for both would run the oracle on a Node it isn't pinned to.
/// The version is asked with the sidecar's own cleared environment, and a `program`
/// that gives no answer within `limit` is killed and refused.
fn check_node_version(
    program: &Path,
    dir: &Path,
    env: &[(&'static str, OsString)],
    limit: Duration,
) -> Result<(), SidecarError> {
    let floor = node_floor(dir)?;
    let mut probe = std::process::Command::new(program)
        .args(["-p", "process.versions.node"])
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().map(|(name, value)| (*name, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(SidecarError::ProcessSpawn)?;
    let deadline = std::time::Instant::now() + limit;
    while probe
        .try_wait()
        .map_err(SidecarError::ProcessSpawn)?
        .is_none()
    {
        if std::time::Instant::now() >= deadline {
            #[expect(clippy::let_underscore_must_use)]
            let _ = probe.kill();
            #[expect(clippy::let_underscore_must_use)]
            let _ = probe.wait();
            return Err(SidecarError::NodeVersionUnknown {
                program: program.to_path_buf(),
                output: format!("no answer within {}s", limit.as_secs_f64()),
            });
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = probe
        .wait_with_output()
        .map_err(SidecarError::ProcessSpawn)?;
    let printed = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let version = output
        .status
        .success()
        .then(|| parse_node_version(&printed))
        .flatten()
        .ok_or_else(|| SidecarError::NodeVersionUnknown {
            program: program.to_path_buf(),
            output: if output.status.success() {
                printed.clone()
            } else {
                format!(
                    "{}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                )
            },
        })?;
    if version < floor.version {
        return Err(SidecarError::NodeTooOld {
            program: program.to_path_buf(),
            version: printed,
            floor: floor.range,
            manifest: dir.join("package.json"),
        });
    }
    Ok(())
}

/// The first `node` on `path_var` (the parent's `PATH`): an executable file in an
/// absolute directory. Relative entries are skipped, since one would resolve
/// against the sidecar's working directory, not the caller's.
///
/// Resolved here rather than by the spawn because the child's environment is
/// cleared, and a cleared environment has no `PATH`: on Unix the spawn would
/// search the libc default (`/bin:/usr/bin`) and on Windows the system
/// directories before the parent's `PATH`.
fn find_node(path_var: Option<&OsStr>) -> Option<PathBuf> {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::split_paths(path_var?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// The sidecar's whole environment, from the parent's: nothing is inherited but
/// what the sidecar needs, so a caller's `NODE_OPTIONS` (an `--import`, a stack
/// or heap size, permission flags) can't change or crash the oracle, and a
/// locale or time zone can't reach it.
fn sidecar_env(parent: impl Fn(&str) -> Option<OsString>) -> Vec<(&'static str, OsString)> {
    // Without this, prettier-plugin-svelte silently emits the whole
    // <script>/<style> block verbatim when the embedded formatter
    // throws — fake "prettier-stable" output that poisons fixture
    // baselines and comparisons. PRETTIER_DEBUG makes the plugin and
    // prettier-core rethrow, so the failure surfaces as a tool error.
    let mut env = vec![("PRETTIER_DEBUG", OsString::from("1"))];
    // Windows processes expect the system root (Node's startup and its system
    // calls read it); unverified which, if any, the sidecar would fail without.
    if cfg!(windows)
        && let Some(root) = parent("SystemRoot")
    {
        env.push(("SystemRoot", root));
    }
    env
}

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
/// pulled a now-dead actor from the pool and got [`SidecarError::ActorShutdown`].
/// Pinning the tasks (and the child-process I/O) to this dedicated runtime
/// decouples the pool from any caller's runtime: it lives for the whole process
/// regardless of which runtime first touched it.
fn sidecar_runtime() -> &'static Runtime {
    static SIDECAR_RT: OnceLock<Runtime> = OnceLock::new();
    SIDECAR_RT.get_or_init(|| {
        // One worker thread is plenty: the sidecars are I/O-bound and each
        // queues its own requests (the JS side processes them serially); this
        // thread only drives the actor loops and the stdout/stderr readers.
        #[expect(clippy::expect_used)]
        // runtime build fails only on catastrophic OS resource exhaustion, with no recovery path
        // The one reservation every tsv thread takes (`tsv_lang::STACK_SIZE`, held on
        // every route by `tsv_cli::cli::stack`): this thread reads the canonical AST back
        // with the recursion limit disabled (`crate::json`), whose soundness argument is
        // precisely that every reader runs on a `STACK_SIZE` stack — tokio's 2 MiB default
        // would be a route ~16x shallower than every other.
        Builder::new_multi_thread()
            .worker_threads(1)
            .thread_stack_size(tsv_lang::STACK_SIZE)
            .enable_all()
            // 15 bytes at most, the OS thread name Linux keeps (`cli::stack::sized_thread`)
            .thread_name("tsv-sidecar")
            .build()
            .expect("failed to build the sidecar runtime")
    })
}

/// Internal request to the actor: the wire request plus where its response goes
struct ActorRequest {
    wire: WireRequest,
    response_tx: oneshot::Sender<Result<Value, SidecarError>>,
}

/// Commands sent to the actor task
enum ActorCommand {
    Request(ActorRequest),
    Shutdown,
}

/// Handle to communicate with the sidecar actor
#[derive(Debug)]
pub(super) struct SidecarActor {
    tx: mpsc::Sender<ActorCommand>,
}

impl SidecarActor {
    /// Spawn a new sidecar actor
    ///
    /// This starts the Node process and background task for handling requests.
    pub(super) fn spawn() -> Result<Self, SidecarError> {
        Self::spawn_in(
            &sidecar_dir()?,
            std::env::var_os("PATH").as_deref(),
            |name| std::env::var_os(name),
        )
    }

    /// Spawn a sidecar from the package at `dir` (canonical) with the first `node`
    /// on `path_var`, its environment taken from `parent` by [`sidecar_env`]. The
    /// install ([`check_install`]) and the Node's version ([`check_node_version`])
    /// are checked first.
    fn spawn_in(
        dir: &Path,
        path_var: Option<&OsStr>,
        parent: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, SidecarError> {
        check_install(dir)?;
        let program = find_node(path_var).ok_or(SidecarError::NodeNotFound)?;
        let env = sidecar_env(parent);
        check_node_version(&program, dir, &env, VERSION_PROBE_TIMEOUT)?;
        let mut grant = OsString::from("--allow-fs-read=");
        grant.push(dir);

        // Bind the child process I/O and all three background tasks below to the
        // process-lifetime sidecar runtime rather than whatever runtime is
        // current here. This guard must stay live through the final
        // `tokio::spawn` (it restores the previous context on drop), so it is a
        // named binding — `let _` would drop it immediately. See
        // `sidecar_runtime`.
        let _rt_guard = sidecar_runtime().enter();

        // The read grant on the package directory is the sidecar's whole reach:
        // no writes, child processes, or workers (on Node 24 the network is not
        // fenced; Node 26 fences it too).
        let mut child = Command::new(&program)
            .arg("--permission")
            .arg(grant)
            .arg(dir.join("sidecar.ts"))
            .current_dir(dir)
            .env_clear()
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(SidecarError::ProcessSpawn)?;

        let stdin = child
            .stdin
            .take()
            .ok_or(SidecarError::PipeMissing { pipe: "stdin" })?;
        let stdout = child
            .stdout
            .take()
            .ok_or(SidecarError::PipeMissing { pipe: "stdout" })?;
        let stderr = child
            .stderr
            .take()
            .ok_or(SidecarError::PipeMissing { pipe: "stderr" })?;

        // Spawn task to log stderr
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("[sidecar] {line}");
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
                        eprintln!("[sidecar] stdout read error: {e}");
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
            program,
            stdin: BufWriter::new(stdin),
            line_rx,
            pending: HashMap::new(),
            answered: false,
        };
        tokio::spawn(run_actor(actor_state, rx));

        Ok(Self { tx })
    }

    /// Enqueue a request on this actor's event loop.
    ///
    /// Returns the response receiver on success, or `None` when the loop has
    /// already exited (its command channel is closed) — in that case the
    /// request provably never left the caller, so the caller may dispatch it
    /// to another (e.g. freshly respawned) actor without ever re-running work
    /// a sidecar might have seen. That distinction is what makes the pool's
    /// dispatch-level heal in `mod.rs` safe: delivered-then-failed requests
    /// surface as hard errors, never as silent retries.
    pub(super) async fn dispatch(
        &self,
        tool: &str,
        content: &str,
        options: Option<&Value>,
    ) -> Option<oneshot::Receiver<Result<Value, SidecarError>>> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();

        let request = ActorRequest {
            wire: WireRequest {
                id,
                tool: tool.to_string(),
                content: content.to_string(),
                options: options.cloned(),
            },
            response_tx,
        };

        (self.tx.send(ActorCommand::Request(request)).await).ok()?;
        Some(response_rx)
    }

    /// Whether this actor's event loop has exited (its command channel closed).
    pub(super) fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

    /// Send a shutdown and wait for the event loop to exit (test-only — lets
    /// the pool-heal test kill an actor deterministically).
    #[cfg(test)]
    pub(super) async fn shutdown_and_wait(&self) {
        let _ = self.tx.send(ActorCommand::Shutdown).await;
        while !self.tx.is_closed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

impl Drop for SidecarActor {
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
    /// The `node` the sidecar was started with, named by [`SidecarError::NotNode`].
    program: PathBuf,
    stdin: BufWriter<ChildStdin>,
    line_rx: mpsc::Receiver<String>,
    pending: HashMap<u64, oneshot::Sender<Result<Value, SidecarError>>>,
    /// Whether the sidecar has written a stdout line yet: until it has, it is
    /// still starting, and a response waits [`STARTUP_TIMEOUT`].
    answered: bool,
}

impl ActorState {
    /// Send a request to the sidecar
    async fn send_request(&mut self, req: ActorRequest) -> Result<(), SidecarError> {
        // Store the response channel
        self.pending.insert(req.wire.id, req.response_tx);

        // Serialize and send
        let json = serde_json::to_string(&req.wire).map_err(|e| {
            SidecarError::Communication(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
        self.stdin
            .write_all(json.as_bytes())
            .await
            .map_err(SidecarError::Communication)?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(SidecarError::Communication)?;
        self.stdin
            .flush()
            .await
            .map_err(SidecarError::Communication)?;

        Ok(())
    }

    /// Read and dispatch a response from the sidecar
    ///
    /// Returns false on EOF (sidecar crashed)
    async fn read_response(&mut self) -> Result<bool, SidecarError> {
        let limit = if self.answered {
            READ_TIMEOUT
        } else {
            STARTUP_TIMEOUT
        };
        // Receive from the dedicated reader task (cancel-safe, unlike read_line)
        let line = match timeout(limit, self.line_rx.recv()).await {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(false), // EOF - reader task ended
            Err(_) => {
                return Err(SidecarError::Timeout {
                    seconds: limit.as_secs(),
                });
            }
        };
        // Any stdout line means the module graph is loaded and running.
        self.answered = true;

        // Skip empty lines and non-JSON output (defensive against stdout noise from npm packages)
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(true);
        }
        if !trimmed.starts_with('{') {
            // npm packages sometimes write warnings to stdout - log and continue
            eprintln!("[sidecar] Unexpected stdout: {trimmed}");
            return Ok(true);
        }

        let response: WireResponse = match crate::json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                // Log the actual content that failed to parse for debugging
                eprintln!("[sidecar] Failed to parse response: {e}");
                eprintln!(
                    "[sidecar] Raw content ({} bytes): {trimmed:?}",
                    trimmed.len()
                );
                return Err(SidecarError::ResponseParse(e));
            }
        };

        // id: -1 means the sidecar couldn't parse our request (log and continue)
        if response.id < 0 {
            eprintln!(
                "[sidecar] Malformed request error: {}",
                response.error.as_deref().unwrap_or("unknown")
            );
            return Ok(true);
        }

        // Find and complete the pending request
        #[expect(clippy::cast_sign_loss)]
        if let Some(tx) = self.pending.remove(&(response.id as u64)) {
            let result = if response.ok {
                response.output.ok_or(SidecarError::MissingOutput)
            } else {
                Err(SidecarError::ToolError {
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
    fn fail_all_pending(&mut self, error_fn: impl Fn() -> SidecarError) {
        for (_, tx) in self.pending.drain() {
            let _ = tx.send(Err(error_fn()));
        }
    }
}

impl ActorState {
    /// Fails every pending request for a sidecar that went away (its stdout closed, or a
    /// request couldn't be written to it): [`SidecarError::NotNode`] when it exited refusing
    /// its runtime, else [`SidecarError::Crashed`]. A sidecar that refused exits before
    /// its first read, so a request written after that fails at the write, and is told why
    /// the same way.
    async fn fail_all_pending_exited(&mut self) {
        match timeout(EXIT_TIMEOUT, self.child.wait()).await {
            Ok(Ok(status)) if status.code() == Some(NOT_NODE_EXIT) => {
                let program = self.program.clone();
                self.fail_all_pending(|| SidecarError::NotNode {
                    program: program.clone(),
                });
            }
            _ => self.fail_all_pending(|| SidecarError::Crashed),
        }
    }
}

impl Drop for ActorState {
    fn drop(&mut self) {
        // Kill the child process
        #[expect(clippy::let_underscore_must_use)]
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
                            // A stdin-write failure means the sidecar died. Fail
                            // every pending request (including the one whose write
                            // just failed — it's already in the map) with the
                            // accurate error BEFORE breaking; dropping the map
                            // instead would surface as a misleading ActorShutdown.
                            // A partially-written request may or may not have
                            // reached the sidecar, so it must NOT be re-dispatched.
                            eprintln!("Failed to send request to the sidecar: {e}");
                            state.fail_all_pending_exited().await;
                            break;
                        }
                    }
                    Some(ActorCommand::Shutdown) | None => {
                        state.fail_all_pending(|| SidecarError::ActorShutdown);
                        break;
                    }
                }
            }

            // Read responses from sidecar (only when requests are pending)
            result = state.read_response(), if !state.pending.is_empty() => {
                match result {
                    Ok(true) => {} // Response handled
                    Ok(false) => {
                        eprintln!("sidecar process exited unexpectedly ({} requests pending)", state.pending.len());
                        state.fail_all_pending_exited().await;
                        break;
                    }
                    Err(SidecarError::Timeout { seconds }) => {
                        eprintln!("sidecar timed out after {seconds}s ({} requests pending)", state.pending.len());
                        state.fail_all_pending(|| SidecarError::Timeout { seconds });
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error reading from the sidecar: {e} ({} requests pending)", state.pending.len());
                        state.fail_all_pending(|| SidecarError::Crashed);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// One request on `actor`, answered.
    async fn request(
        actor: &SidecarActor,
        tool: &str,
        content: &str,
        options: Option<Value>,
    ) -> Result<Value, SidecarError> {
        let rx = actor
            .dispatch(tool, content, options.as_ref())
            .await
            .expect("a fresh actor accepts the request");
        rx.await
            .expect("the actor answers every request it accepts")
    }

    /// A copy of the sidecar's `package.json` and committed lock in a fresh directory. With
    /// `installed`, that is npm's hidden lockfile, and each locked package's installed
    /// `package.json` is copied from the checkout's install (no package's other files).
    fn package_copy(installed: Option<&str>) -> tempfile::TempDir {
        let source = sidecar_dir_recorded();
        let dir = tempfile::tempdir().expect("temp dir");
        for file in ["package.json", "package-lock.json"] {
            std::fs::copy(source.join(file), dir.path().join(file)).expect("copy the package");
        }
        if let Some(text) = installed {
            std::fs::create_dir(dir.path().join("node_modules")).expect("node_modules");
            std::fs::write(dir.path().join("node_modules/.package-lock.json"), text)
                .expect("write the hidden lockfile");
            let lock = std::fs::read_to_string(source.join("package-lock.json")).expect("lock");
            for key in lock_packages(&lock).expect("the lock's packages").keys() {
                let to = dir.path().join(key);
                std::fs::create_dir_all(&to).expect("package dir");
                std::fs::copy(
                    source.join(key).join("package.json"),
                    to.join("package.json"),
                )
                .expect("copy an installed package.json");
            }
        }
        dir
    }

    /// The hidden lockfile of the checkout's own sidecar install.
    fn installed_lock() -> String {
        std::fs::read_to_string(sidecar_dir_recorded().join("node_modules/.package-lock.json"))
            .expect("the sidecar is installed (npm ci --prefix crates/tsv_debug/sidecar)")
    }

    /// U+2028 and U+2029 travel raw inside a request's strings (serde_json escapes neither),
    /// so the sidecar must end a request at LF alone: a split at either cuts the request in
    /// half, and it is never answered.
    #[tokio::test]
    async fn line_and_paragraph_separators_stay_inside_their_request() {
        let actor = SidecarActor::spawn().expect("spawn the sidecar");
        let source = "const a = \"x\u{2028}y\u{2029}z\";\n";
        assert!(
            serde_json::to_string(source).unwrap().contains('\u{2028}'),
            "the request must carry the separator raw for this to pin the split"
        );
        let formatted = request(
            &actor,
            "prettier",
            source,
            Some(json!({ "parser": "typescript" })),
        )
        .await;
        assert_eq!(
            formatted.expect("prettier answers"),
            "const a = 'x\u{2028}y\u{2029}z';\n"
        );
        let parsed = request(&actor, "svelte-parse", "<p>\u{2029}</p>", None)
            .await
            .expect("svelte-parse answers the next request");
        assert_eq!(parsed["type"], "Root");
    }

    /// No install beside the lock is its own error, naming the install command, before any
    /// process starts.
    #[test]
    fn a_missing_install_is_refused_before_the_spawn() {
        let dir = package_copy(None);
        let error =
            SidecarActor::spawn_in(dir.path(), std::env::var_os("PATH").as_deref(), |_| None)
                .expect_err("no install");
        assert!(
            matches!(&error, SidecarError::NotInstalled { dir: d } if d == dir.path()),
            "{error:?}"
        );
        assert!(
            error
                .to_string()
                .contains(&format!("npm ci --prefix {}", dir.path().display())),
            "{error}"
        );
    }

    /// An install that differs from the committed lock is refused: the oracle it would run is
    /// one no file in the repo names.
    #[test]
    fn an_install_that_differs_from_the_lock_is_refused() {
        let current = installed_lock();
        assert!(check_install(package_copy(Some(&current)).path()).is_ok());

        let stale = current.replacen("\"version\": \"2.4.0\"", "\"version\": \"2.3.1\"", 1);
        assert_ne!(stale, current, "the hidden lockfile pins esrap 2.4.0");
        let dir = package_copy(Some(&stale));
        let error =
            SidecarActor::spawn_in(dir.path(), std::env::var_os("PATH").as_deref(), |_| None)
                .expect_err("a stale install");
        assert!(
            matches!(&error, SidecarError::InstallStale { dir: d } if d == dir.path()),
            "{error:?}"
        );

        let unreadable = package_copy(Some("{"));
        assert!(matches!(
            check_install(unreadable.path()),
            Err(SidecarError::InstallStale { .. })
        ));
    }

    /// The hidden lockfile says what `npm ci` installed, not what is there now: an installed
    /// package whose own `package.json` names another version than its lock entry, or whose
    /// directory is gone, is a stale install.
    #[test]
    fn an_installed_package_that_differs_from_its_lock_entry_is_refused() {
        let current = installed_lock();
        let edited = package_copy(Some(&current));
        let manifest = edited.path().join("node_modules/svelte/package.json");
        let text = std::fs::read_to_string(&manifest).expect("svelte's package.json");
        let bumped = text.replacen("\"version\": \"5.57.3\"", "\"version\": \"5.57.4\"", 1);
        assert_ne!(bumped, text, "the install holds svelte 5.57.3");
        std::fs::write(&manifest, bumped).expect("bump svelte");
        assert!(matches!(
            check_install(edited.path()),
            Err(SidecarError::InstallStale { .. })
        ));

        let scoped = package_copy(Some(&current));
        let manifest = scoped
            .path()
            .join("node_modules/@sveltejs/acorn-typescript/package.json");
        let text = std::fs::read_to_string(&manifest).expect("acorn-typescript's package.json");
        let bumped = text.replacen("\"version\": \"1.0.13\"", "\"version\": \"1.0.14\"", 1);
        assert_ne!(
            bumped, text,
            "the install holds @sveltejs/acorn-typescript 1.0.13"
        );
        std::fs::write(&manifest, bumped).expect("bump acorn-typescript");
        assert!(matches!(
            check_install(scoped.path()),
            Err(SidecarError::InstallStale { .. })
        ));

        let removed = package_copy(Some(&current));
        std::fs::remove_dir_all(removed.path().join("node_modules/esrap")).expect("remove esrap");
        assert!(matches!(
            check_install(removed.path()),
            Err(SidecarError::InstallStale { .. })
        ));
    }

    /// The floor is `package.json`'s `engines.node`, in its one form.
    #[test]
    fn the_node_floor_is_the_packages_engines() {
        let floor = node_floor(&sidecar_dir_recorded()).expect("the sidecar states a floor");
        let manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(sidecar_dir_recorded().join("package.json")).expect("read"),
        )
        .expect("parse");
        assert_eq!(floor.range, manifest["engines"]["node"]);
        assert_eq!(parse_node_floor(">=24.14"), Some((24, 14, 0)));
        assert_eq!(parse_node_floor(">=24.14.2"), Some((24, 14, 2)));
        for range in [
            "24.14",
            ">=24",
            "^24.14",
            ">=24.14.0.1",
            ">=24.x",
            ">= 24.14",
            ">=+24.14",
        ] {
            assert_eq!(parse_node_floor(range), None, "{range}");
        }
        assert_eq!(parse_node_version("26.13.0"), Some((26, 13, 0)));
        assert_eq!(parse_node_version("27.0.0-nightly2026"), Some((27, 0, 0)));
        for text in ["", "v26.13.0", "26.13", "deno 2.9.8", "26.13.0.1"] {
            assert_eq!(parse_node_version(text), None, "{text}");
        }

        let unstated = package_copy(None);
        std::fs::write(unstated.path().join("package.json"), "{\"engines\": {}}").expect("write");
        assert!(matches!(
            node_floor(unstated.path()),
            Err(SidecarError::PackageUnreadable { .. })
        ));
    }

    /// A `node` script in a fresh `PATH` directory, printing `version` for
    /// `-p process.versions.node` (and anything else exiting as an old Node does on
    /// `--permission`).
    #[cfg(unix)]
    fn fake_node(version: &str) -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let bin = tempfile::tempdir().expect("temp dir");
        let node = bin.path().join("node");
        std::fs::write(
            &node,
            format!(
                "#!/bin/sh\nif [ \"$1\" = -p ]; then echo '{version}'; exit 0; fi\n\
                 echo \"$0: bad option: $1\" >&2\nexit 9\n"
            ),
        )
        .expect("write node");
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).expect("mode");
        bin
    }

    /// A Node below the floor `engines.node` states is refused before the sidecar starts, named
    /// with its version, the floor, and where the floor is stated. At the floor it is let
    /// through (and this fake then fails as a too-old Node does, a crash).
    #[cfg(unix)]
    #[tokio::test]
    async fn a_node_below_the_floor_is_refused() {
        let dir = sidecar_dir().expect("sidecar dir");
        let floor = node_floor(&dir).expect("floor");
        let (major, minor, _) = floor.version;
        let below_version = if minor > 0 {
            format!("{major}.{}.9", minor - 1)
        } else {
            format!("{}.9.9", major - 1)
        };
        let below = fake_node(&below_version);
        let error = SidecarActor::spawn_in(&dir, Some(below.path().as_os_str()), |_| None)
            .expect_err("a Node below the floor");
        match &error {
            SidecarError::NodeTooOld {
                program,
                version,
                floor: range,
                manifest,
            } => {
                assert_eq!(program, &below.path().join("node"));
                assert_eq!(version, &below_version);
                assert_eq!(range, &floor.range);
                assert_eq!(manifest, &dir.join("package.json"));
            }
            other => panic!("expected NodeTooOld, got {other:?}"),
        }

        let at = fake_node(&format!("{major}.{minor}.0"));
        let actor = SidecarActor::spawn_in(&dir, Some(at.path().as_os_str()), |_| None)
            .expect("a Node at the floor starts");
        assert!(matches!(
            request(&actor, "__version_info", "", None).await,
            Err(SidecarError::Crashed)
        ));
    }

    /// A `node` that prints no Node version is refused before the sidecar starts.
    #[cfg(unix)]
    #[test]
    fn a_node_with_no_version_is_refused() {
        let dir = sidecar_dir().expect("sidecar dir");
        let bin = fake_node("deno 2.9.8");
        let error = SidecarActor::spawn_in(&dir, Some(bin.path().as_os_str()), |_| None)
            .expect_err("no Node version");
        assert!(
            matches!(&error, SidecarError::NodeVersionUnknown { output, .. } if output == "deno 2.9.8"),
            "{error:?}"
        );
    }

    /// A `node` that never answers the version probe is killed and refused, not waited on.
    #[cfg(unix)]
    #[test]
    fn a_node_that_never_answers_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let bin = tempfile::tempdir().expect("temp dir");
        let node = bin.path().join("node");
        std::fs::write(&node, "#!/bin/sh\nexec /bin/sleep 600\n").expect("write node");
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let dir = sidecar_dir().expect("sidecar dir");
        let error = check_node_version(&node, &dir, &[], Duration::from_millis(300))
            .expect_err("no answer");
        assert!(
            matches!(&error, SidecarError::NodeVersionUnknown { output, .. } if output == "no answer within 0.3s"),
            "{error:?}"
        );
    }

    /// A symlinked package directory resolves to its target, the one spelling Node's
    /// permission model accepts for the grant and the script, and a sidecar spawned from it
    /// answers.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlinked_package_directory_is_resolved() {
        let links = tempfile::tempdir().expect("temp dir");
        let link = links.path().join("sidecar");
        std::os::unix::fs::symlink(sidecar_dir_recorded(), &link).expect("link the package");
        let dir = canonical_dir(&link).expect("canonical");
        assert_eq!(dir, sidecar_dir().expect("sidecar dir"));
        assert_ne!(dir, link);
        let actor = SidecarActor::spawn_in(&dir, std::env::var_os("PATH").as_deref(), |_| None)
            .expect("spawn");
        let info = request(&actor, "__version_info", "", None)
            .await
            .expect("the sidecar answers");
        assert!(info["runtime"].is_string(), "{info}");
    }

    /// The parent's environment never reaches the sidecar. Runs this test binary again with a
    /// `NODE_OPTIONS` that stops Node at startup, and the sidecar there still answers.
    #[test]
    fn a_parent_node_options_does_not_reach_the_sidecar() {
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "sidecar::actor::tests::answers_under_a_poisoned_parent_environment",
                "--ignored",
                "--test-threads=1",
            ])
            .env(
                "NODE_OPTIONS",
                "--require=/nonexistent/tsv_sidecar_poison.cjs",
            )
            .output()
            .expect("run the child test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The child half of `a_parent_node_options_does_not_reach_the_sidecar`.
    #[ignore = "run by a_parent_node_options_does_not_reach_the_sidecar, which poisons its environment"]
    #[tokio::test]
    async fn answers_under_a_poisoned_parent_environment() {
        assert!(
            std::env::var_os("NODE_OPTIONS").is_some(),
            "run by its parent test"
        );
        let actor = SidecarActor::spawn().expect("spawn the sidecar");
        let formatted = request(
            &actor,
            "prettier",
            "<div>a</div>",
            Some(json!({ "parser": "svelte" })),
        )
        .await;
        assert_eq!(formatted.expect("prettier answers"), "<div>a</div>\n");
    }

    /// A `node` that runs Deno (the one `deno task` puts on `PATH` with no Node installed) is
    /// refused by the sidecar, and the actor names it. Needs `deno` on `PATH`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_node_that_runs_deno_is_refused() {
        let Some(deno) = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("deno"))
                .find(|candidate| is_executable(candidate))
        }) else {
            eprintln!("skipped: no deno on PATH");
            return;
        };
        let shim = tempfile::tempdir().expect("temp dir");
        let node = shim.path().join("node");
        std::os::unix::fs::symlink(&deno, &node).expect("link node to deno");
        let actor = SidecarActor::spawn_in(
            &sidecar_dir().expect("sidecar dir"),
            Some(shim.path().as_os_str()),
            |_| None,
        )
        .expect("spawn the sidecar under deno");
        match request(&actor, "__version_info", "", None).await {
            Err(SidecarError::NotNode { program }) => assert_eq!(program, node),
            other => panic!("expected NotNode, got {other:?}"),
        }
        // A request sent after the refusal fails at its write, and is told the same.
        let late = SidecarActor::spawn_in(
            &sidecar_dir().expect("sidecar dir"),
            Some(shim.path().as_os_str()),
            |_| None,
        )
        .expect("spawn the sidecar under deno");
        tokio::time::sleep(Duration::from_secs(5)).await;
        match request(&late, "__version_info", "", None).await {
            Err(SidecarError::NotNode { program }) => assert_eq!(program, node),
            other => panic!("expected NotNode for a late request, got {other:?}"),
        }
        // The refusal comes before the oracle loads: in a package whose install holds the
        // manifests and no code, the script is still refused rather than failing an import.
        let package = package_copy(Some(&installed_lock()));
        std::fs::copy(
            sidecar_dir_recorded().join("sidecar.ts"),
            package.path().join("sidecar.ts"),
        )
        .expect("copy the script");
        let dir = canonical_dir(package.path()).expect("canonical temp dir");
        let codeless = SidecarActor::spawn_in(&dir, Some(shim.path().as_os_str()), |_| None)
            .expect("spawn the sidecar under deno");
        match request(&codeless, "__version_info", "", None).await {
            Err(SidecarError::NotNode { program }) => assert_eq!(program, node),
            other => panic!("expected NotNode before any oracle import, got {other:?}"),
        }
    }

    /// A sidecar that exits on its own (not the runtime refusal) is a crash, not `NotNode`.
    #[tokio::test]
    async fn a_sidecar_that_exits_is_a_crash() {
        let package = package_copy(Some(&installed_lock()));
        std::fs::write(package.path().join("sidecar.ts"), "process.exit(1);\n").expect("script");
        let dir = canonical_dir(package.path()).expect("canonical temp dir");
        let actor = SidecarActor::spawn_in(&dir, std::env::var_os("PATH").as_deref(), |name| {
            std::env::var_os(name)
        })
        .expect("spawn");
        assert!(matches!(
            request(&actor, "__version_info", "", None).await,
            Err(SidecarError::Crashed)
        ));
    }

    /// The first `node` that is an executable file in an absolute `PATH` entry.
    #[cfg(unix)]
    #[test]
    fn node_is_the_first_executable_in_an_absolute_path_entry() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().expect("temp dir");
        let entry = |name: &str, mode: Option<u32>| {
            let dir = root.path().join(name);
            std::fs::create_dir(&dir).expect("dir");
            if let Some(mode) = mode {
                let node = dir.join("node");
                std::fs::write(&node, "").expect("node");
                std::fs::set_permissions(&node, std::fs::Permissions::from_mode(mode))
                    .expect("mode");
            }
            dir
        };
        // a relative spelling of an entry holding a `node`, which the search must skip
        let relative = {
            let cwd = std::env::current_dir().expect("cwd");
            let ups = "../".repeat(cwd.components().count() - 1);
            let dir = entry("relative", Some(0o755));
            PathBuf::from(ups).join(dir.strip_prefix("/").expect("absolute temp dir"))
        };
        assert!(
            is_executable(&relative.join("node")),
            "{}",
            relative.display()
        );
        let empty = entry("empty", None);
        let directory = entry("directory", None);
        std::fs::create_dir(directory.join("node")).expect("a directory named node");
        let unexecutable = entry("unexecutable", Some(0o644));
        let first = entry("first", Some(0o755));
        let second = entry("second", Some(0o755));
        let path = std::env::join_paths([
            relative,
            empty,
            directory,
            unexecutable,
            first.clone(),
            second,
        ])
        .expect("join");
        assert_eq!(find_node(Some(&path)), Some(first.join("node")));
        assert_eq!(find_node(None), None);
    }

    /// The sidecar's environment is `PRETTIER_DEBUG` alone (and the system root on Windows),
    /// whatever the parent holds.
    #[test]
    fn the_sidecar_environment_names_only_what_it_needs() {
        let env = sidecar_env(|name| Some(OsString::from(format!("parent {name}"))));
        let names: Vec<&str> = env.iter().map(|(name, _)| *name).collect();
        let expected: &[&str] = if cfg!(windows) {
            &["PRETTIER_DEBUG", "SystemRoot"]
        } else {
            &["PRETTIER_DEBUG"]
        };
        assert_eq!(names, expected);
        assert_eq!(env[0].1, "1");
    }

    #[test]
    fn the_verbatim_prefix_is_stripped() {
        let strip = |p: &str| strip_verbatim_prefix(PathBuf::from(p));
        assert_eq!(
            strip(r"\\?\C:\tsv\sidecar"),
            PathBuf::from(r"C:\tsv\sidecar")
        );
        assert_eq!(
            strip(r"\\?\UNC\host\share\tsv"),
            PathBuf::from(r"\\host\share\tsv")
        );
        assert_eq!(
            strip(r"\\?\GLOBALROOT\x"),
            PathBuf::from(r"\\?\GLOBALROOT\x")
        );
        assert_eq!(
            strip("/home/tsv/sidecar"),
            PathBuf::from("/home/tsv/sidecar")
        );
    }
}
