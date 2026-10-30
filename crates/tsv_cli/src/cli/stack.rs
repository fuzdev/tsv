//! The one stack reservation every tsv thread runs on, and how many such threads tsv asks for.
//!
//! The size, why a reservation is needed at all, and what it costs are the library's to
//! state: [`tsv_lang::STACK_SIZE`] and [`tsv_lang::run_with_stack`] (a stack overflow is not
//! a catchable panic, so for a directory format it means dying partway through with some
//! files already rewritten; the parsers' depth limits are sized for a 2 MiB thread
//! optimized and a 16 MiB one in debug, which an inherited stack does not promise). What
//! this module adds is the CLI's layer over them.
//!
//! **Every route.** Applying the reservation to *every* thread tsv dispatches language work
//! on — the wrapper [`run_on_sized_stack`] puts around the whole subcommand, each format
//! worker, and, in `tsv_debug`, each injection-audit worker and each thread of the tokio
//! runtime its async commands format on — is what makes the stack a property of tsv rather
//! than of the route, the host and the platform. [`sized_thread`] is the single constructor
//! every `thread::Builder` spawn goes through, so one cannot quietly miss the reservation by
//! building its own; the runtime, which builds its own threads by construction, takes
//! [`tsv_lang::STACK_SIZE`] directly. A debug CLI and every audit `tsv_debug` runs over
//! whole corpora (which also read the canonical parsers' trees, bounded by no tsv limit)
//! stay on the safe side of any accepted input. `docs/cli.md` §Recursion Depth states the
//! contract and the margins `tests/depth_limit.rs` holds.
//!
//! **Names, the fallback and the exit code.** Each thread is named (`tsv`, `tsv-format`),
//! since an overflow's one diagnostic is the runtime's line naming the thread. A spawn the
//! OS refuses is warned about and the work runs on the inherited stack rather than failing
//! the run, and a panic that reaches the top exits 101, as an unhandled one would.
//!
//! **How many.** Both `--jobs` flags (the format pool's and the injection audits') are held
//! to [`clamp_worker_count`], stated here for the same reason the size is stated once — a
//! rule with two spellings drifts, and this is the module both pools already go through to
//! build a thread. The reservation is free on a 64-bit target multiplied by any sane
//! count; the ceiling exists for the blast radius (below).

use crate::err_line;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Mutex, PoisonError};
use std::thread;
use tsv_lang::STACK_SIZE;

/// The exit code Rust's runtime uses for an unhandled panic, reproduced here because
/// [`run_on_sized_stack`] *catches* the panic rather than letting it unwind out of
/// `main`.
const PANIC_EXIT_CODE: i32 = 101;

/// Workers per logical CPU an explicit `--jobs` is held to.
///
/// Four is far past anything tsv's workloads can *use*: the format pool's own default
/// (`pool::default_jobs`) deliberately lands **below** the logical count,
/// because the per-file work is memory-bound and a discovery walk competes with the pool
/// for the same cores. So this is not a tuning knob — it is loose enough that no
/// deliberate over-subscription (a slow filesystem, a shared machine, a bench sweep
/// comparing widths) reaches it, and tight enough that a mistyped number cannot ask the
/// OS for every task slot it will give.
///
/// The honest reason for a ceiling at all is **blast radius**, not throughput: each
/// worker reserves [`tsv_lang::STACK_SIZE`] of address space, and an unbounded `--jobs` takes task
/// slots until the OS starts refusing — which on a systemd machine means the login
/// session's whole `TasksMax`, wedging every *other* process on it. A formatter should
/// not be able to do that to a user who mistyped a number.
pub const MAX_WORKERS_PER_LOGICAL_CPU: usize = 4;

/// Hold an explicit `--jobs` to [`MAX_WORKERS_PER_LOGICAL_CPU`] per logical CPU.
///
/// Here rather than beside either pool because **both** `--jobs` flags ask it — `tsv
/// format`'s and every corpus-walking audit's (`tsv_debug`'s `audit::parallel::run_pool`,
/// which the injection audits drive directly and the as-authored ones reach through
/// `audit::sweep`) — and
/// a rule with two spellings is the shape that lets them drift. This module already
/// states how *big* every tsv work thread is; this is how *many* tsv will ask for.
///
/// Said out loud when it bites, never silently: the flag means "use this width", and a
/// run that quietly used a different one is a worse answer than a slower run. Anything
/// at or under the ceiling passes through untouched — including 0, which each pool floors
/// at 1 in its own way, not a value for this function to have an opinion about.
pub fn clamp_worker_count(requested: usize) -> usize {
    let logical = thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let ceiling = logical.saturating_mul(MAX_WORKERS_PER_LOGICAL_CPU);
    if requested > ceiling {
        err_line!("warning: --jobs {requested} exceeds this machine's ceiling; using {ceiling}");
        return ceiling;
    }
    requested
}

