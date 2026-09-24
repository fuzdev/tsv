//! The writer's answer to a reader that leaves, drains late, or never blocks: a closed
//! pipe on either fd, a slow consumer, and a non-blocking stdin or stdout.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::common::{FORMATTED_TS, TempTree, UNFORMATTED_TS, built_tsv, temp_dir, tsv_in_dir};

/// A tree big enough to fill a pipe: 1,200 unformatted files whose **names** are long
/// enough that the changed-path report clears the 64 KiB pipe buffer.
///
/// Both numbers are load-bearing and independent. 1,200 files with ordinary names is
/// only ~54 KB of paths and does **not** reproduce the closed-pipe bug — the whole
/// report fits in the buffer, the single write succeeds, and nothing ever sees `EPIPE`.
/// The 150-byte name brings it to ~190 KB. The same pair of thresholds is why the JS
/// mirror's rows (`scripts/test_npm.ts`) are shaped this way.
///
/// A fresh tree per call, because a second run has nothing to report: the files are
/// formatted in place, so the report that has to overflow the pipe exists only once.
#[cfg(unix)]
fn pipe_overflow_tree(name: &str) -> TempTree {
    long_name_tree(name, PIPE_TREE_FILES)
}

/// `count` unformatted files with 150-byte names — the lever the pipe and socket rows
/// turn, since what has to overflow the buffer is the changed-path report. Test
/// helper; panicking on IO failure is the desired behavior.
#[cfg(unix)]
#[allow(clippy::expect_used)]
fn long_name_tree(name: &str, count: usize) -> TempTree {
    let tree = temp_dir(name);
    let pad = "p".repeat(150);
    for i in 0..count {
        fs::write(tree.path().join(format!("{pad}_{i}.ts")), UNFORMATTED_TS)
            .expect("write seed file");
    }
    tree
}

#[cfg(unix)]
const PIPE_TREE_FILES: usize = 1200;

/// A piped run: the shell's output plus the **CLI's own** exit code.
#[cfg(unix)]
struct PipedRun {
    out: std::process::Output,
    /// `tsv`'s own status, which the pipeline otherwise hides behind the last
    /// command's. Captured through a file rather than `PIPESTATUS`, which is a bashism
    /// — `/bin/sh` is dash on most Linux distributions, where it expands to nothing.
    code: i32,
}

/// Run `tsv <tsv_args> <pipeline>` through `sh` with `dir` as the cwd.
///
/// A pipe needs a shell: `std::process::Command` can spawn a child but not hand its
/// stdout to a second process that then *exits early*, which is the condition under
/// test. The status file is a sibling of `dir`, not inside it, so it cannot perturb a
/// tree the run is formatting.
/// Test helper; panicking on spawn failure is the desired behavior.
#[cfg(unix)]
#[allow(clippy::expect_used)]
fn tsv_piped(dir: &Path, tsv_args: &str, pipeline: &str) -> PipedRun {
    let bin = built_tsv();
    let status_file = dir.with_extension("status");
    let _ = fs::remove_file(&status_file);
    let script = format!(
        "{{ '{}' {tsv_args}; echo $? > '{}'; }} {pipeline}",
        bin.display(),
        status_file.display()
    );
    let out = Command::new("sh")
        .args(["-c", &script])
        .current_dir(dir)
        .output()
        .expect("Failed to execute shell");
    let code = fs::read_to_string(&status_file)
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(-1);
    let _ = fs::remove_file(&status_file);
    PipedRun { out, code }
}

/// A panic notice in either stream means the run aborted rather than stopping.
#[cfg(unix)]
fn assert_no_pipe_panic(label: &str, run: &PipedRun) {
    let stderr = String::from_utf8_lossy(&run.out.stderr);
    let stdout = String::from_utf8_lossy(&run.out.stdout);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "{label}: the CLI crashed on a closed pipe: {stderr}"
    );
    assert!(
        !stdout.contains("panicked"),
        "{label}: the CLI crashed on a closed pipe: {stdout}"
    );
}

