//! The stride-chunked worker-pool driver both injection audits run their per-file loop under.
//!
//! `gap_audit` and `blank_audit` each walk the fixture corpus on a small thread pool, folding a
//! per-worker tally into one after the join. The loop was byte-identical between them (job-count
//! resolution, stride chunking, join-and-merge) — extracted here so there is one copy, and one
//! home for the panic-safety contract below.

use std::num::NonZero;
use std::path::{Path, PathBuf};

use crate::cli::CliError;

/// The process-global arming bracket every injection audit wraps around its [`run_pool`]
/// call: the print-once comment ledger armed (the per-thread ledgers are thread-local, so
/// arming once covers every worker), optionally the swallow check (`gap_audit` — armed on
/// the SAME format the ledger rides, no extra format), and the default panic hook
/// suppressed (the audits provoke panics on purpose — a formatter crash IS a finding — so
/// the hook's per-panic output is noise).
///
/// RAII: `Drop` restores the hook and disarms the flags — including on the early-return
/// error path out of [`run_pool`], which the hand-rolled bracket this replaces leaked on
/// (documented as immaterial since nothing formats after a failed run, but structural
/// correctness is free here). Callers `drop(armed)` explicitly at the point the audit
/// stops formatting (gap's verify pass formats, so its window is wider), keeping each
/// audit's disarm point deliberate rather than wherever scope happens to end.
pub(crate) struct ArmedRun {
    prev_hook: Option<PanicHook>,
    swallow: bool,
}

/// The boxed hook `std::panic::take_hook` hands back — held for the restore on drop.
type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

impl ArmedRun {
    pub(crate) fn arm(swallow: bool) -> Self {
        tsv_lang::comment_ledger::set_comment_check(true);
        if swallow {
            tsv_lang::doc::swallow::set_swallow_check(true);
        }
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        Self {
            prev_hook: Some(prev_hook),
            swallow,
        }
    }
}

impl Drop for ArmedRun {
    fn drop(&mut self) {
        if let Some(hook) = self.prev_hook.take() {
            std::panic::set_hook(hook);
        }
        tsv_lang::comment_ledger::set_comment_check(false);
        if self.swallow {
            tsv_lang::doc::swallow::set_swallow_check(false);
        }
    }
}

/// Run `per_file` over `files` on a stride-chunked worker pool and return the merged tally.
///
/// `jobs_hint` is the `--jobs N` flag (`None` / `Some(0)` → `available_parallelism`, capped at the
/// file count). Chunking is by **stride**, not contiguous block: fixture sizes cluster by
/// directory and the per-file work is quadratic in file size, so blocks would strand every large
/// file on one worker. Each file is visited by exactly one worker and `merge` must be
/// order-independent (workers finish in arbitrary order), so the merged result is
/// `--jobs`-invariant — the property the audits' snapshots depend on.
///
/// A worker that panics **outside** `per_file`'s own `catch_unwind` (a parse / format / enumerate
/// step the per-injection catches don't cover) is a HARD FAILURE, not a warning: its tally is
/// lost, and a lost tally can silently flip the ratchet verdict — a dropped `new` shape reads as a
/// false pass, a dropped sole instance of a pinned shape as a false stale. A gate that can
/// silently mis-verdict is worse than a loud abort, so this returns [`CliError::Failed`]. (On that
/// path the caller's suppressed panic hook / armed ledger leak until the process exits on the
/// error — immaterial, since nothing formats after.)
pub(crate) fn run_pool<T: Default + Send>(
    files: &[PathBuf],
    jobs_hint: Option<usize>,
    per_file: impl Fn(&Path, &mut T) + Sync,
    merge: impl Fn(&mut T, T),
) -> Result<T, CliError> {
    let jobs = jobs_hint
        .filter(|j| *j > 0)
        .or_else(|| std::thread::available_parallelism().ok().map(NonZero::get))
        .unwrap_or(1)
        .min(files.len());

    let per_file = &per_file;
    let mut total = T::default();
    let mut panicked = false;
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..jobs)
            .map(|worker| {
                scope.spawn(move || {
                    let mut tally = T::default();
                    for path in files.iter().skip(worker).step_by(jobs) {
                        per_file(path, &mut tally);
                    }
                    tally
                })
            })
            .collect();
        for h in handles {
            match h.join() {
                Ok(t) => merge(&mut total, t),
                Err(_) => {
                    eprintln!(
                        "error: a worker thread panicked outside the audit's per-injection catch \
                         — its tally is lost, so the gate verdict would be unsound. Failing the \
                         run."
                    );
                    panicked = true;
                }
            }
        }
    });
    if panicked {
        return Err(CliError::Failed);
    }
    Ok(total)
}
