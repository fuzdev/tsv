use crate::cli::commands::parse::parse_goal_arg;
use crate::cli::discover::{Diagnostics, FileSink, discover_files, discover_into, path_sort_key};
use crate::cli::format_source::{format_source_in, format_source_with_goal};
use crate::cli::input::{InputArgs, ParserType};
use crate::cli::stack::{clamp_worker_count, sized_thread};
use argh::FromArgs;
use std::fs;
use std::num::NonZeroUsize;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;

/// Format source code in place (near-Prettier output).
///
/// Paths are formatted in place (written only when the output differs);
/// `--content`/`--stdin` print to stdout. Exit codes: 0 clean, 1 would
/// change (`--check`), 2 errors. Directory discovery is gitignore-aware:
/// inside a git repo it honors `.gitignore` (hierarchically, like git) plus
/// hierarchical `.formatignore` / `.prettierignore`; outside one, only
/// `.formatignore`. An explicitly named file skips the ignore files, but its
/// extension must still be one tsv formats. `--list` prints the discovered
/// in-scope files without formatting (path mode only).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "format")]
pub struct FormatCommand {
    /// content to format, printed to stdout (requires --parser)
    #[argh(option)]
    content: Option<String>,

    /// read from stdin, print to stdout (requires --parser)
    #[argh(switch)]
    stdin: bool,

    /// parser type: svelte | typescript | css (--content/--stdin only; paths use the extension)
    #[argh(option)]
    parser: Option<ParserType>,

    /// parse goal for TypeScript: script | module (default: module).
    /// `--content`/`--stdin` only — file paths are formatted as modules.
    #[argh(option)]
    goal: Option<String>,

    /// check instead of writing/printing: exit 1 if any input would change
    #[argh(switch)]
    check: bool,

    /// list the discovered in-scope files (one per line) without formatting; path mode only
    #[argh(switch)]
    list: bool,

    /// worker thread count (default: 1.5x physical cores; explicit values capped at 4x logical)
    #[argh(option)]
    jobs: Option<usize>,

    /// files and/or directories (directories recurse over .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css)
    #[argh(positional)]
    paths: Vec<String>,
}

/// Per-file result, reported in sorted-path order.
enum FileOutcome {
    Unchanged,
    /// Output differs from source: written in write mode, listed in check mode.
    Changed,
    Error(String),
}

impl FormatCommand {
    pub fn run(self) {
        if self.content.is_some() || self.stdin {
            self.run_single();
        } else {
            self.run_paths();
        }
    }

