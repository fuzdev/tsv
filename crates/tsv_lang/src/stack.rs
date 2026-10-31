//! The stack tsv's parsers and printers are sized for, and a way to run on one.
//!
//! The parser and the printer are recursive descents, so nesting depth costs stack, and a
//! stack overflow is **not** a catchable panic — no `catch_unwind` can turn it into an
//! error the way it does every other failure. It aborts the process. The parsers' depth
//! limits (`NESTING_LIMIT`, `SPINE_LIMIT`) bound how deep an accepted tree goes, sized so
//! every walk of it fits a 2 MiB thread in an optimized build and a 16 MiB one in a debug
//! build. What this module offers is a thread that has at least that, whatever thread the
//! caller happens to be on: the main thread takes the platform default (the process
//! `RLIMIT_STACK` on Unix, commonly 8 MiB, and 1 MiB on Windows, where the linker writes it
//! into the executable header and nothing at run time can raise it), a spawned thread
//! takes Rust's 2 MiB, and a test thread the harness's 2 MiB — the Windows main thread sits
//! below the optimized contract, and every one of them below a debug build's.
//!
//! [`STACK_SIZE`] clears both contracts with room: 32 MiB is twice the debug build's, so a
//! debug build handed tsv's deepest accepted input — and a tool that also walks other
//! parsers' trees over the same input — stays on the safe side.
//!
//! The size is a *reservation*, not a commitment: pages are committed lazily on first
//! touch, so it costs address space and ~0 RSS, and spawning and joining one thread costs
//! the same (tens of microseconds) from 2 MiB to 32 MiB. On a 64-bit target the address
//! space stays free multiplied by any reasonable thread count. The one limit it is *not*
//! free against is a hard `RLIMIT_AS` (`ulimit -v`), which counts reservations rather than
//! resident pages — container memory limits do not work that way — and it is the setting
//! under which a spawn would be refused.

use std::thread;

/// Stack reserved by [`run_with_stack`]: 32 MiB, twice the debug build's contract.
///
/// Every walk of a tree the parsers accept fits a 2 MiB thread in an optimized build and a
/// 16 MiB one in a debug build; this is the size to reserve on a thread of your own that
/// runs tsv on input it does not control. A reservation costs address space, not memory —
/// pages commit as they are touched.
pub const STACK_SIZE: usize = 32 * 1024 * 1024;

/// The name of the thread [`run_with_stack`] runs on.
///
/// Not decoration: an overflow is uncatchable, so the runtime's `thread '<name>' has
/// overflowed its stack` line is the only diagnostic it leaves, and an unnamed thread
/// prints `<unknown>` there. Within Linux's 15-byte limit, so `ps`, `perf` and a core dump
/// show it whole.
const THREAD_NAME: &str = "tsv";

/// Run `f` on a thread reserving [`STACK_SIZE`] and return its value.
///
/// For a caller whose own thread may be too small for tsv's deepest accepted input: a
/// debug build's tests (the harness runs each on a 2 MiB thread), a Windows main thread
/// (1 MiB), or any thread whose stack you do not set. `f` runs on a scoped thread, so it
/// may borrow from the caller, and the call returns once it has finished.
///
/// A panic in `f` resumes on the calling thread (`std::panic::resume_unwind`), so a
/// `catch_unwind` around this call sees it as it would see a panic in `f` called
/// directly. The panic hook runs once, on the spawned thread (the default hook names it
/// `tsv`); resuming does not run it again.
///
/// ```
/// # use tsv_lang as tsv_ts;
/// let sources = vec![String::from("const x=42;"), String::from("let y")];
/// // `f` borrows `sources`; the thread is joined before the call returns
/// let lengths = tsv_ts::run_with_stack(|| sources.iter().map(String::len).collect::<Vec<_>>())
///     .expect("the OS refused the thread");
/// assert_eq!(lengths, [11, 5]);
/// ```
///
/// # Errors
///
/// Only when the thread cannot be spawned: the OS refuses it (out of task slots, or a hard
/// address-space limit), or the target has no threads at all — on `wasm32-unknown-unknown`
/// (without the threads proposal), whose standard library cannot spawn one, every call
/// returns `Err`. `f` has then been dropped without running; a caller that wants to fall
/// back to the current thread keeps the work reachable outside `f` (builds it again, or
/// hands `f` a cell it can take the work back out of).
pub fn run_with_stack<F, T>(f: F) -> std::io::Result<T>
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    thread::scope(|scope| {
        let handle = thread::Builder::new()
            .name(THREAD_NAME.to_owned())
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, f)?;
        Ok(handle
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload)))
    })
}

#[cfg(test)]
mod tests {
    use super::{STACK_SIZE, run_with_stack};

    #[test]
    fn the_closure_borrows_and_its_value_returns() {
        let mut seen = Vec::new();
        let words = ["a", "b"];
        let n = run_with_stack(|| {
            seen.extend(words.iter().map(|w| w.len()));
            seen.len()
        })
        .expect("spawn");
        assert_eq!(n, 2);
        assert_eq!(seen, [1, 1]);
    }

    #[test]
    fn runs_on_a_named_thread() {
        let name =
            run_with_stack(|| std::thread::current().name().map(str::to_owned)).expect("spawn");
        assert_eq!(name.as_deref(), Some("tsv"));
    }

    /// A panic in the closure is the caller's again, payload intact — not an `Err`, not an
    /// abort, not a panic of the scope's own.
    #[test]
    fn a_panic_resumes_on_the_caller() {
        let caught = std::panic::catch_unwind(|| {
            let _ = run_with_stack(|| -> u8 { std::panic::panic_any(7_u32) });
        })
        .expect_err("the panic must reach the caller");
        assert_eq!(caught.downcast_ref::<u32>(), Some(&7));
    }

    /// The reservation is real: a frame chain far past Rust's 2 MiB default fits.
    #[test]
    fn the_reservation_holds_a_deep_recursion() {
        #[inline(never)]
        fn descend(n: usize) -> usize {
            let pad = std::hint::black_box([0_u8; 1024]);
            if n == 0 {
                return usize::from(pad[0]);
            }
            descend(n - 1) + usize::from(std::hint::black_box(pad)[1])
        }
        // at least 4 MiB of frames: twice a default spawned thread
        let depth = 4 * 1024;
        assert!(depth * 1024 < STACK_SIZE);
        assert_eq!(run_with_stack(|| descend(depth)).expect("spawn"), 0);
    }
}
