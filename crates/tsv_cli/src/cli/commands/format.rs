use crate::cli::discover::{
    Diagnostics, Discovered, FileSink, discover_files, discover_into, path_from_sort_key,
    path_sort_key,
};
use crate::cli::format_source::{format_source_in, format_source_with_source_type};
use crate::cli::input::{InputArgs, ParserType, check_source_type_language, parse_source_type_arg};
use crate::cli::out::{exit_with_error, path_bytes, path_text, write_stdout};
use crate::cli::stack::{clamp_worker_count, sized_thread};
use crate::err_line;
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
/// `.formatignore`. A named file or directory is bounded by the ignore files
/// alone: one they exclude is skipped (quietly for a file a `.formatignore` or
/// `.prettierignore` excludes, with a warning otherwise), and a named file's
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

    /// parse goal for TypeScript: script | module. Unset: parsed as a module, retried
    /// as a script if that fails. `--content`/`--stdin` only; an error with svelte/css.
    #[argh(option)]
    source_type: Option<String>,

    /// check instead of writing/printing: exit 1 if any input would change
    #[argh(switch)]
    check: bool,

    /// list the discovered in-scope files (one per line) without formatting; path mode only
    #[argh(switch)]
    list: bool,

    /// worker thread count (default: sized to this machine, at most 1.5x physical cores; explicit values capped at 4x logical)
    #[argh(option)]
    jobs: Option<usize>,

    /// files and/or directories (directories recurse over .ts/.mts/.cts/.js/.mjs/.cjs/.svelte/.css)
    // TODO: the list above is a hand copy of `tsv_discover::FORMATTABLE_EXTENSIONS` — argh
    // takes help text from the doc comment alone, so it cannot be rendered from the
    // const the way `exit_if_nothing_in_scope`'s message is; a ninth language must
    // update it by hand (as it must `cli.js`'s help)
    #[argh(positional)]
    paths: Vec<String>,
}

/// What a report slot says when no worker ever filled it.
///
/// Two readers reach it — the streamed path's walk-index sweep and the collected path's
/// merge — and both mean the same thing: a worker died *outside* `catch_unwind`, which
/// cannot happen in release (`panic = "abort"` kills the process first). One spelling so
/// the two cannot drift; `test_format_jobs_zero_means_one` in `tests/cli_tests.rs`
/// asserts a `--jobs 0` run never produces it.
const WORKER_PANICKED: &str = "worker thread panicked";

