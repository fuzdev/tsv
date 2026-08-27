//! The one stack reservation every tsv thread runs on.
//!
//! The parser and the printer are recursive descents, so nesting depth costs stack,
//! and a stack overflow is **not** a catchable panic — no `catch_unwind` and no panic
//! contract can turn it into a per-file error the way they do every other failure. It
//! aborts the process, which for a directory format means dying partway through with
//! some files already rewritten.
//!
//! So the ceiling has to be a decision rather than an inheritance. Left alone, every
//! route picks up a different one from its host: the main thread takes the platform
//! default (the process `RLIMIT_STACK` on Unix — commonly 8 MiB, but larger or smaller
//! on any given machine — and 1 MiB on Windows, where the linker writes it into the
//! executable header and nothing at run time can raise it), a spawned thread takes
//! Rust's 2 MiB, and `RUST_MIN_STACK` moves the second but not the first. One binary
//! would then have an 8x depth difference between `tsv format <path>` and
//! `tsv format --content` on the same input on the same Windows machine — a
//! same-output-everywhere problem, not only a robustness one.
//!
//! Stating it once and applying it to **every** thread tsv dispatches language work on —
//! the wrapper [`run_on_sized_stack`] puts around the whole subcommand, each format
//! worker, and, in `tsv_debug`, each injection-audit worker and each thread of the tokio
//! runtime its async commands format on — is what makes the ceiling a property of tsv
//! rather than of the route, the host and the platform. [`sized_thread`] is the single
//! constructor every `thread::Builder` spawn goes through, so one cannot quietly miss the
//! reservation by building its own; the runtime, which builds its own threads by
//! construction, takes [`STACK_SIZE`] directly.
//!
//! The value is chosen so that even the **debug** profile out-reaches the parsers tsv
//! stands in for. Measured on `const x = ((((…1…))));`, the cost is ~1.2 KiB of stack
//! per nesting level in a release build and ~21 KiB in a debug build, where frames are
//! far larger; acorn + `@sveltejs/acorn-typescript` give up at 497 levels and prettier
//! at 805, both through V8's own checked stack limit. 32 MiB clears ~26,900 levels in
//! release and ~1,570 in debug, so no profile of tsv dies before the tools it replaces
//! do. Parens are the cheapest shape to state, not the tightest — nested arrow bodies
//! cost ~4.5 KiB a level, so ~7,300 is the depth every shape clears (the per-construct
//! table is in `docs/cli.md` §Recursion Depth). Nesting is not the only recursion that
//! costs stack, but it is the deepest real code reaches: the deepest file in the tsc
//! corpus nests 69 levels, and the exposure is generated and minified code.
//!
//! The size is a *reservation*, not a commitment: pages are committed lazily on first
//! touch, so it costs address space and ~0 RSS, and nothing the benchmarks measure
//! moves. On a 64-bit target that stays free multiplied by the worker count, and every
//! shipped target is 64-bit. The one limit it is *not* free against is a hard
//! `RLIMIT_AS` (`ulimit -v`), which counts reservations rather than resident pages —
//! container memory limits do not work that way, so this is a nearly-unused knob, but
//! it is the setting under which a wide pool would fail to spawn.
//!
//! One question over: how *many* such threads tsv will ask the OS for. Both `--jobs`
//! flags (the format pool's and the injection audits') are held to
//! [`clamp_worker_count`], stated here for the same reason the size is — a rule with two
//! spellings drifts, and this is the module both pools already go through to build a
//! thread.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

/// Stack reserved for every thread tsv runs language work on.
///
/// See the module docs for why this is stated rather than inherited, and where the
/// number comes from.
pub const STACK_SIZE: usize = 32 * 1024 * 1024;

/// The exit code Rust's runtime uses for an unhandled panic, reproduced here because
/// [`run_on_sized_stack`] *joins* the panic rather than letting it unwind out of
/// `main`.
const PANIC_EXIT_CODE: i32 = 101;

/// Workers per logical CPU an explicit `--jobs` is held to.
///
/// Four is far past anything tsv's workloads can *use*: the format pool's own default
/// (`commands::format::default_jobs`) deliberately lands **below** the logical count,
/// because the per-file work is memory-bound and a discovery walk competes with the pool
/// for the same cores. So this is not a tuning knob — it is loose enough that no
/// deliberate over-subscription (a slow filesystem, a shared machine, a bench sweep
/// comparing widths) reaches it, and tight enough that a mistyped number cannot ask the
/// OS for every task slot it will give.
///
/// The honest reason for a ceiling at all is **blast radius**, not throughput: each
/// worker reserves [`STACK_SIZE`] of address space, and an unbounded `--jobs` takes task
/// slots until the OS starts refusing — which on a systemd machine means the login
/// session's whole `TasksMax`, wedging every *other* process on it. A formatter should
/// not be able to do that to a user who mistyped a number.
pub const MAX_WORKERS_PER_LOGICAL_CPU: usize = 4;

/// Hold an explicit `--jobs` to [`MAX_WORKERS_PER_LOGICAL_CPU`] per logical CPU.
///
/// Here rather than beside either pool because **both** `--jobs` flags ask it — `tsv
/// format`'s and the injection audits' (`tsv_debug`'s `audit::parallel::run_pool`) — and
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
        eprintln!("warning: --jobs {requested} exceeds this machine's ceiling; using {ceiling}");
        return ceiling;
    }
    requested
}

/// A [`thread::Builder`] with [`STACK_SIZE`] reserved and `name` set.
///
/// The name is not decoration: an overflow is uncatchable, so the runtime's
/// `thread '<name>' has overflowed its stack` line is the *only* diagnostic the
/// failure leaves behind, and an unnamed thread prints `<unknown>` there. Keep names
/// under 15 bytes — Linux truncates past that.
pub fn sized_thread(name: &str) -> thread::Builder {
    thread::Builder::new()
        .stack_size(STACK_SIZE)
        .name(name.to_owned())
}

/// Run `f` on a thread with [`STACK_SIZE`] reserved, returning its value and
/// reproducing its exit contract.
///
/// The whole subcommand goes through here, not only the parts known to recurse: a
/// route that stays on the main thread is a route with a different ceiling, and the
/// point of the reservation is that there is exactly one. `f`'s own `process::exit`
/// calls still exit the process from this thread, so nothing about the exit codes
/// changes; the one path that needs restating is a panic, which arrives as a join
/// error after the default hook has already printed it — and only under a `panic =
/// "unwind"` profile, since `abort` kills the process before the join.
///
/// A refused spawn runs `f` on this thread instead. That gives up the reservation, but
/// a machine that cannot spawn a thread is not one where refusing to work is the
/// better answer — and the format pool answers the same refusal the same way (narrow
/// to however many threads the OS gave, and format on the calling thread if that was
/// none), so no route turns a busy machine into a failed run. The closure is consumed
/// by the failed attempt, so it is handed over in a cell the fallback can take it back
/// out of.
pub fn run_on_sized_stack<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let held = Arc::new(Mutex::new(Some(f)));
    let claimed = Arc::clone(&held);
    match sized_thread("tsv").spawn(move || take_and_run(&claimed)) {
        Ok(handle) => handle
            .join()
            .unwrap_or_else(|_| std::process::exit(PANIC_EXIT_CODE)),
        Err(_) => take_and_run(&held),
    }
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
