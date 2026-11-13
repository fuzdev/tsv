//! The type strip's measuring driver behind `npm run perf:ab` (`scripts/perf_ab.ts`): the
//! strippable TypeScript of a corpus, timed phase for phase the way `tsv_debug profile --strip`
//! times it, printed as one JSON line.
//!
//! A separate example from `perf_ab.rs` (whose module doc says why the drivers are examples)
//! because it reads an entry point the format driver does not, the strip
//! (`tsv_ts::__private::strip_in`, or `strip_into` with the warm stores): an older base tree
//! without it still builds the format driver, and the task leaves the strip row out for such a
//! tree, saying so, rather than losing every row to a failed build.
//!
//! Usage: `perf_ab_strip <corpus-dir> --iterations N [--parse-only]`.
//!
//! - The file set is `perf_ab.rs`'s directory walk filtered to what a module loader strips —
//!   `.ts` / `.mts` / `.cts`, no declaration file — `profile --strip`'s set.
//! - Each iteration parses the source (`tsv_ts::__private::parse_with_goal_or_fallback_into`,
//!   the goal read from the extension as `tsv strip` reads a path), then strips it
//!   (`tsv_ts::__private::strip_into`, which parses again inside, into the same warm stores, its
//!   edits in the warm arena);
//!   the strip column is the strip's time less the parse's — its walk and blank. Both read
//!   the author's bytes: the strip folds no carriage returns, so the TypeScript format row's
//!   parse (fold + `parse_folded`) is not this one.
//! - The calls sit in an `api` module with one arm per entry-point generation, as
//!   `perf_ab.rs`'s do (its module doc): the current arm parses into stores a
//!   `tsv_ts::__private::WarmStores` handle takes before each timed call and parks after it,
//!   `perf_ab_api = "no_warm_stores"` (a tree with the stores but no such handle) does the same
//!   with the stores in an `Option` of its own, `perf_ab_api = "ts_parse_bump"` (a tree whose
//!   parse still takes the `Bump` beside the stores) does that through the `Bump`, and
//!   `perf_ab_api = "no_stores"`, a tree older than the stores, calls
//!   `parse_with_goal_or_fallback` and `strip_in` over the `Bump` alone. The
//!   take, the park and the arena resets sit outside the timed regions.
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
    let mut warm = api::Warm::default();
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
        let goal = tsv_ts::Goal::from_extension(path);
        parse_times.clear();
        strip_times.clear();
        let mut failed = false;
        let mut file_output = None;
        for iteration in 0..args.iterations {
            let (parse, parsed) = api::parse(&source, goal, &arena, &mut warm);
            arena.reset();
            if !parsed {
                failed = true;
                break;
            }
            parse_times.push(parse);
            if args.parse_only {
                continue;
            }
            let (strip, stripped) = api::strip(&source, goal, &arena, &mut warm);
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

/// `f`'s wall and its result, kept from the optimizer.
fn timed<R>(f: impl FnOnce() -> R) -> (Duration, R) {
    let t0 = Instant::now();
    let result = std::hint::black_box(f());
    (t0.elapsed(), result)
}

/// The entry points of the trees this driver measures: the current tree's, and under a
/// `perf_ab_api` value those of an older generation (`perf_ab.rs`'s module doc says how an arm
/// is chosen).
///
/// Each arm's `Warm` is what it keeps across files and iterations besides the AST `Bump`: the
/// parked stores, taken before each timed region and parked after it, as the bindings' strip
/// keeps them.
#[cfg(not(any(
    perf_ab_api = "no_warm_stores",
    perf_ab_api = "ts_parse_bump",
    perf_ab_api = "no_stores"
)))]
mod api {
    use super::timed;
    use std::time::Duration;

    /// The stores parked between the timed calls, rewound.
    pub(super) type Warm = tsv_ts::__private::WarmStores;

    /// The parse's wall and whether it parsed.
    pub(super) fn parse(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        _arena: &bumpalo::Bump,
        warm: &mut Warm,
    ) -> (Duration, bool) {
        warm.with(|stores| {
            timed(|| {
                tsv_ts::__private::parse_with_goal_or_fallback_into(source, goal, stores).is_ok()
            })
        })
    }

