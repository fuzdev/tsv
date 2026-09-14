use crate::cli::discover::{
    Diagnostics, Discovered, discover_files, discover_into, path_from_sort_key,
};
use crate::cli::format_source::{format_source_in, format_source_with_source_type};
use crate::cli::input::{InputArgs, ParserType, ResolvedInput};
use crate::cli::out::{exit_with_error, path_bytes, path_text, write_stdout};
use crate::cli::pool::{
    FileQueue, QueueSink, ReleasePoolOnUnwind, default_jobs, drain, join_pool, slot_outcomes,
    spawn_pool,
};
use crate::cli::stack::clamp_worker_count;
use crate::err_line;
use argh::FromArgs;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
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
/// Both routes read their slots out of one `slot_outcomes` (`cli/pool.rs`), and an empty
/// slot means one thing: a worker died *outside* `catch_unwind`, which cannot happen in
/// release (`panic = "abort"` kills the process first). `test_format_jobs_zero_means_one`
/// in `tests/cli_tests/` asserts a `--jobs 0` run never produces it.
const WORKER_PANICKED: &str = "worker thread panicked";

/// What path mode does with a file whose output differs from its source: write it back,
/// or only report it (`--check`). Threaded from the flag down to `format_file` as the
/// mode it is, rather than as a `bool` whose meaning each frame restates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FormatMode {
    Write,
    Check,
}

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

impl FileOutcome {
    /// The outcome of a report slot no worker filled ([`WORKER_PANICKED`]).
    fn worker_panicked() -> Self {
        FileOutcome::Error(WORKER_PANICKED.to_string())
    }
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
        let ResolvedInput {
            input,
            parser_type,
            goal,
        } = InputArgs {
            content: self.content,
            stdin: self.stdin,
            parser: self.parser,
            file: None,
        }
        .resolve_with_source_type(self.source_type.as_deref())
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
        let mode = if self.check {
            FormatMode::Check
        } else {
            FormatMode::Write
        };
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
            format_streamed(&self.paths, mode, jobs)
        } else {
            format_collected(&self.paths, mode, jobs)
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
fn format_collected(paths: &[String], mode: FormatMode, jobs: usize) -> Formatted {
    let Discovered { files, diagnostics } =
        discover_files(paths).unwrap_or_else(|bad_args| exit_bad_args(&bad_args));
    report_discovery(&diagnostics);
    exit_if_nothing_in_scope(files.len(), &diagnostics);
    let outcomes = format_files(&files, mode, jobs);
    Formatted {
        files,
        outcomes,
        discovery_errors: diagnostics.errors.len(),
    }
}

/// Stream one directory root into the pool: this thread walks while the workers
/// format, so the walk's wall time hides behind the first files instead of adding
/// to the run. Returns the same `Formatted` as `format_collected` — in sorted-path
/// order — so the caller can't tell which path produced it.
fn format_streamed(paths: &[String], mode: FormatMode, jobs: usize) -> Formatted {
    let queue = FileQueue::new();
    let mut sink = QueueSink::new(&queue);

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
        let worker = || {
            let mut arenas = WorkerArenas::new();
            drain(|| queue.claim(), |path| arenas.format(path, mode))
        };
        let handles = spawn_pool(scope, jobs, worker);

        // this thread is the producer
        let discovery = discover_into(paths, &mut sink);
        queue.finish();
        // ordering the results is this thread's work too, and the pool is still
        // draining, so it costs nothing on the wall
        sink.close();

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
    let order = sink.report_order();
    exit_if_nothing_in_scope(order.len(), &diagnostics);

    // slot by walk index, then read out in key order
    let mut slots = slot_outcomes(
        order.len(),
        claimed
            .into_iter()
            .map(|(i, path, outcome)| (i, (path, outcome))),
    );
    let mut files = Vec::with_capacity(order.len());
    let mut outcomes = Vec::with_capacity(order.len());
    for (key, walk_index) in &order {
        // an empty slot is a worker that died outside catch_unwind (cannot happen in
        // release, which is `panic = "abort"`); the path went with the worker, and its
        // sort key still names it for the report
        let (path, outcome) = slots[*walk_index as usize]
            .take()
            .unwrap_or_else(|| (path_from_sort_key(key), FileOutcome::worker_panicked()));
        files.push(path);
        outcomes.push(outcome);
    }
    Formatted {
        files,
        outcomes,
        discovery_errors: diagnostics.errors.len(),
    }
}

/// One worker's reusable arenas, and the only way to reach [`format_file`] — the work a
/// pool worker hands to `drain` (`cli/pool.rs`) for each file it claims.
///
/// **The rewind is the contract, so it lives here rather than at each caller.** Each
/// `reset()` keeps the largest chunk and rewinds, so only the first file (and any that
/// grow past the high-water mark) pays an alloc — the rest reuse it. The drain loop
/// wants that, and its two predecessors each spelled the setup and the two resets
/// themselves; a loop that forgot one, or an early `continue` past it, would grow a
/// worker's arena monotonically over its whole share of the tree. Owning the pair makes
/// the rewind structural instead of a rule stated in a doc comment.
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
    fn format(&mut self, path: impl AsRef<Path>, mode: FormatMode) -> FileOutcome {
        let outcome = format_file(path.as_ref(), mode, &self.ast, &self.doc);
        self.ast.reset();
        self.doc.reset();
        outcome
    }
}

/// Format one file into the worker's reusable arenas, writing in place when the
/// output differs (in [`FormatMode::Write`]). Reached only through
/// [`WorkerArenas::format`], which rewinds both arenas once this has returned its owned
/// outcome.
fn format_file(
    path: &Path,
    mode: FormatMode,
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
    if mode == FormatMode::Write
        && let Err(e) = fs::write(path, &formatted)
    {
        return FileOutcome::Error(format!("write failed: {e}"));
    }
    FileOutcome::Changed
}

/// Format files in parallel: a shared next-index counter over the sorted list
/// gives dynamic load balancing; each worker returns (index, outcome) pairs so
/// results land in input order without locks.
fn format_files(files: &[PathBuf], mode: FormatMode, jobs: usize) -> Vec<FileOutcome> {
    if files.is_empty() {
        return Vec::new();
    }
    let next = AtomicUsize::new(0);
    let workers = jobs.clamp(1, files.len());
    let claimed = thread::scope(|scope| {
        // the next index off the shared counter — dynamic load balancing with no lock,
        // and results that land in input order without one either
        let worker = || {
            let mut arenas = WorkerArenas::new();
            drain(
                || {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    (i < files.len()).then(|| (i, &files[i]))
                },
                |path| arenas.format(path, mode),
            )
        };
        let handles = spawn_pool(scope, workers, worker);
        join_pool(handles, worker)
    });
    // an empty slot is a worker that died outside catch_unwind (cannot happen in release)
    slot_outcomes(
        files.len(),
        claimed.into_iter().map(|(i, _, outcome)| (i, outcome)),
    )
    .into_iter()
    .map(|outcome| outcome.unwrap_or_else(FileOutcome::worker_panicked))
    .collect()
}
