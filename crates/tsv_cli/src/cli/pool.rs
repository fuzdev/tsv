//! The worker pool behind `format`'s two path-mode routes: how wide it is by default
//! ([`default_jobs`]), how it comes up and goes down ([`spawn_pool`] / [`join_pool`]),
//! the hand-off a streaming walk feeds it through ([`FileQueue`] / [`QueueSink`]), the
//! claim loop every worker runs ([`drain`]) and the read-out that puts each claimed
//! outcome back at its index ([`slot_outcomes`]). Nothing here knows what a file's
//! outcome *is* — that stays with the command (`commands/format.rs`, whose
//! `WorkerArenas` is the work a claim hands to `drain`) — so the pool reads as one
//! mechanism beside the stack sizing it shares with `main` (`cli/stack.rs`).

use crate::cli::discover::{FileSink, path_sort_key};
use crate::cli::stack::sized_thread;
use crate::err_line;
use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;

/// Worker count when `--jobs` is not given.
///
/// **Not `available_parallelism()`** — that counts *logical* CPUs, and this
/// workload does not scale onto SMT siblings. Two costs compound: the per-file
/// work is memory-bound, so a sibling thread adds far less than a core; and on a
/// large tree the discovery walk is the bottleneck, so every extra worker is
/// competing with the producer for the core it needs. Measured on `tsv format
/// --check` across five synthetic topologies (SMT siblings masked off with
/// `taskset`), one worker per logical CPU costs up to **28%** on walk-bound trees
/// while buying nothing on flat repos.
///
/// `min(logical, ceil(1.5 × physical))` is the width with the lowest worst-case
/// regret over those topologies (mean 3.6% vs 17.8% for the logical count). Note
/// what it does *not* do: on a machine without SMT it returns
/// `available_parallelism()` unchanged, and so does any platform where the sibling
/// count is unreadable — the cap can only lower the worker count, never raise it,
/// so the fallback everywhere else is exactly today's behavior.
pub(crate) fn default_jobs() -> usize {
    let logical = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    // One read, not one per CPU: SMT width is uniform on every machine that has it
    // (a heterogeneous core layout — big.LITTLE — has no SMT at all, so this reads
    // 1 and the cap is inert). Walking every `cpuN` instead would put hundreds of
    // file reads in front of a run that can finish in ten milliseconds.
    let siblings = fs::read_to_string("/sys/devices/system/cpu/cpu0/topology/thread_siblings_list")
        .map_or(1, |list| cpu_list_len(&list));
    let physical = (logical / siblings.max(1)).max(1);
    logical.min((physical * 3).div_ceil(2))
}

/// Length of a Linux CPU-list string (`"0-1"`, `"0,6"`, `"0-3,8-11"`, `"0"`).
/// Any malformed field yields 0 so a surprise format degrades to "no SMT" rather
/// than to a bogus width.
fn cpu_list_len(list: &str) -> usize {
    list.trim()
        .split(',')
        .map(|part| match part.split_once('-') {
            Some((lo, hi)) => match (lo.trim().parse::<usize>(), hi.trim().parse::<usize>()) {
                (Ok(lo), Ok(hi)) if hi >= lo => hi - lo + 1,
                _ => 0,
            },
            None => usize::from(part.trim().parse::<usize>().is_ok()),
        })
        .sum()
}

/// How many discovered paths accumulate before the sink hands them to the pool.
/// Deliberately small — the point of the batch is only to keep a lock acquire
/// and a condvar signal off *every* file, not to build up a backlog; workers
/// should start on the first directory the walk finishes.
const DISCOVERY_BATCH: usize = 8;