/// A consumer that exits early (`| head`) stops the output and nothing else.
///
/// Rust sets `SIGPIPE` to `SIG_IGN`, so the write returns `EPIPE` rather than killing
/// the process — and `println!` turns that into a panic (exit 134 under the release
/// profile's `panic = "abort"`, 101 under an unwinding one) *after* every file has
/// already been rewritten: the changes land and the report of them becomes a crash
/// notice. So `EPIPE` stops the write and the run finishes on its own terms, which is
/// what `ls | head` looks like from the caller's side and what the JS mirror does
/// (`cli.js`'s `write_fd`; `scripts/test_npm.ts` carries the twin rows).
///
/// **Both fds**, in two shapes that fail differently: bare `| head` closes stdout while
/// stderr stays on the terminal, grading the changed-path write; `2>&1 | head` closes
/// the *same* pipe for both, grading the summary — the write a stdout-only fix leaves
/// panicking one line further down.
#[cfg(unix)]
#[test]
fn test_format_closed_pipe_consumer_is_not_a_crash() {
    for (label, pipeline) in [
        ("stdout_only", "| head -2 >/dev/null"),
        ("both_fds", "2>&1 | head -2 >/dev/null"),
    ] {
        let tree = pipe_overflow_tree(&format!("closed_pipe_{label}"));
        let run = tsv_piped(tree.path(), "format .", pipeline);
        assert_no_pipe_panic(label, &run);
        assert_eq!(run.code, 0, "{label}: format reported a failure");
        // The report is truncated, not the work: every file is still formatted.
        let formatted = fs::read_dir(tree.path())
            .unwrap()
            .filter(|e| fs::read_to_string(e.as_ref().unwrap().path()).unwrap() == FORMATTED_TS)
            .count();
        assert_eq!(
            formatted, PIPE_TREE_FILES,
            "{label}: files were left unformatted"
        );
    }
}

/// `--list`'s single buffered write is the largest the CLI makes, and it takes the
/// same quiet stop: a listing cut short by `| head` is not an error.
#[cfg(unix)]
#[test]
fn test_format_list_closed_pipe_is_not_a_crash() {
    let tree = pipe_overflow_tree("closed_pipe_list");
    let run = tsv_piped(tree.path(), "format --list .", "| head -2 >/dev/null");
    assert_no_pipe_panic("--list", &run);
    assert_eq!(
        run.code,
        0,
        "--list reported a failure: {}",
        String::from_utf8_lossy(&run.out.stderr)
    );
    // and listed nothing it formatted
    let pad = "p".repeat(150);
    assert_eq!(
        fs::read_to_string(tree.path().join(format!("{pad}_0.ts"))).unwrap(),
        UNFORMATTED_TS
    );
}

/// The closed pipe does not overwrite the exit code, because for `--check` the exit
/// code **is** the API.
///
/// This is why a `BrokenPipe` write goes quiet instead of reporting 141, or restoring
/// `SIGPIPE` to `SIG_DFL` and dying by the signal: either of those answers "the reader
/// left" to a caller that asked "would anything change?".
#[cfg(unix)]
#[test]
fn test_format_check_exit_code_survives_a_closed_pipe() {
    let tree = pipe_overflow_tree("closed_pipe_check");
    let run = tsv_piped(tree.path(), "format --check .", "| head -2 >/dev/null");
    assert_no_pipe_panic("--check", &run);
    assert_eq!(
        run.code,
        1,
        "--check must still report would-change through a closed pipe, stderr: {}",
        String::from_utf8_lossy(&run.out.stderr)
    );
    // And `--check` still wrote nothing.
    let pad = "p".repeat(150);
    assert_eq!(
        fs::read_to_string(tree.path().join(format!("{pad}_0.ts"))).unwrap(),
        UNFORMATTED_TS
    );
}

/// The other end of the same rule: a consumer that is merely **slow** gets everything.
///
/// `EPIPE` is the only write error the CLI absorbs, so a reader that drains late must
/// still receive every line — otherwise "stop quietly" would have become "stop early",
/// which is a silently truncated report rather than a fixed crash.
#[cfg(unix)]
#[test]
fn test_format_slow_pipe_consumer_gets_every_line() {
    let tree = pipe_overflow_tree("slow_pipe");
    let run = tsv_piped(tree.path(), "format .", "| { sleep 1; cat; }");
    assert_no_pipe_panic("slow consumer", &run);
    let stderr = String::from_utf8_lossy(&run.out.stderr);
    assert_eq!(run.code, 0, "the run died: {stderr}");
    assert_eq!(
        String::from_utf8_lossy(&run.out.stdout).lines().count(),
        PIPE_TREE_FILES,
        "the changed-path list was truncated, stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("{PIPE_TREE_FILES} formatted")),
        "stderr: {stderr}"
    );
}

/// `parse`'s wire goes through the same writer, so a closed reader is not a parse error.
///
/// This one never panicked — `parse` wrote through a hand-locked handle and answered a
/// failed write with `exit(1)`, its *parse-error* code, so `tsv parse big.ts | head` was
/// indistinguishable from invalid syntax. One writer for both commands settles it at 0,
/// the answer `cli.js` gives.
#[cfg(unix)]
#[test]
fn test_parse_closed_pipe_is_not_a_parse_error() {
    let tree = temp_dir("parse_closed_pipe");
    let path = tree.path().join("big.ts");
    // Enough statements that the wire clears the 64 KiB pipe buffer by a wide margin.
    use std::fmt::Write as _;
    let mut src = String::new();
    for i in 0..2000 {
        let _ = writeln!(src, "const x{i} = {i};");
    }
    fs::write(&path, &src).unwrap();

    let run = tsv_piped(tree.path(), "parse big.ts", "| head -c 100 >/dev/null");
    assert_no_pipe_panic("parse", &run);
    assert_eq!(
        run.code,
        0,
        "a closed reader is not a parse failure, stderr: {}",
        String::from_utf8_lossy(&run.out.stderr)
    );

    // A real parse error still reports 1 — the closed-pipe answer didn't swallow it.
    fs::write(&path, "const bad = ;\n").unwrap();
    let broken = tsv_in_dir(tree.path(), &["parse", "big.ts"]);
    assert_eq!(broken.status.code(), Some(1));
}