    /// `--content`/`--stdin` mode — format one input to stdout (or `--check` it).
    fn run_single(self) {
        if !self.paths.is_empty() {
            eprintln!("Error: --content/--stdin cannot be combined with file paths");
            process::exit(2);
        }
        if self.jobs.is_some() {
            eprintln!(
                "Error: --jobs applies to file paths; --content/--stdin format a single input"
            );
            process::exit(2);
        }
        if self.list {
            eprintln!(
                "Error: --list applies to file paths; --content/--stdin format a single input"
            );
            process::exit(2);
        }
        let goal = match parse_goal_arg(self.goal.as_deref()) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(2);
            }
        };
        let input_args = InputArgs {
            content: self.content,
            stdin: self.stdin,
            parser: self.parser,
            file: None,
        };
        let (input, parser_type) = match input_args.resolve() {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(2);
            }
        };
        match format_source_with_goal(input.content(), parser_type, goal) {
            Ok(formatted) => {
                if self.check {
                    if formatted != input.content() {
                        eprintln!("would change");
                        process::exit(1);
                    }
                } else {
                    print!("{formatted}");
                }
            }
            Err(e) => {
                eprintln!("Parse error: {e}");
                process::exit(2);
            }
        }
    }

    /// Path mode — discover files, format in parallel, report sorted.
    fn run_paths(self) {
        if self.paths.is_empty() {
            eprintln!("Error: No input provided. Use a file path, --content, or --stdin");
            process::exit(2);
        }
        if self.parser.is_some() {
            eprintln!(
                "Error: --parser applies to --content/--stdin; file paths use extension detection"
            );
            process::exit(2);
        }
        if self.goal.is_some() {
            eprintln!(
                "Error: --goal applies to --content/--stdin; file paths are formatted as modules"
            );
            process::exit(2);
        }
        if self.list && self.check {
            eprintln!("Error: --list and --check cannot be combined");
            process::exit(2);
        }
        // --list reports the in-scope set and stops — no formatting, and an
        // empty result is a valid answer (exit 0), unlike the format action
        // below which treats "nothing found" as a usage error. It wants the whole
        // set, so it takes the collecting walk.
        if self.list {
            let discovered = match discover_files(&self.paths) {
                Ok(discovered) => discovered,
                Err(bad_args) => exit_bad_args(&bad_args),
            };
            report_discovery(&discovered.errors, &discovered.warnings);
            // build the whole listing and emit it in one write: a per-path
            // `println!` re-locks stdout and re-enters the formatter for each of
            // (potentially thousands of) lines, which dominates `--list` on a large
            // tree; one buffered write is dramatically cheaper.
            use std::fmt::Write as _;
            let mut listing = String::new();
            for path in &discovered.files {
                let _ = writeln!(listing, "{}", path.display());
            }
            print!("{listing}");
            if !discovered.errors.is_empty() {
                process::exit(2);
            }
            return;
        }

        // An explicit width is held to the shared ceiling (`cli::stack`); the default
        // is computed under one already (`default_jobs`).
        let jobs = self.jobs.map_or_else(default_jobs, clamp_worker_count);
        // A single directory root streams: the walk hands files to the pool as it
        // finds them, so it runs *beside* the first files' parse+format instead of
        // in front of an idle pool. Every other shape needs the whole set before
        // formatting — explicit file arguments are trivial to discover, and
        // multiple roots need the canonical-path dedup, which is set-wide (see
        // `discover_into`). Both paths report in sorted-path order.
        let (files, outcomes, discovery_errors) =
            if self.paths.len() == 1 && Path::new(&self.paths[0]).is_dir() {
                format_streamed(&self.paths, self.check, jobs)
            } else {
                format_collected(&self.paths, self.check, jobs)
            };

        // Buffer the changed-path lines and emit them in one write, for the same
        // reason `--list` does (above): a per-path `println!` re-locks stdout and
        // flushes for each of (potentially thousands of) changed files, which
        // dominates `--check` on a large unformatted tree. The common case (few
        // changes) keeps the buffer tiny. Errors stay per-line on stderr (rare,
        // and stderr is for immediate diagnostics).
        use std::fmt::Write as _;
        let (mut changed, mut unchanged) = (0u32, 0u32);
        let mut errors = discovery_errors as u32;
        let mut changed_paths = String::new();
        for (path, outcome) in files.iter().zip(&outcomes) {
            match outcome {
                FileOutcome::Unchanged => unchanged += 1,
                FileOutcome::Changed => {
                    changed += 1;
                    let _ = writeln!(changed_paths, "{}", path.display());
                }
                FileOutcome::Error(e) => {
                    errors += 1;
                    eprintln!("error: {}: {e}", path.display());
                }
            }
        }
        print!("{changed_paths}");

        let action = if self.check {
            "would change"
        } else {
            "formatted"
        };
        let error_note = if errors > 0 {
            format!(", {errors} errors")
        } else {
            String::new()
        };
        eprintln!("{changed} {action}, {unchanged} unchanged{error_note}");

        if errors > 0 {
            process::exit(2);
        }
        if self.check && changed > 0 {
            process::exit(1);
        }
    }
}