/// Bring up at most `jobs` format workers in `scope`, returning however many the
/// OS actually gave.
///
/// [`sized_thread`] is the same constructor `main` runs the whole subcommand through
/// (see `cli::stack`): the pool is one more thread tsv dispatches language work on,
/// not a route with a ceiling of its own.
///
/// **A refused thread narrows the pool; it never fails the run.** `--jobs` is a
/// user-supplied number, so the OS refusing the *n*th thread is an ordinary outcome
/// of an ordinary argument — and `Builder::spawn_scoped`'s `Err` must not reach an
/// `expect` here, which would make this the one `format` argument that answers with a
/// panic where every other bad one exits 2 with a message. On the streamed path it
/// is worse than a crash: the panic unwinds past [`FileQueue::finish`], so every
/// worker already parked on the condvar stays parked, and `thread::scope` joins the
/// pool *before* it resumes a panic — the process hangs holding N thread stacks
/// instead of dying (see [`ReleasePoolOnUnwind`], which covers that gap for any
/// other unwind through the producer).
///
/// Narrowing is safe because the work is *claimed*, not partitioned: however few
/// workers exist drain the whole list between them. It is the answer the JS CLI
/// already gives for the same situation (`crates/tsv_wasm/npm/cli.js`), warning text
/// included, and the caller's own thread is the floor under it — [`join_pool`] runs
/// the same drain there when the pool comes up empty, so "no thread was available"
/// costs parallelism rather than the run.
pub(crate) fn spawn_pool<'scope, F, T>(
    scope: &'scope thread::Scope<'scope, '_>,
    jobs: usize,
    worker: F,
) -> Vec<thread::ScopedJoinHandle<'scope, T>>
where
    F: FnOnce() -> T + Send + Copy + 'scope,
    T: Send + 'scope,
{
    // Deliberately not `with_capacity(jobs)`: `jobs` is whatever the user typed, and
    // reserving for `--jobs 18446744073709551615` aborts on the allocation failure —
    // the same "an argument reaches a fatal" shape one layer down.
    let mut handles = Vec::new();
    for _ in 0..jobs {
        match sized_thread("tsv-format").spawn_scoped(scope, worker) {
            Ok(handle) => handles.push(handle),
            Err(e) => {
                // Same two sentences the JS CLI prints, deliberately word for word:
                // one situation should not read as two different failures depending
                // on which `tsv` the caller invoked.
                if handles.is_empty() {
                    err_line!(
                        "warning: could not start format workers ({e}); formatting on one thread"
                    );
                } else {
                    err_line!(
                        "warning: only {} of {jobs} format workers started",
                        handles.len()
                    );
                }
                break;
            }
        }
    }
    handles
}

/// Every outcome the pool produced, in no particular order — and when [`spawn_pool`]
/// came up empty, `fallback`'s: the calling thread runs the same drain the workers
/// would have, so a refused pool costs parallelism rather than the run (or, worse, a
/// run that formats nothing and reads every unclaimed file out as a panic from a
/// worker that never existed). The one join for both discovery paths, so the fallback
/// cannot drift between them; a worker that died outside `catch_unwind` contributes
/// nothing, and its files read out as `WORKER_PANICKED` at the caller ([`slot_outcomes`]).
pub(crate) fn join_pool<T>(
    handles: Vec<thread::ScopedJoinHandle<'_, Vec<T>>>,
    fallback: impl FnOnce() -> Vec<T>,
) -> Vec<T> {
    let mut outcomes = if handles.is_empty() {
        fallback()
    } else {
        Vec::new()
    };
    for handle in handles {
        if let Ok(mut claimed) = handle.join() {
            outcomes.append(&mut claimed);
        }
    }
    outcomes
}

/// Releases the pool if the producer unwinds.
///
/// Every parked worker is waiting for [`FileQueue::finish`], and `thread::scope`
/// joins the pool before it resumes a panic — so a producer that dies before calling
/// it hangs the process on its own workers rather than crashing, holding every
/// worker's stack reservation until something kills it. Release builds are
/// `panic = "abort"` and never unwind here; the dev and `corpus` profiles do, and
/// `corpus` is what whole-tree audit sweeps run.
///
/// [`FileQueue::finish`] is idempotent (a second `done = true` plus a `notify_all`
/// nobody is parked for), so the happy path's explicit call stands and this only ever
/// fires on the way out.
pub(crate) struct ReleasePoolOnUnwind<'a>(pub(crate) &'a FileQueue);