    /// The strip's wall, its own parse included, and what it returned.
    pub(super) fn strip(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        arena: &bumpalo::Bump,
        warm: &mut Warm,
    ) -> (Duration, Result<tsv_ts::Stripped, tsv_ts::StripError>) {
        warm.with(|stores| timed(|| tsv_ts::__private::strip_into(source, goal, arena, stores)))
    }
}

/// The entry points of a tree with the stores but no `WarmStores` handle: as the current arm,
/// the stores kept in an `Option` of the arm's own. Built only against such a tree, never
/// against the current one.
#[cfg(perf_ab_api = "no_warm_stores")]
mod api {
    use super::timed;
    use std::time::Duration;
    use tsv_ts::ast::AstStores;

    /// The stores parked between the timed calls, rewound.
    #[derive(Default)]
    pub(super) struct Warm {
        stores: Option<AstStores<'static>>,
    }

    /// The parse's wall and whether it parsed.
    pub(super) fn parse(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        _arena: &bumpalo::Bump,
        warm: &mut Warm,
    ) -> (Duration, bool) {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let (wall, parsed) = timed(|| {
            tsv_ts::__private::parse_with_goal_or_fallback_into(source, goal, &mut stores).is_ok()
        });
        warm.stores = Some(stores.park());
        (wall, parsed)
    }

    /// The strip's wall, its own parse included, and what it returned.
    pub(super) fn strip(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        arena: &bumpalo::Bump,
        warm: &mut Warm,
    ) -> (Duration, Result<tsv_ts::Stripped, tsv_ts::StripError>) {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let timed = timed(|| tsv_ts::__private::strip_into(source, goal, arena, &mut stores));
        warm.stores = Some(stores.park());
        timed
    }
}

/// The entry points of a tree whose TypeScript parse still takes the `Bump` beside the stores:
/// the parse and the strip into warm stores, both through the `Bump`. Built only against such
/// a tree, never against the current one.
#[cfg(perf_ab_api = "ts_parse_bump")]
mod api {
    use super::timed;
    use std::time::Duration;
    use tsv_ts::ast::AstStores;

    /// The stores parked between the timed calls, rewound.
    #[derive(Default)]
    pub(super) struct Warm {
        stores: Option<AstStores<'static>>,
    }

    /// The parse's wall and whether it parsed.
    pub(super) fn parse(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        arena: &bumpalo::Bump,
        warm: &mut Warm,
    ) -> (Duration, bool) {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let (wall, parsed) = timed(|| {
            tsv_ts::__private::parse_with_goal_or_fallback_into(source, goal, arena, &mut stores)
                .is_ok()
        });
        warm.stores = Some(stores.park());
        (wall, parsed)
    }

    /// The strip's wall, its own parse included, and what it returned.
    pub(super) fn strip(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        arena: &bumpalo::Bump,
        warm: &mut Warm,
    ) -> (Duration, Result<tsv_ts::Stripped, tsv_ts::StripError>) {
        let mut stores: AstStores<'_> = warm.stores.take().unwrap_or_default();
        let timed = timed(|| tsv_ts::__private::strip_into(source, goal, arena, &mut stores));
        warm.stores = Some(stores.park());
        timed
    }
}

/// The entry points of a tree older than the parse stores: the parse and the strip into the
/// `Bump` alone. Built only against such a tree, never against the current one.
#[cfg(perf_ab_api = "no_stores")]
mod api {
    use super::timed;
    use std::time::Duration;

    /// Nothing is kept warm beside the `Bump`.
    #[derive(Default)]
    pub(super) struct Warm;

    /// The parse's wall and whether it parsed.
    pub(super) fn parse(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        arena: &bumpalo::Bump,
        _warm: &mut Warm,
    ) -> (Duration, bool) {
        timed(|| tsv_ts::__private::parse_with_goal_or_fallback(source, goal, arena).is_ok())
    }

    /// The strip's wall, its own parse included, and what it returned.
    pub(super) fn strip(
        source: &str,
        goal: Option<tsv_ts::Goal>,
        arena: &bumpalo::Bump,
        _warm: &mut Warm,
    ) -> (Duration, Result<tsv_ts::Stripped, tsv_ts::StripError>) {
        timed(|| tsv_ts::__private::strip_in(source, goal, arena))
    }
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
