//! Shared helpers for the CLI integration tests: the binary builder and the three
//! spawn wrappers, the temp-tree guard, the path-normalizing assertion helpers, the
//! file-mode probe and guard, and the unformatted/formatted source constants.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Once, OnceLock};

/// Run the tsv binary with the given arguments.
/// Test helper; panicking on spawn failure is the desired behavior.
#[allow(clippy::expect_used)]
pub(crate) fn tsv(args: &[&str]) -> std::process::Output {
    // the binary `built_tsv` built once, not a `cargo run` per call: a cargo
    // invocation per test is slow, and a `cargo run -p tsv_cli` resolves features
    // for that package alone where the outer `cargo test --workspace` unified them,
    // so a call could relink the binary under a sibling test mid-spawn
    Command::new(built_tsv())
        .args(args)
        .output()
        .expect("Failed to execute command")
}

/// Run the tsv binary, piping `input` to its stdin (for `--stdin` mode).
/// Test helper; panicking on spawn/IO failure is the desired behavior.
#[allow(clippy::expect_used)]
pub(crate) fn tsv_stdin(args: &[&str], input: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(built_tsv())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn command");
    // Writing then dropping the handle closes the pipe so the child sees EOF.
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input.as_bytes())
        .expect("Failed to write to stdin");
    child.wait_with_output().expect("Failed to wait for output")
}

/// Create a fresh temp directory unique to this test, removed when the test leaves.
///
/// Returns the [`TempTree`] guard rather than a bare path so the cleanup rides the
/// scope exit — see that type for why a trailing `remove_dir_all` cleans up the wrong
/// half of the runs.
/// Test helper; panicking on IO failure is the desired behavior.
#[allow(clippy::expect_used)]
pub(crate) fn temp_dir(name: &str) -> TempTree {
    let dir = std::env::temp_dir().join(format!("tsv_cli_tests_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    TempTree(dir)
}

/// Separators normalized to `/`, the spelling an expectation here is written in. The
/// CLI emits **native** separators (`PathBuf::push` parity), so on Windows a listed path
/// and a diagnostic's path both come back `\`-joined and an expectation spelled `/`-joined
/// matches nothing. The discovery harnesses (`tests/discovery_parity.rs`,
/// `scripts/discovery_parity_suite.ts`) normalize the same two sides for the same reason.
/// Apply it per assertion, never blanket: `\` is a legal posix filename byte and the
/// quoted-path tests in `ignore_files` use one on purpose.
pub(crate) fn to_posix(text: &str) -> String {
    text.replace('\\', "/")
}

/// `fs::canonicalize`'s spelling **as the CLI prints it**, for an expectation built from
/// the same resolution the walk does. Two host facts sit between the two otherwise: macOS
/// resolves the `$TMPDIR` symlink (`/var` → `/private/var`), which is why an expectation
/// canonicalizes at all, and Windows returns a verbatim path (`\\?\C:\…`) that the CLI
/// strips before printing — through `tsv_cli::cli::discover::strip_verbatim_prefix`, the
/// CLI's own rule rather than a copy of it, so the two cannot drift.
/// Test helper; panicking on an unresolvable path is the desired behavior.
#[allow(clippy::unwrap_used)]
pub(crate) fn canonical_display(path: &Path) -> String {
    tsv_cli::cli::discover::strip_verbatim_prefix(fs::canonicalize(path).unwrap())
        .display()
        .to_string()
}

static BUILD: Once = Once::new();

/// Path to the built `tsv` binary, built once on first use. Spelled with
/// `EXE_SUFFIX` rather than leaning on Windows' implicit `.exe` resolution,
/// since a test may need the path as a *file* (to copy) and not only as
/// something to spawn.
/// Test helper; panicking on build failure is the desired behavior.
#[allow(clippy::expect_used)]
pub(crate) fn built_tsv() -> PathBuf {
    BUILD.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "-p", "tsv_cli", "-q"])
            .status()
            .expect("Failed to build tsv_cli");
        assert!(status.success(), "tsv_cli build failed");
    });
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("target/debug/tsv{}", std::env::consts::EXE_SUFFIX))
}