impl Drop for ReleasePoolOnUnwind<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// The hand-off between the discovery walk and the format workers.
///
/// Paths arrive in walk order and are claimed by index, so the reporting order is
/// recovered by sorting afterwards rather than by the order work is handed out —
/// `format` promises sorted-path *output*, not sorted-path execution.
pub(crate) struct FileQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
}

struct QueueState {
    /// Discovered paths in walk order. A worker claiming index `i` takes the
    /// `PathBuf` out of its slot and hands it back with the outcome, so the path
    /// is never copied and the lock is held for a pointer swap.
    queued: Vec<PathBuf>,
    /// Index of the next unclaimed path.
    next: usize,
    /// Workers parked on `ready`. Lets the walk skip the signal entirely while
    /// the pool is saturated, which is the steady state on any real tree.
    waiting: usize,
    /// The walk is finished — a worker that finds nothing left can exit.
    done: bool,
}

impl FileQueue {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(QueueState {
                queued: Vec::new(),
                next: 0,
                waiting: 0,
                done: false,
            }),
            ready: Condvar::new(),
        }
    }

    /// Poisoning can only come from a panic while the lock is held, and nothing
    /// under it can panic (the format work happens outside it) — so recovering
    /// the guard is strictly better than turning a worker's death into every
    /// other worker's death.
    fn lock(&self) -> MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push_batch(&self, batch: &mut Vec<PathBuf>) {
        if batch.is_empty() {
            return;
        }
        let added = batch.len();
        let waiting = {
            let mut state = self.lock();
            state.queued.append(batch);
            state.waiting
        };
        // Wake exactly as many workers as there is new work for. A `notify_all`
        // here is correct, and is fine while the pool is saturated (it then finds
        // nobody parked), but it is badly wrong in the opposite regime: when the
        // *walk* is the bottleneck — a big tree with a small in-scope set, e.g.
        // the Svelte and prettier repos, where discovery is 63%/67% of the run —
        // every batch finds all N workers parked, so `notify_all` pays N wakeups
        // to hand out `added` files and N−added of them park again having done
        // nothing. Measured on those two repos, that alone was worth +14% and
        // +12% against the collecting path this replaces. Under-waking is safe: a
        // worker that misses a batch is by definition busy and comes back to the
        // queue when it finishes, and `finish` wakes everyone unconditionally.
        for _ in 0..waiting.min(added) {
            self.ready.notify_one();
        }
    }

    /// No more paths are coming; wake every parked worker so it can exit.
    pub(crate) fn finish(&self) {
        self.lock().done = true;
        self.ready.notify_all();
    }

    /// Claim the next path, blocking while the walk is still running and the
    /// queue is empty. `None` once the walk is done and the queue is drained.
    pub(crate) fn claim(&self) -> Option<(usize, PathBuf)> {
        let mut state = self.lock();
        loop {
            if state.next < state.queued.len() {
                let i = state.next;
                state.next += 1;
                return Some((i, std::mem::take(&mut state.queued[i])));
            }
            if state.done {
                return None;
            }
            state.waiting += 1;
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
            state.waiting -= 1;
        }
    }
}

/// Feeds the pool as the walk finds files, and takes each path's sort key on the
/// way past. Building the keys here costs what `sort_by_cached_key` would have
/// cost anyway, but it lets the *whole* ordering step happen on the walk's thread
/// while the pool is still formatting, instead of in front of it.
pub(crate) struct QueueSink<'a> {
    queue: &'a FileQueue,
    /// `(sort key, walk index)`, ordered by [`Self::close`] once the walk is done to give
    /// the reporting order.
    keys: Vec<(Vec<u8>, u32)>,
    batch: Vec<PathBuf>,
}

impl<'a> QueueSink<'a> {
    pub(crate) fn new(queue: &'a FileQueue) -> Self {
        Self {
            queue,
            keys: Vec::new(),
            batch: Vec::new(),
        }
    }

