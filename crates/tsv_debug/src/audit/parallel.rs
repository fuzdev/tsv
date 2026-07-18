//! The stride-chunked worker-pool driver both injection audits run their per-file loop under.
//!
//! `gap_audit` and `blank_audit` each walk the fixture corpus on a small thread pool, folding a
//! per-worker tally into one after the join. The loop was byte-identical between them (job-count
//! resolution, stride chunking, join-and-merge) — extracted here so there is one copy, and one
//! home for the panic-safety contract below.

use std::num::NonZero;
use std::path::{Path, PathBuf};

use crate::cli::CliError;

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