/// A **non-blocking** stdout is a slow consumer, not a failure: the run waits it out and
/// every line arrives.
///
/// Whether the fd blocks is a property of the open file description, which the child
/// shares with its parent — and a Node parent that opens its own piped
/// `process.stdout` while `tsv` runs (a task runner logging beside an async child)
/// flips that description to non-blocking under it. A full pipe then answers `EAGAIN` where a
/// blocking one would park the write, and a bare `write_all` panics on it (exit 134 under
/// `panic = "abort"`) after every file has already been rewritten — the same crash
/// `cli.js`'s `write_fd` guards against. A `UnixStream` pair stands in for the flipped
/// pipe: the child's end is set non-blocking before it is handed over as stdout, the
/// reader holds the other end and drains late, and a socket's send buffer fills and
/// answers `EAGAIN` exactly as a pipe's does — once it is full. A socket buffers
/// ~200 KiB on Linux where a pipe buffers 64 KiB, so this tree is `SOCKET_TREE_FILES`
/// deep rather than `PIPE_TREE_FILES`: the 1,200-file report fits in a socket whole, and
/// the row then passes with the `EAGAIN` arm removed (it was checked; it did). The
/// reader keys on the report itself rather than on a clock: its first byte arrives only
/// once the report's first write has landed and filled the buffer, so however long the
/// formatting took, the writer's next attempt meets a full socket.
/// The read side of the same rule: a non-blocking stdin — flipped under the child by a
/// parent that opens its own piped `process.stdin` — is waited out, not reported as
/// `Resource temporarily unavailable` the moment the writer pauses.
#[cfg(unix)]
#[test]
fn test_format_stdin_non_blocking_is_waited_out_not_a_read_error() {
    use std::io::Write as _;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    let (reader, mut writer) = UnixStream::pair().expect("socket pair");
    reader.set_nonblocking(true).expect("set O_NONBLOCK");
    let child = Command::new(built_tsv())
        .args(["format", "--stdin", "--parser", "typescript"])
        .stdin(Stdio::from(OwnedFd::from(reader)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tsv");
    // the slow writer: the first half, a pause long enough for the child to find the
    // socket empty, then the rest and EOF
    writer.write_all(b"const   x   =").expect("first half");
    std::thread::sleep(std::time::Duration::from_millis(200));
    writer.write_all(b"   1;\n").expect("second half");
    drop(writer);
    let output = child.wait_with_output().expect("wait");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), FORMATTED_TS);
}

#[cfg(unix)]
#[test]
fn test_format_non_blocking_stdout_is_waited_out_not_a_crash() {
    use std::io::Read as _;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    const SOCKET_TREE_FILES: usize = 6000;
    let tree = long_name_tree("non_blocking_stdout", SOCKET_TREE_FILES);
    let (mut reader, writer) = UnixStream::pair().expect("socket pair");
    writer.set_nonblocking(true).expect("set O_NONBLOCK");
    let mut child = Command::new(built_tsv())
        .args(["format", "--check", "."])
        .current_dir(tree.path())
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tsv");
    // the slow consumer: take one byte, which comes only once the report's first write
    // has filled the socket, then leave the rest undrained while the writer retries
    let mut first = [0_u8; 1];
    reader
        .read_exact(&mut first)
        .expect("the report's first byte");
    std::thread::sleep(std::time::Duration::from_millis(200));
    let mut stdout = String::from_utf8(first.to_vec()).expect("an ASCII path byte");
    reader.read_to_string(&mut stdout).expect("drain stdout");
    let status = child.wait().expect("wait");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("piped stderr")
        .read_to_string(&mut stderr)
        .expect("read stderr");

    assert!(
        !stderr.contains("panicked") && !stderr.contains("temporarily unavailable"),
        "the CLI crashed on a non-blocking stdout: {stderr}"
    );
    assert_eq!(
        status.code(),
        Some(1),
        "--check still reports would-change: {stderr}"
    );
    assert_eq!(
        stdout.lines().count(),
        SOCKET_TREE_FILES,
        "the changed-path list was truncated, stderr: {stderr}"
    );
}