/// Report the walk's diagnostics. Errors are counted by the caller; warnings
/// (e.g. the heuristic-shadow no-op) go to stderr but have no effect on the exit
/// code or stdout, so `--list` / `--check` output stays clean.
fn report_discovery(errors: &[String], warnings: &[String]) {
    for msg in errors {
        eprintln!("error: {msg}");
    }
    for msg in warnings {
        eprintln!("warning: {msg}");
    }
}

/// An argument that resolved to neither a file nor a directory fails the whole
/// run before anything is formatted.
fn exit_bad_args(bad_args: &[String]) -> ! {
    for msg in bad_args {
        eprintln!("error: {msg}");
    }
    process::exit(2);
}

/// An empty scope is a usage error for the format action (`--list` reports the
/// empty set and exits 0 instead). Neutral wording: an empty result can mean "no
/// .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css here" *or* "all of them are ignored"
/// (e.g. a target under a gitignored dir), so don't imply a wrong-extension cause.
fn exit_if_nothing_in_scope(file_count: usize, error_count: usize) {
    if file_count == 0 && error_count == 0 {
        eprintln!(
            "Error: No files to format — no unignored .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css files in scope"
        );
        process::exit(2);
    }
}

/// Discover the whole set, then format it: the path for explicit file arguments
/// and for multiple roots, whose canonical-path dedup needs every path in hand.
fn format_collected(
    paths: &[String],
    check: bool,
    jobs: usize,
) -> (Vec<PathBuf>, Vec<FileOutcome>, usize) {
    let discovered = match discover_files(paths) {
        Ok(discovered) => discovered,
        Err(bad_args) => exit_bad_args(&bad_args),
    };
    report_discovery(&discovered.errors, &discovered.warnings);
    exit_if_nothing_in_scope(discovered.files.len(), discovered.errors.len());
    let outcomes = format_files(&discovered.files, check, jobs);
    (discovered.files, outcomes, discovered.errors.len())
}

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
fn default_jobs() -> usize {
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
/// included, and the caller's own thread is the floor under it — both call sites
/// format on it when the pool comes up empty, so "no thread was available" costs
/// parallelism rather than the run.
fn spawn_pool<'scope, F, T>(
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
                    eprintln!(
                        "warning: could not start format workers ({e}); formatting on one thread"
                    );
                } else {
                    eprintln!(
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
struct ReleasePoolOnUnwind<'a>(&'a FileQueue);

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
struct FileQueue {
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
    fn new() -> Self {
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
    fn finish(&self) {
        self.lock().done = true;
        self.ready.notify_all();
    }

    /// Claim the next path, blocking while the walk is still running and the
    /// queue is empty. `None` once the walk is done and the queue is drained.
    fn claim(&self) -> Option<(usize, PathBuf)> {
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
struct QueueSink<'a> {
    queue: &'a FileQueue,
    /// `(sort key, walk index)`, sorted once the walk is done to give the
    /// reporting order.
    keys: Vec<(Vec<u8>, u32)>,
    batch: Vec<PathBuf>,
}

impl FileSink for QueueSink<'_> {
    fn push(&mut self, path: PathBuf) {
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

/// Stream one directory root into the pool: this thread walks while the workers
/// format, so the walk's wall time hides behind the first files instead of adding
/// to the run. Returns the same `(files, outcomes)` pairing as `format_collected`
/// — in sorted-path order — so the caller can't tell which path produced it.
fn format_streamed(
    paths: &[String],
    check: bool,
    jobs: usize,
) -> (Vec<PathBuf>, Vec<FileOutcome>, usize) {
    let queue = FileQueue::new();
    let mut sink = QueueSink {
        queue: &queue,
        keys: Vec::new(),
        batch: Vec::new(),
    };
    // the caller only streams a directory root, which always resolves, so the
    // bad-argument arm is unreachable here — handled anyway rather than asserted
    let mut discovery: Result<Diagnostics, Vec<String>> = Ok(Diagnostics {
        errors: Vec::new(),
        warnings: Vec::new(),
    });
    let mut claimed: Vec<(usize, PathBuf, FileOutcome)> = Vec::new();

    // `--jobs 0` is a width, not an opt-out: a pool of zero leaves every
    // discovered file unclaimed, and each one then reads out through the
    // died-outside-`catch_unwind` arm below as a bogus "worker thread panicked"
    // — a run that formats nothing and blames a worker that never existed.
    // `format_files` (the collected path) already clamps, so without this the
    // same flag works on explicit file arguments and fails on a directory.
    // Clamped only at the bottom: the file count isn't known up front here,
    // which is the whole point of streaming.
    let jobs = jobs.max(1);

    thread::scope(|scope| {
        let _release = ReleasePoolOnUnwind(&queue);
        let handles = spawn_pool(scope, jobs, || drain_queue(&queue, check));

        // this thread is the producer
        discovery = discover_into(paths, &mut sink);
        queue.finish();
        // ordering the results is this thread's work too, and the pool is still
        // draining, so it costs nothing on the wall
        sink.keys.sort_by(|a, b| a.0.cmp(&b.0));

        // A pool the OS refused outright leaves this thread as the only worker. It
        // has already walked, so nothing streams — but the alternative is a run that
        // formats nothing and reports every file as a panic from a worker that never
        // existed (the slot sweep below), which is the lie an unclamped `--jobs 0`
        // would tell. The queue is finished, so this drains what the walk left and stops.
        if handles.is_empty() {
            claimed = drain_queue(&queue, check);
        }

        for handle in handles {
            if let Ok(mut outcomes) = handle.join() {
                claimed.append(&mut outcomes);
            }
        }
    });

    let diagnostics = match discovery {
        Ok(diagnostics) => diagnostics,
        Err(bad_args) => exit_bad_args(&bad_args),
    };
    report_discovery(&diagnostics.errors, &diagnostics.warnings);
    exit_if_nothing_in_scope(sink.keys.len(), diagnostics.errors.len());

    // slot by walk index, then read out in key order
    let mut slots: Vec<Option<(PathBuf, FileOutcome)>> = Vec::new();
    slots.resize_with(sink.keys.len(), || None);
    for (i, path, outcome) in claimed {
        slots[i] = Some((path, outcome));
    }
    let mut files = Vec::with_capacity(sink.keys.len());
    let mut outcomes = Vec::with_capacity(sink.keys.len());
    for (_, walk_index) in &sink.keys {
        // `None` only if a worker died outside catch_unwind (shouldn't happen —
        // and cannot in release, which is `panic = "abort"`). The path went with
        // the worker, so there is nothing to name in the report.
        let (path, outcome) = slots[*walk_index as usize].take().unwrap_or_else(|| {
            (
                PathBuf::new(),
                FileOutcome::Error("worker thread panicked".to_string()),
            )
        });
        files.push(path);
        outcomes.push(outcome);
    }
    (files, outcomes, diagnostics.errors.len())
}

/// One streamed worker's whole life: claim a path, format it, keep the outcome
/// against the index it was claimed at. Returns when the walk is done and the queue
/// is drained.
///
/// A function rather than a closure inside the spawn loop because the producer runs
/// it too when the pool comes up empty — the fallback and the worker cannot drift
/// apart if there is only one of them.
fn drain_queue(queue: &FileQueue, check: bool) -> Vec<(usize, PathBuf, FileOutcome)> {
    // one AST `Bump` and one `DocArena` per worker, reused across its files — see
    // `drain_indexed`, which does the same
    let mut arena = bumpalo::Bump::new();
    let mut doc_arena = tsv_lang::doc::arena::DocArena::new();
    let mut outcomes = Vec::new();
    while let Some((i, path)) = queue.claim() {
        let outcome = format_file(&path, check, &arena, &doc_arena);
        arena.reset();
        doc_arena.reset();
        outcomes.push((i, path, outcome));
    }
    outcomes
}

/// Format one file into the worker's reusable arenas, writing in place when the
/// output differs (unless `check`). Nothing borrowed from `arena`/`doc_arena`
/// escapes, so the caller resets both after this returns (see `format_files`).
fn format_file(
    path: &Path,
    check: bool,
    arena: &bumpalo::Bump,
    doc_arena: &tsv_lang::doc::arena::DocArena,
) -> FileOutcome {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => return FileOutcome::Error(format!("read failed: {e}")),
    };
    let parser_type = ParserType::from_extension(&path.to_string_lossy());
    // catch_unwind isolates formatter bugs to the file; release builds use
    // panic=abort so this only pays off in dev/corpus profiles
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        format_source_in(&source, parser_type, arena, doc_arena)
    }));
    let formatted = match result {
        Ok(Ok(formatted)) => formatted,
        Ok(Err(e)) => return FileOutcome::Error(e),
        Err(_) => return FileOutcome::Error("panic while formatting (internal bug)".to_string()),
    };
    if formatted == source {
        return FileOutcome::Unchanged;
    }
    if !check && let Err(e) = fs::write(path, &formatted) {
        return FileOutcome::Error(format!("write failed: {e}"));
    }
    FileOutcome::Changed
}