/// Run the built `tsv` binary with `cwd` as its working directory — needed for
/// ignore-file tests that pass a relative target like `.` (resolved against the
/// cwd; the format root is then derived from that target, never the cwd itself),
/// since the `cargo run` helper above always runs in the workspace root.
/// Test helper; panicking on spawn failure is the desired behavior.
#[allow(clippy::expect_used)]
pub(crate) fn tsv_in_dir(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(built_tsv())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("Failed to execute tsv binary")
}

pub(crate) const UNFORMATTED_TS: &str = "const   x   =   1;\n";
pub(crate) const FORMATTED_TS: &str = "const x = 1;\n";

/// Create a temp dir that looks like a git repo root — a `.git` marker directory
/// is all `find_repo_root` checks for, so this turns on gitignore-aware discovery
/// without needing a real `git` binary.
/// Test helper; panicking on IO failure is the desired behavior.
#[allow(clippy::unwrap_used)]
pub(crate) fn git_repo(name: &str) -> TempTree {
    let dir = temp_dir(name);
    fs::create_dir(dir.join(".git")).unwrap();
    dir
}

/// A temp tree that is removed when the test leaves — **assertion failure included**.
///
/// `temp_dir` names its directory after the pid, so a leak is never reclaimed by a
/// later run, and a `remove_dir_all` written after the assertions only cleans up the
/// runs that passed: the wrong half, since a failing run is the one a developer
/// re-runs. The pipe tests' trees are 1,200 files each, so the leak is measured in
/// thousands of files per red test. Same shape as `ReleasePoolOnUnwind` in the format
/// command — the cleanup belongs on the way out, not on the happy path.
///
/// Every temp tree in this suite is one, because a guard two call sites use is a
/// convention the other forty do not follow: `temp_dir` returns it, so a test cannot
/// opt out by forgetting. It [`Deref`]s to `Path`, so `&dir` and `dir.join(..)` read
/// exactly as they did against the bare `PathBuf`.
pub(crate) struct TempTree(PathBuf);

impl TempTree {
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for TempTree {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Whether a file's mode bits actually stop **this** process from reading it.
///
/// A probe rather than a uid check, because the two answers come apart in both
/// directions: root reads a `0o000` file, and so does any process holding
/// `CAP_DAC_OVERRIDE` without being root (a container, a CI image that grants it). The
/// probe asks the exact question every permission test depends on — write a file, take
/// its mode away, try to open it — so the tests skip precisely when they would
/// otherwise pass vacuously, with the CLI's unreadable-file arm never reached and its
/// error never produced. Run once per process and remembered.
#[cfg(unix)]
#[allow(clippy::expect_used)]
pub(crate) fn mode_bits_are_enforced() -> bool {
    use std::os::unix::fs::PermissionsExt;

    static ENFORCED: OnceLock<bool> = OnceLock::new();
    *ENFORCED.get_or_init(|| {
        let dir = temp_dir("mode_bits_probe");
        let file = dir.join("probe.txt");
        fs::write(&file, b"probe").expect("write mode probe");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).expect("set probe mode");
        fs::File::open(&file).is_err()
    })
}

/// The mode a [`set_mode`] call applied, restored when the guard leaves scope.
#[cfg(unix)]
pub(crate) struct ModeGuard {
    path: PathBuf,
    previous: fs::Permissions,
}

#[cfg(unix)]
impl Drop for ModeGuard {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.previous.clone());
    }
}

/// Set `path`'s mode to `mode`, restoring the mode it had when the returned guard drops.
///
/// Declare it **after** the [`TempTree`] it sits inside, so it drops first: a `0o000`
/// directory cannot be walked, and the tree's `remove_dir_all` would leave it behind.
/// Where an assertion reads back a file whose mode was taken away, `drop(guard)` ahead
/// of it — the same point a hand-written restore sat at.
#[cfg(unix)]
#[allow(clippy::expect_used)]
pub(crate) fn set_mode(path: &Path, mode: u32) -> ModeGuard {
    use std::os::unix::fs::PermissionsExt;

    let previous = fs::metadata(path)
        .expect("read the mode to restore")
        .permissions();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set mode");
    ModeGuard {
        path: path.to_path_buf(),
        previous,
    }
}