/// A [`thread::Builder`] with [`tsv_lang::STACK_SIZE`] reserved and `name` set.
///
/// The name is not decoration: an overflow is uncatchable, so the runtime's
/// `thread '<name>' has overflowed its stack` line is the *only* diagnostic the
/// failure leaves behind, and an unnamed thread prints `<unknown>` there. That line
/// prints the name in full; keep it to 15 bytes all the same, because the copy the OS
/// keeps — what `ps`, `top`, `perf`, a debugger and a core dump show — is truncated
/// past that on Linux.
pub fn sized_thread(name: &str) -> thread::Builder {
    debug_assert!(
        name.len() < 16,
        "thread name `{name}` is over Linux's 15-byte limit, so `ps`, `perf` and a core dump would show it truncated"
    );
    thread::Builder::new()
        .stack_size(STACK_SIZE)
        .name(name.to_owned())
}

/// Run `f` on a thread with [`tsv_lang::STACK_SIZE`] reserved, returning its value and
/// reproducing its exit contract.
///
/// The whole subcommand goes through here, not only the parts known to recurse: a
/// route that stays on the main thread is a route with a different ceiling, and the
/// point of the reservation is that there is exactly one. The thread is
/// [`tsv_lang::run_with_stack`]'s (named `tsv`). `f`'s own `process::exit` calls still
/// exit the process from that thread, so nothing about the exit codes changes; the one
/// path that needs restating is a panic, which the library resumes here after the
/// default hook has already printed it — and only under a `panic = "unwind"` profile,
/// since `abort` kills the process first — and which exits 101 as an unhandled one would.
///
/// A refused spawn runs `f` on this thread instead, and says so: that gives up the
/// reservation (on Windows, for the 1 MiB main-thread stack the reservation exists to
/// escape), and an overflow that follows would otherwise read as
/// `thread 'main' has overflowed its stack` with nothing explaining why the ceiling
/// was the inherited one. A machine that cannot spawn a thread is still not one where
/// refusing to work is the better answer — the format pool answers the same refusal
/// the same way (narrow to however many threads the OS gave, and format on the
/// calling thread if that was none, warning each time), so no route turns a busy
/// machine into a failed run. A refused spawn drops the closure it was handed, so `f`
/// is handed over in a cell the fallback can take it back out of.
pub fn run_on_sized_stack<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    let held = Mutex::new(Some(f));
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        tsv_lang::run_with_stack(|| take_and_run(&held))
    }))
    .unwrap_or_else(|_| std::process::exit(PANIC_EXIT_CODE));
    outcome.unwrap_or_else(|e| {
        err_line!(
            "warning: could not start the sized thread ({e}); running on the inherited stack"
        );
        take_and_run(&held)
    })
}

/// Take the closure out of the cell and run it.
///
/// [`run_on_sized_stack`] reaches this from exactly one of its two sides — the spawn
/// either started, and the spawned thread takes the closure, or it did not, and the
/// caller does — so the cell is full whenever this runs. An empty cell would mean both
/// sides ran, which the spawn result rules out; it is spelled as a panic so that
/// impossibility can never become a silent no-op that formats nothing and exits 0.
///
/// A poisoned lock cannot lose the closure — the only code holding this lock is the
/// take itself, which cannot panic — so the poison is stepped over rather than treated
/// as a failure.
#[expect(
    clippy::expect_used,
    reason = "an empty cell contradicts the spawn result; failing loudly beats a silent no-op"
)]
fn take_and_run<F: FnOnce() -> T, T>(cell: &Mutex<Option<F>>) -> T {
    let f = cell
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .expect("the closure is taken by exactly one side");
    f()
}

#[cfg(test)]
mod tests {
    use super::{MAX_WORKERS_PER_LOGICAL_CPU, clamp_worker_count};

    /// The ceiling holds against any number a caller can type — `usize::MAX` included,
    /// which is what makes it a bound on how many threads tsv will ask the OS for.
    /// Everything at or under it passes through untouched: the clamp is a ceiling, not a
    /// second opinion about the width that was asked for.
    #[test]
    fn clamp_worker_count_holds_the_ceiling_and_passes_everything_under_it() {
        let logical = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        let ceiling = logical * MAX_WORKERS_PER_LOGICAL_CPU;
        assert_eq!(clamp_worker_count(usize::MAX), ceiling);
        assert_eq!(clamp_worker_count(ceiling + 1), ceiling);
        assert_eq!(clamp_worker_count(ceiling), ceiling);
        assert_eq!(clamp_worker_count(1), 1);
        // 0 is a width each pool floors at 1 in its own way, not something to
        // reinterpret here.
        assert_eq!(clamp_worker_count(0), 0);
    }
}