    /// The walk is over: put the keys in reporting order. Called on the walk's thread
    /// while the pool is still draining, so the sort costs nothing on the wall — which is
    /// why it is a step of its own rather than part of [`Self::report_order`], read once
    /// the pool has been joined.
    pub(crate) fn close(&mut self) {
        self.keys.sort_unstable();
    }

    /// Every discovered file as `(sort key, walk index)`, in reporting order.
    pub(crate) fn report_order(self) -> Vec<(Vec<u8>, u32)> {
        debug_assert!(
            self.keys.is_sorted(),
            "`close` orders the keys before the report reads them"
        );
        self.keys
    }
}

impl FileSink for QueueSink<'_> {
    fn push(&mut self, path: PathBuf) {
        // the walk index is a `u32` to keep the key small; past 2³² files it would alias
        debug_assert!(
            self.keys.len() < u32::MAX as usize,
            "more discovered files than a u32 walk index can address"
        );
        self.keys
            .push((path_sort_key(&path), self.keys.len() as u32));
        self.batch.push(path);
        if self.batch.len() >= DISCOVERY_BATCH {
            self.flush();
        }
    }

    fn flush(&mut self) {
        self.queue.push_batch(&mut self.batch);
    }
}

/// One worker's whole life, on either discovery path: `claim` the next item — a streamed
/// path taken out of the [`FileQueue`], or the next index of a collected list — hand it
/// to `work`, keep the outcome against the index it was claimed at; stop when `claim` has
/// nothing left. The one loop behind both pools, so the two ways files reach them cannot
/// drift on what happens to a file once claimed — and the loop the calling thread runs
/// itself when the pool comes up empty ([`join_pool`]).
pub(crate) fn drain<P, T>(
    mut claim: impl FnMut() -> Option<(usize, P)>,
    mut work: impl FnMut(&P) -> T,
) -> Vec<(usize, P, T)> {
    let mut outcomes = Vec::new();
    while let Some((i, item)) = claim() {
        let outcome = work(&item);
        outcomes.push((i, item, outcome));
    }
    outcomes
}

/// Every claimed outcome placed at the index it was claimed for, in a `len`-long list —
/// `None` where no worker ever filled a slot, which the caller names
/// (`WORKER_PANICKED` in `commands/format.rs`). The one read-out for both routes: the
/// streamed one slots by walk index and then reads the slots out in sort-key order, the
/// collected one slots by list index, and neither spells the fill on its own.
pub(crate) fn slot_outcomes<T>(
    len: usize,
    claimed: impl IntoIterator<Item = (usize, T)>,
) -> Vec<Option<T>> {
    let mut slots: Vec<Option<T>> = Vec::with_capacity(len);
    slots.resize_with(len, || None);
    for (i, outcome) in claimed {
        slots[i] = Some(outcome);
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::{cpu_list_len, default_jobs};

    #[test]
    fn cpu_list_len_counts_ranges_and_singletons() {
        assert_eq!(cpu_list_len("0"), 1); // no SMT
        assert_eq!(cpu_list_len("0-1"), 2); // the common SMT pair
        assert_eq!(cpu_list_len("0,6"), 2); // siblings numbered apart
        assert_eq!(cpu_list_len("0-3,8-11"), 8); // 4-way SMT, split numbering
        assert_eq!(cpu_list_len(" 0-1 \n"), 2); // sysfs writes a trailing newline
    }

    /// A shape this doesn't understand must read as "no SMT", which makes the cap
    /// inert and leaves `available_parallelism()` in charge — never a bogus width.
    #[test]
    fn cpu_list_len_degrades_to_zero_on_junk() {
        assert_eq!(cpu_list_len(""), 0);
        assert_eq!(cpu_list_len("garbage"), 0);
        assert_eq!(cpu_list_len("3-1"), 0); // reversed range
        assert_eq!(cpu_list_len("0-"), 0);
    }

    /// The cap can only ever lower the worker count, on any machine.
    #[test]
    fn default_jobs_never_exceeds_available_parallelism() {
        let logical = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        let jobs = default_jobs();
        assert!(
            jobs >= 1 && jobs <= logical,
            "jobs={jobs} logical={logical}"
        );
    }
}
