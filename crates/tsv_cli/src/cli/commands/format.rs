use crate::cli::discover::discover_files;
use crate::cli::format_source::format_source;
use crate::cli::input::{InputArgs, ParserType};
use argh::FromArgs;
use std::fs;
use std::num::NonZeroUsize;
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
/// inside a git repo it honors `.gitignore` (hierarchically, like git) plus a
/// repo-root `.formatignore` / `.prettierignore`; outside one, only
/// `.formatignore`. An explicitly named file is always formatted. `--list`
/// prints the discovered in-scope files without formatting (path mode only).
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

    /// check instead of writing/printing: exit 1 if any input would change
    #[argh(switch)]
    check: bool,

    /// list the discovered in-scope files (one per line) without formatting; path mode only
    #[argh(switch)]
    list: bool,

    /// worker thread count (default: available parallelism)
    #[argh(option)]
    jobs: Option<usize>,

    /// files and/or directories (directories recurse over .ts/.svelte/.css)
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
        match format_source(input.content(), parser_type) {
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
        if self.list && self.check {
            eprintln!("Error: --list and --check cannot be combined");
            process::exit(2);
        }
        let discovered = match discover_files(&self.paths) {
            Ok(discovered) => discovered,
            Err(bad_args) => {
                for msg in bad_args {
                    eprintln!("error: {msg}");
                }
                process::exit(2);
            }
        };
        for msg in &discovered.errors {
            eprintln!("error: {msg}");
        }
        // discovery warnings (e.g. the heuristic-shadow no-op) go to stderr but
        // are NOT errors — no effect on the exit code or stdout, so `--list` /
        // `--check` output stays clean. Fires in every path mode.
        for msg in &discovered.warnings {
            eprintln!("warning: {msg}");
        }
        // --list reports the in-scope set and stops — no formatting, and an
        // empty result is a valid answer (exit 0), unlike the format action
        // below which treats "nothing found" as a usage error.
        if self.list {
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
        let files = discovered.files;
        if files.is_empty() && discovered.errors.is_empty() {
            // neutral wording: an empty result can mean "no .ts/.svelte/.css here"
            // *or* "all of them are ignored" (e.g. a target under a gitignored dir),
            // so don't imply a wrong-extension cause. `--list` reports the empty
            // set on stdout and exits 0 instead.
            eprintln!("Error: No files to format — no unignored .ts/.svelte/.css files in scope");
            process::exit(2);
        }

        let jobs = self
            .jobs
            .unwrap_or_else(|| thread::available_parallelism().map_or(1, NonZeroUsize::get));
        let outcomes = format_files(&files, self.check, jobs);

        let (mut changed, mut unchanged) = (0u32, 0u32);
        let mut errors = discovered.errors.len() as u32;
        for (path, outcome) in files.iter().zip(&outcomes) {
            match outcome {
                FileOutcome::Unchanged => unchanged += 1,
                FileOutcome::Changed => {
                    changed += 1;
                    println!("{}", path.display());
                }
                FileOutcome::Error(e) => {
                    errors += 1;
                    eprintln!("error: {}: {e}", path.display());
                }
            }
        }

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

/// Format one file, writing in place when the output differs (unless `check`).
fn format_file(path: &Path, check: bool) -> FileOutcome {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => return FileOutcome::Error(format!("read failed: {e}")),
    };
    let parser_type = ParserType::from_extension(&path.to_string_lossy());
    // catch_unwind isolates formatter bugs to the file; release builds use
    // panic=abort so this only pays off in dev/corpus profiles
    let result = panic::catch_unwind(AssertUnwindSafe(|| format_source(&source, parser_type)));
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
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut outcomes = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= files.len() {
                            break;
                        }
                        outcomes.push((i, format_file(&files[i], check)));
                    }
                    outcomes
                })
            })
            .collect();
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