/// What either path-mode route hands back: the in-scope files with their outcomes,
/// index-aligned and in sorted-path order, plus how many traversal errors the walk
/// reported (counted into the summary's error total; the messages were already
/// printed). The caller cannot tell which route produced it — save for one ordering
/// on stderr: the collected route reports the walk's diagnostics before any file is
/// formatted, the streamed one after the last is, since its walk runs beside the pool.
struct Formatted {
    files: Vec<PathBuf>,
    outcomes: Vec<FileOutcome>,
    discovery_errors: usize,
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
            exit_with_error(
                2,
                "Error: --content/--stdin cannot be combined with file paths",
            );
        }
        if self.jobs.is_some() {
            exit_with_error(
                2,
                "Error: --jobs applies to file paths; --content/--stdin format a single input",
            );
        }
        if self.list {
            exit_with_error(
                2,
                "Error: --list applies to file paths; --content/--stdin format a single input",
            );
        }
        let goal = parse_source_type_arg(self.source_type.as_deref())
            .unwrap_or_else(|e| exit_with_error(2, format_args!("Error: {e}")));
        // refused before the input is read: a `--stdin` this would turn away must not
        // first wait on its writer (an open, empty stdin would hang the refusal)
        if let Some(parser_type) = self.parser
            && let Err(e) = check_source_type_language(self.source_type.as_deref(), parser_type)
        {
            exit_with_error(2, format_args!("Error: {e}"));
        }
        let input_args = InputArgs {
            content: self.content,
            stdin: self.stdin,
            parser: self.parser,
            file: None,
        };
        // `--content`/`--stdin` require `--parser`, so the parser resolved here is the flag
        // the check above already graded
        let (input, parser_type) = input_args
            .resolve()
            .unwrap_or_else(|e| exit_with_error(2, format_args!("Error: {e}")));
        let formatted = format_source_with_source_type(input.content(), parser_type, goal)
            .unwrap_or_else(|e| exit_with_error(2, format_args!("Parse error: {e}")));
        if self.check {
            if formatted != input.content() {
                exit_with_error(1, "would change");
            }
        } else {
            write_stdout(formatted.as_bytes());
        }
    }

    /// Path mode — discover files, format in parallel, report sorted.
    fn run_paths(self) {
        if self.paths.is_empty() {
            exit_with_error(
                2,
                "Error: No input provided. Use a file path, --content, or --stdin",
            );
        }
        if self.parser.is_some() {
            exit_with_error(
                2,
                "Error: --parser applies to --content/--stdin; file paths use extension detection",
            );
        }
        // Path mode resolves the source type per file instead of taking one for the
        // whole run: a Svelte or CSS file on the same command line has no source
        // type to honor, and every JS/TS file formats under whichever grammar
        // accepts it unless its own extension settles one
        // (`tsv_ts::Goal::from_extension`, read per file in `format_file`). So a set
        // `--source-type` here asks for something no answer fits, and is refused
        // rather than ignored.
        if self.source_type.is_some() {
            exit_with_error(
                2,
                "Error: --source-type applies to --content/--stdin; file paths take the module grammar, retried as a script",
            );
        }
        if self.list && self.check {
            exit_with_error(2, "Error: --list and --check cannot be combined");
        }
        // `--jobs` sizes the pool that formats; `--list` never spawns one, so a width
        // there is the same category error as with `--content`
        if self.list && self.jobs.is_some() {
            exit_with_error(
                2,
                "Error: --jobs applies to formatting; --list reports the in-scope set without formatting",
            );
        }
        // --list reports the in-scope set and stops — no formatting, and an
        // empty result is a valid answer (exit 0), unlike the format action
        // below which treats "nothing found" as a usage error. It wants the whole
        // set, so it takes the collecting walk.
        if self.list {
            let Discovered { files, diagnostics } =
                discover_files(&self.paths).unwrap_or_else(|bad_args| exit_bad_args(&bad_args));
            report_discovery(&diagnostics);
            // build the whole listing and emit it in one write: a per-path write
            // re-locks stdout and flushes for each of (potentially thousands of)
            // lines, which dominates `--list` on a large tree; one buffered write is
            // dramatically cheaper.
            let mut listing = Vec::new();
            for path in &files {
                listing.extend_from_slice(&path_bytes(path));
                listing.push(b'\n');
            }
            write_stdout(&listing);
            if !diagnostics.errors.is_empty() {
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
        //
        // The `is_dir` here is a second `stat` on the one argument — `classify_args`
        // takes its own inside whichever route this picks — rather than a
        // classification threaded out of discovery: the route has to be chosen before
        // the walk that would classify it, and one extra `stat` on ONE path is cheaper
        // than the API that would avoid it. A race between the two readings is benign:
        // discovery's is the one that decides how the argument is treated, and the only
        // cost of having picked the streaming route for what turns out to be a file is
        // the set-wide dedup it skips, which a single argument cannot need.
        let Formatted {
            files,
            outcomes,
            discovery_errors,
        } = if self.paths.len() == 1 && Path::new(&self.paths[0]).is_dir() {
            format_streamed(&self.paths, self.check, jobs)
        } else {
            format_collected(&self.paths, self.check, jobs)
        };

        // Buffer the changed-path lines and emit them in one write, for the same
        // reason `--list` does (above): a per-path write re-locks stdout and
        // flushes for each of (potentially thousands of) changed files, which
        // dominates `--check` on a large unformatted tree. The common case (few
        // changes) keeps the buffer tiny. Errors stay per-line on stderr (rare,
        // and stderr is for immediate diagnostics).
        let (mut changed, mut unchanged) = (0usize, 0usize);
        let mut errors = discovery_errors;
        let mut changed_paths = Vec::new();
        for (path, outcome) in files.iter().zip(&outcomes) {
            match outcome {
                FileOutcome::Unchanged => unchanged += 1,
                FileOutcome::Changed => {
                    changed += 1;
                    changed_paths.extend_from_slice(&path_bytes(path));
                    changed_paths.push(b'\n');
                }
                FileOutcome::Error(e) => {
                    errors += 1;
                    err_line!("error: {}: {e}", path_text(path));
                }
            }
        }
        write_stdout(&changed_paths);

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
        err_line!("{changed} {action}, {unchanged} unchanged{error_note}");

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
fn report_discovery(diagnostics: &Diagnostics) {
    for msg in &diagnostics.errors {
        err_line!("error: {msg}");
    }
    for msg in &diagnostics.warnings {
        err_line!("warning: {msg}");
    }
}

/// A bad path argument — neither a file nor a directory, or a file whose extension
/// tsv doesn't handle — fails the whole run before anything is formatted.
fn exit_bad_args(bad_args: &[String]) -> ! {
    for msg in bad_args {
        err_line!("error: {msg}");
    }
    process::exit(2);
}

/// An empty scope is a usage error for the format action (`--list` reports the
/// empty set and exits 0 instead). Neutral wording: an empty result can mean "none of
/// the extensions tsv formats are here" *or* "all of them are ignored" (e.g. a target
/// under a gitignored dir), so don't imply a wrong-extension cause.
///
/// The extension list is rendered from `tsv_discover`'s `FORMATTABLE_EXTENSIONS`, the
/// same const the discovery filter reads, so a new language reaches this sentence
/// rather than leaving it naming the old set. `crates/tsv_wasm/npm/cli.js` restates the
/// finished message by hand (as it does `clamp_worker_count`), and the two are compared
/// byte for byte by `scripts/test_napi_npm.ts`'s message-parity suite.
///
/// An empty run every argument accounts for is not the error: when each argument was a
/// named file an ignore rule excluded (`Diagnostics::all_arguments_excluded`), the
/// caller's own files were refused by the caller's own rules — what a pre-commit hook
/// hands over when only ignored files are staged — and the run exits 0. A directory
/// argument the rules exclude keeps the error, so a mis-scoped command still fails loudly.
fn exit_if_nothing_in_scope(file_count: usize, diagnostics: &Diagnostics) {
    if file_count == 0 && diagnostics.errors.is_empty() && !diagnostics.all_arguments_excluded {
        let extensions = tsv_discover::formattable_extension_list("/");
        exit_with_error(
            2,
            format_args!("Error: No files to format — no unignored {extensions} files in scope"),
        );
    }
}

/// Discover the whole set, then format it: the path for explicit file arguments
/// and for multiple roots, whose canonical-path dedup needs every path in hand.
fn format_collected(paths: &[String], check: bool, jobs: usize) -> Formatted {
    let Discovered { files, diagnostics } =
        discover_files(paths).unwrap_or_else(|bad_args| exit_bad_args(&bad_args));
    report_discovery(&diagnostics);
    exit_if_nothing_in_scope(files.len(), &diagnostics);
    let outcomes = format_files(&files, check, jobs);
    Formatted {
        files,
        outcomes,
        discovery_errors: diagnostics.errors.len(),
    }
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
/// included, and the caller's own thread is the floor under it — [`join_pool`] runs
/// the same drain there when the pool comes up empty, so "no thread was available"
/// costs parallelism rather than the run.
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
/// nothing, and its files read out as [`WORKER_PANICKED`] at the caller.
fn join_pool<T>(
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

/// Stream one directory root into the pool: this thread walks while the workers
/// format, so the walk's wall time hides behind the first files instead of adding
/// to the run. Returns the same `Formatted` as `format_collected` — in sorted-path
/// order — so the caller can't tell which path produced it.
fn format_streamed(paths: &[String], check: bool, jobs: usize) -> Formatted {
    let queue = FileQueue::new();
    let mut sink = QueueSink {
        queue: &queue,
        keys: Vec::new(),
        batch: Vec::new(),
    };

    // `--jobs 0` is a width, not an opt-out: a pool of zero leaves every
    // discovered file unclaimed, and each one then reads out through the
    // died-outside-`catch_unwind` arm below as a bogus "worker thread panicked"
    // — a run that formats nothing and blames a worker that never existed.
    // `format_files` (the collected path) already clamps, so without this the
    // same flag works on explicit file arguments and fails on a directory.
    // Clamped only at the bottom: the file count isn't known up front here,
    // which is the whole point of streaming.
    let jobs = jobs.max(1);

    // The scope's two products: the walk's diagnostics and every claimed outcome.
    let (discovery, claimed): (Result<Diagnostics, Vec<String>>, Vec<_>) = thread::scope(|scope| {
        let _release = ReleasePoolOnUnwind(&queue);
        let worker = || drain(check, || queue.claim());
        let handles = spawn_pool(scope, jobs, worker);

        // this thread is the producer
        let discovery = discover_into(paths, &mut sink);
        queue.finish();
        // ordering the results is this thread's work too, and the pool is still
        // draining, so it costs nothing on the wall
        sink.keys.sort_unstable();

        // A pool the OS refused outright leaves this thread as the only worker. It has
        // already walked, so nothing streams — the queue is finished, so the fallback
        // drains what the walk left and stops.
        (discovery, join_pool(handles, worker))
    });

    // the caller only streams a directory root it has just seen resolve, so the
    // bad-argument arm is reached only if the root vanished between that check and the
    // walk's own — handled rather than asserted, and the pool has nothing queued by then
    let diagnostics = discovery.unwrap_or_else(|bad_args| exit_bad_args(&bad_args));
    report_discovery(&diagnostics);
    exit_if_nothing_in_scope(sink.keys.len(), &diagnostics);

    // slot by walk index, then read out in key order
    let mut slots: Vec<Option<(PathBuf, FileOutcome)>> = Vec::new();
    slots.resize_with(sink.keys.len(), || None);
    for (i, path, outcome) in claimed {
        slots[i] = Some((path, outcome));
    }
    let mut files = Vec::with_capacity(sink.keys.len());
    let mut outcomes = Vec::with_capacity(sink.keys.len());
    for (key, walk_index) in &sink.keys {
        // `None` only if a worker died outside catch_unwind (shouldn't happen —
        // and cannot in release, which is `panic = "abort"`). The path went with
        // the worker; its sort key still names it for the report.
        let (path, outcome) = slots[*walk_index as usize].take().unwrap_or_else(|| {
            (
                path_from_sort_key(key),
                FileOutcome::Error(WORKER_PANICKED.to_string()),
            )
        });
        files.push(path);
        outcomes.push(outcome);
    }
    Formatted {
        files,
        outcomes,
        discovery_errors: diagnostics.errors.len(),
    }
}

/// One worker's whole life, on either discovery path: `claim` the next file — a
/// streamed path taken out of the [`FileQueue`], or the next index of a collected
/// list — format it, keep the outcome against the index it was claimed at; stop when
/// `claim` has nothing left. The one loop behind both pools, so the two ways files
/// reach them cannot drift on what happens to a file once claimed — and the loop the
/// calling thread runs itself when the pool comes up empty ([`join_pool`]).
fn drain<P: AsRef<Path>>(
    check: bool,
    mut claim: impl FnMut() -> Option<(usize, P)>,
) -> Vec<(usize, P, FileOutcome)> {
    let mut arenas = WorkerArenas::new();
    let mut outcomes = Vec::new();
    while let Some((i, path)) = claim() {
        let outcome = arenas.format(path.as_ref(), check);
        outcomes.push((i, path, outcome));
    }
    outcomes
}

/// One worker's reusable arenas, and the only way to reach [`format_file`].
///
/// **The rewind is the contract, so it lives here rather than at each caller.** Each
/// `reset()` keeps the largest chunk and rewinds, so only the first file (and any that
/// grow past the high-water mark) pays an alloc — the rest reuse it. The drain loop
/// ([`drain`]) wants that, and its two predecessors each spelled the setup and the two
/// resets themselves; a loop that forgot one, or an early `continue` past it, would
/// grow a worker's arena monotonically over its whole share of the tree. Owning the
/// pair makes the rewind structural instead of a rule stated in a doc comment.
struct WorkerArenas {
    ast: bumpalo::Bump,
    doc: tsv_lang::doc::arena::DocArena,
}

impl WorkerArenas {
    fn new() -> Self {
        Self {
            ast: bumpalo::Bump::new(),
            doc: tsv_lang::doc::arena::DocArena::new(),
        }
    }

    /// Format one file into the arenas, then rewind both.
    ///
    /// The `&mut self` is what makes the resets sound: the per-file AST and doc tree
    /// borrow the arenas and are dropped inside [`format_file`], which returns an owned
    /// [`FileOutcome`], so nothing borrowed from either arena is alive by the time this
    /// returns.
    fn format(&mut self, path: &Path, check: bool) -> FileOutcome {
        let outcome = format_file(path, check, &self.ast, &self.doc);
        self.ast.reset();
        self.doc.reset();
        outcome
    }
}

/// Format one file into the worker's reusable arenas, writing in place when the
/// output differs (unless `check`). Reached only through [`WorkerArenas::format`],
/// which rewinds both arenas once this has returned its owned outcome.
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
    let name = path.to_string_lossy();
    let parser_type = ParserType::from_extension(&name);
    // A path's own extension can settle the goal (`.mjs`/`.mts` are ES modules by
    // name), and where it does the module-then-script fallback has nothing to fall
    // back to — see `tsv_ts::Goal::from_extension`. Everything else stays unnamed and
    // takes the fallback, which is what reaches a legacy sloppy script.
    let goal = tsv_ts::Goal::from_extension(&name);
    // catch_unwind isolates formatter bugs to the file; release builds use
    // panic=abort so this only pays off in dev/corpus profiles
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        format_source_in(&source, parser_type, goal, arena, doc_arena)
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
        // the next index off the shared counter — dynamic load balancing with no lock,
        // and results that land in input order without one either
        let worker = || {
            drain(check, || {
                let i = next.fetch_add(1, Ordering::Relaxed);
                (i < files.len()).then(|| (i, &files[i]))
            })
        };
        let handles = spawn_pool(scope, workers, worker);
        for (i, _, outcome) in join_pool(handles, worker) {
            merged[i] = Some(outcome);
        }
    });

    // None only if a worker died outside catch_unwind (shouldn't happen)
    merged
        .into_iter()
        .map(|outcome| outcome.unwrap_or_else(|| FileOutcome::Error(WORKER_PANICKED.to_string())))
        .collect()
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
