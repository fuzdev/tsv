//! The type strip's measuring driver behind `deno task perf:ab` (`scripts/perf_ab.ts`): the
//! strippable TypeScript of a corpus, timed phase for phase the way `tsv_debug profile --strip`
//! times it, printed as one JSON line.
//!
//! A separate example from `perf_ab.rs` (whose module doc says why the drivers are examples)
//! because it reads an entry point the format driver does not, `tsv_ts::strip_with_goal`: an
//! older base tree without it still builds the format driver, and the task leaves the strip row
//! out for such a tree, saying so, rather than losing every row to a failed build.
//!
//! Usage: `perf_ab_strip <corpus-dir> --iterations N [--parse-only]`.
//!
//! - The file set is `perf_ab.rs`'s directory walk filtered to what a module loader strips —
//!   `.ts` / `.mts` / `.cts`, no declaration file — `profile --strip`'s set.
//! - Each iteration parses the source (`tsv_ts::parse_with_goal_or_fallback`, the goal read from
//!   the extension as `tsv strip` reads a path), then strips it (`tsv_ts::strip_with_goal`, which
//!   parses again inside); the strip column is the strip's time less the parse's — its walk and
//!   blank. Both read the author's bytes: the strip folds no carriage returns, so the TypeScript
//!   format row's parse (fold + `parse_folded`) is not this one.
//! - `--parse-only` skips the strip. Since a full run parses twice per iteration, the task reads
//!   the strip's own `instructions:u` as full − 2 × parse-only + the zero-iteration fixed cost;
//!   `--iterations 0` reads the files and times nothing.
//! - A refusal counts as a strip (the walk ran; `profile` times it the same way) and adds no
//!   output bytes. A file that fails to parse is counted in `errors` and left out of the sums.
//!
//! Output (stdout): `{"lang":"strip","files","bytes","errors","refused","iterations","parse_us",
//! "strip_us","output_bytes"}` — the per-file medians summed, in microseconds, and the stripped
//! bytes of each file's first iteration, summed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

struct Args {
    corpus: PathBuf,
    iterations: usize,
    parse_only: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut corpus = None;
    let mut iterations = 10;
    let mut parse_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--iterations" => {
                let value = args.next().ok_or("--iterations takes a count")?;
                iterations = value
                    .parse()
                    .map_err(|e| format!("--iterations {value}: {e}"))?;
            }
            "--parse-only" => parse_only = true,
            _ if arg.starts_with("--") => return Err(format!("unknown flag {arg}")),
            _ if corpus.is_none() => corpus = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    Ok(Args {
        corpus: corpus.ok_or("missing <corpus-dir>")?,
        iterations,
        parse_only,
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!(
                "perf_ab_strip: {message}\nusage: perf_ab_strip <corpus-dir> --iterations N [--parse-only]"
            );
            return ExitCode::from(2);
        }
    };
    if !args.corpus.is_dir() {
        eprintln!(
            "perf_ab_strip: corpus is not a directory: {}",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }
    tsv_cli::cli::stack::run_on_sized_stack(move || run(&args))
}

fn run(args: &Args) -> ExitCode {
    let mut files = Vec::new();
    collect_files(&args.corpus, &mut files);
    files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    files.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| !n.starts_with("input_invalid") && is_strippable(n))
    });

    let mut arena = bumpalo::Bump::new();
    let mut ok_files = 0usize;
    let mut bytes = 0usize;
    let mut errors = 0usize;
    let mut refused = 0usize;
    let mut parse_us = 0.0;
    let mut strip_us = 0.0;
    let mut output_bytes = 0usize;
    let mut parse_times = Vec::with_capacity(args.iterations);
    let mut strip_times = Vec::with_capacity(args.iterations);

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("perf_ab_strip: read error {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        let goal = path.to_str().and_then(tsv_ts::Goal::from_extension);
        parse_times.clear();
        strip_times.clear();
        let mut failed = false;
        let mut file_output = None;
        for iteration in 0..args.iterations {
            let t0 = Instant::now();
            let parsed = tsv_ts::parse_with_goal_or_fallback(&source, goal, &arena).is_ok();
            let parse = t0.elapsed();
            arena.reset();
            if !parsed {
                failed = true;
                break;
            }
            parse_times.push(parse);
            if args.parse_only {
                continue;
            }
            let t1 = Instant::now();
            let stripped = std::hint::black_box(tsv_ts::strip_with_goal(&source, goal, &arena));
            let strip = t1.elapsed();
            let output = match &stripped {
                Ok(stripped) => Some(stripped.code.len()),
                Err(tsv_ts::StripError::Parse(_)) => {
                    failed = true;
                    break;
                }
                Err(_) => None,
            };
            drop(stripped);
            arena.reset();
            if iteration == 0 {
                file_output = Some(output);
            }
            strip_times.push(strip.saturating_sub(parse));
        }
        if failed {
            eprintln!("perf_ab_strip: {}: parse error", path.display());
            errors += 1;
            continue;
        }
        ok_files += 1;
        bytes += source.len();
        match file_output {
            Some(Some(len)) => output_bytes += len,
            Some(None) => refused += 1,
            None => {}
        }
        parse_us += median_us(&mut parse_times);
        strip_us += median_us(&mut strip_times);
    }

    println!(
        "{{\"lang\":\"strip\",\"files\":{ok_files},\"bytes\":{bytes},\"errors\":{errors},\"refused\":{refused},\"iterations\":{},\"parse_us\":{parse_us},\"strip_us\":{strip_us},\"output_bytes\":{output_bytes}}}",
        args.iterations
    );
    ExitCode::SUCCESS
}

/// `profile --strip`'s filter: TypeScript a module loader strips — `.ts` / `.mts` / `.cts`,
/// `.svelte.ts` included — and not a declaration file, which has no JS to strip to.
fn is_strippable(name: &str) -> bool {
    [".ts", ".mts", ".cts"]
        .iter()
        .any(|ext| name.ends_with(ext))
        && ![".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

/// The median of `durations` in microseconds (the mean of the middle two for an even count;
/// zero for none) — `profile`'s per-file statistic.
fn median_us(durations: &mut [Duration]) -> f64 {
    durations.sort_unstable();
    let len = durations.len();
    if len == 0 {
        return 0.0;
    }
    let us = |d: Duration| d.as_secs_f64() * 1_000_000.0;
    if len % 2 == 1 {
        us(durations[len / 2])
    } else {
        f64::midpoint(us(durations[len / 2 - 1]), us(durations[len / 2]))
    }
}

/// `perf_ab.rs`'s directory walk: recurse, pruning the discovery safety nets, hidden directories
/// and the build-output heuristic; keep the extensions `tsv format` discovers.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.is_dir() {
            if tsv_discover::is_safety_net(name)
                || name.starts_with('.')
                || tsv_discover::HEURISTIC_DIRS.contains(&name)
            {
                continue;
            }
            collect_files(&path, files);
        } else if tsv_discover::is_formattable(name) {
            files.push(path);
        }
    }
}
