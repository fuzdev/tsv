//! The one place the CLI writes to stdout and stderr.
//!
//! Every byte `tsv` emits goes through here, for one reason: **a consumer that closes
//! the pipe is not a failure.** `println!` and `eprintln!` panic when their write
//! fails, and `tsv format <dir> | head` fails that write by construction — the
//! changed-path report on a large tree exceeds the 64 KiB pipe buffer, so `head` has
//! exited by the time the rest of it is written. Rust sets `SIGPIPE` to `SIG_IGN` at
//! startup, so the write returns `EPIPE` rather than killing the process, and the
//! macro turns that into a panic: exit 134 under `panic = "abort"`, with a panic
//! notice on stderr, *after* every file had already been rewritten — the changes
//! landed and the report of them was replaced by a crash.
//!
//! So a `BrokenPipe` write stops quietly and the run finishes normally. Two things
//! follow, and both are the point:
//!
//! - **The exit code still reports the work** (0 clean, 1 `--check` would-change, 2
//!   errors) rather than the reader. For `--check` the exit code *is* the API, so a
//!   `141`, or a death by signal, would answer a different question than the one
//!   asked. Exiting 0 after `| head` is also the honest answer for `format`: stdout is
//!   a *report* of files already rewritten, not the product, and a reader that left
//!   does not un-format them.
//! - **The stderr summary still prints** when stderr is not the closed fd, so
//!   `tsv format . | head` stays informative.
//!
//! This is the contract `crates/tsv_wasm/npm/cli.js` already holds (its `write_fd`),
//! and the two `tsv` bins are held to one answer. Restoring `SIGPIPE` to `SIG_DFL`
//! instead would reach every fd with no call-site rule at all, but it throws the exit
//! code away, and `cli.js` cannot mirror it — Node ignores `SIGPIPE` too, so the JS
//! side can only fake an exit code where the native side died by a signal, which is a
//! different observable.
//!
//! **Both fds, not only stdout.** `tsv format . 2>&1 | head` closes the same pipe for
//! both, so a stdout-only rule would move the abort one line down, onto the summary.
//!
//! Any *other* write error still panics, exactly as the macros did — a report
//! truncated by a full disk with nothing said about it is the worse failure. The
//! `EAGAIN` retry `cli.js` carries has no counterpart here: that CLI flips its own
//! fd 1 to non-blocking by piping the worker pool's stdio through the parent, where
//! this one's workers are threads and the fd stays blocking.
//!
//! **Scope: the shipped bin.** `tsv_debug` keeps `println!`/`eprintln!` at its ~1,160
//! write sites and still aborts on a closed reader. That is deliberate, not an
//! oversight to finish: it ships in no artifact, its output is a developer's to read,
//! and a panic there names the problem rather than hiding it. The one line of its
//! output that does come through here is `clamp_worker_count`'s `--jobs` warning,
//! because that function is this crate's.

use std::io::{self, Write};

/// Write `bytes` to stdout verbatim, stopping quietly if the consumer closed the pipe.
///
/// Verbatim is what the bulk writers want: the changed-path list, `--list`'s listing
/// and `format --content`'s output are already newline-terminated strings, and
/// `parse`'s wire bytes must not take a UTF-8 round trip.
pub fn write_stdout(bytes: &[u8]) {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    write_or_stop(&mut out, bytes, "stdout");
}

/// Write `bytes` to stderr verbatim, stopping quietly if the consumer closed the pipe.
///
/// The diagnostics reach this through [`err_line!`](crate::err_line) rather than
/// calling it directly.
pub fn write_stderr(bytes: &[u8]) {
    let stderr = io::stderr();
    let mut out = stderr.lock();
    write_or_stop(&mut out, bytes, "stderr");
}

/// `println!` over [`write_stdout`] — the only stdout line-writer in the CLI.
#[macro_export]
macro_rules! out_line {
    ($($arg:tt)*) => {{
        let mut line = format!($($arg)*);
        line.push('\n');
        $crate::cli::out::write_stdout(line.as_bytes());
    }};
}

/// `eprintln!` over [`write_stderr`] — the only stderr line-writer in the CLI.
#[macro_export]
macro_rules! err_line {
    ($($arg:tt)*) => {{
        let mut line = format!($($arg)*);
        line.push('\n');
        $crate::cli::out::write_stderr(line.as_bytes());
    }};
}

/// Write, flush, and treat a consumer that closed the pipe as the end of the output.
///
/// The flush is not redundant: stdout is line-buffered, and every command reaches
/// `process::exit`, which runs none of the runtime cleanup that would otherwise drain
/// it.
#[expect(
    clippy::panic,
    reason = "this is the panic `println!` already raised; a write that failed for any reason but a closed reader would otherwise truncate the output silently"
)]
fn write_or_stop<W: Write>(out: &mut W, bytes: &[u8], name: &str) {
    match out.write_all(bytes).and_then(|()| out.flush()) {
        Ok(()) => {}
        // The consumer went away (`| head`, `| less` quit, a killed log collector).
        // Stop writing; the caller's own exit code still stands.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
        Err(e) => panic!("failed printing to {name}: {e}"),
    }
}