/// Format files in parallel: a shared next-index counter over the sorted list
/// gives dynamic load balancing; each worker returns (index, outcome) pairs so
/// results land in input order without locks.
fn format_files(files: &[PathBuf], check: bool, jobs: usize) -> Vec<FileOutcome> {
    if files.is_empty() {
        return Vec::new();
    }
    let next = AtomicUsize::new(0);
    let workers = jobs.clamp(1, files.len());
    let mut merged: Vec<Option<FileOutcome>> = Vec::with_capacity(files.len());
    merged.resize_with(files.len(), || None);

    thread::scope(|scope| {
        let handles = spawn_pool(scope, workers, || drain_indexed(files, check, &next));
        // A pool the OS refused outright leaves this thread as the only worker —
        // otherwise every slot stays `None` and reads out below as a panic from a
        // worker that never existed.
        if handles.is_empty() {
            for (i, outcome) in drain_indexed(files, check, &next) {
                merged[i] = Some(outcome);
            }
        }
        for handle in handles {
            if let Ok(outcomes) = handle.join() {
                for (i, outcome) in outcomes {
                    merged[i] = Some(outcome);
                }
            }
        }
    });

    // None only if a worker died outside catch_unwind (shouldn't happen)
    merged
        .into_iter()
        .map(|outcome| {
            outcome.unwrap_or_else(|| FileOutcome::Error("worker thread panicked".to_string()))
        })
        .collect()
}

/// One collected worker's whole life: take the next index off the shared counter,
/// format that file, keep the pair — dynamic load balancing with no lock, and
/// results that land in input order without one either.
///
/// A function rather than a closure inside the spawn loop because the caller runs it
/// too when the pool comes up empty (see [`spawn_pool`]).
fn drain_indexed(files: &[PathBuf], check: bool, next: &AtomicUsize) -> Vec<(usize, FileOutcome)> {
    let mut outcomes = Vec::new();
    // One AST `Bump` and one `DocArena` per worker, reused across its files: each
    // `reset()` keeps the largest chunk and rewinds, so only the first file (and any
    // that grow past the high-water mark) pays an alloc — the rest reuse it. The
    // per-file AST and doc tree borrow the arenas and are dropped inside
    // `format_file` (which returns an owned outcome), so the `&mut` resets are sound.
    let mut arena = bumpalo::Bump::new();
    let mut doc_arena = tsv_lang::doc::arena::DocArena::new();
    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= files.len() {
            break;
        }
        outcomes.push((i, format_file(&files[i], check, &arena, &doc_arena)));
        arena.reset();
        doc_arena.reset();
    }
    outcomes
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
