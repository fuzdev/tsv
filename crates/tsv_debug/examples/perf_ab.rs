//! The measuring driver behind `deno task perf:ab` (`scripts/perf_ab.ts`): one language of a
//! corpus, timed phase for phase the way `tsv_debug profile` times it, printed as one JSON line.
//!
//! **Why an example and not a `profile` flag.** An A/B builds this file into BOTH trees — the
//! candidate and an older base ref, which may be a release tag. The task copies the current
//! checkout's copy over each snapshot before building, so both sides run the same measuring
//! code and only the measured libraries differ. That only works for a file that compiles
//! against an old tree unchanged: an example is discovered by cargo with no manifest or
//! subcommand registration to patch, and this one reads nothing but the language crates' stable
//! entry points (`parse_folded` / `format_folded_in`, the carriage-return fold, `DocArena`).
//! `profile` itself could not be overlaid — it carries every mode the subcommand grew since.
//! A base tree that lacks one of those entry points fails the build, loudly.
//!
//! Usage: `perf_ab <corpus-dir> --lang ts|svelte|css --iterations N [--parse-only]`.
//!
//! - The phases are `profile`'s format mode: the fold and the parse in the parse column, the
//!   format in the format column; one AST `Bump` and one `DocArena` reused across every file and
//!   iteration, reset between them outside the timed regions.
//! - The file set is `profile`'s directory walk (the discovery prune policy, the formattable
//!   extensions, `input_invalid_*` dropped), filtered to the files `--lang` parses with.
//! - `--parse-only` skips the format, so an `instructions:u` count around this process splits by
//!   phase (format = full − parse-only); `--iterations 0` reads the files and times nothing,
//!   which is the run's fixed cost.
//!
//! - The arena resets sit outside the timed regions (as in `profile`, whose wall numbers this
//!   matches), but inside the process an `instructions:u` count covers — so the instruction
//!   column carries the resets (the `DocArena` one lands in format = full − parse-only) and the
//!   wall column does not. Excluding them from a process-level count is not possible, and timing
//!   them would part the wall column from `profile`'s; a change that moves work into or out of a
//!   reset shows in the instruction column alone.
//! - Formatting has no error channel; a formatter panic aborts this process (the release
//!   profile's `panic = "abort"`), which the task reports as a harness failure. The output is
//!   kept (`black_box`) and its length summed, so a run whose formatted output changes between
//!   runs of one binary is caught as nondeterministic.
//!
//! Output (stdout): `{"lang","files","bytes","errors","iterations","parse_us","format_us",
//! "output_bytes"}` — the per-file medians summed over the language, in microseconds, and the
//! formatted bytes of each file's first iteration, summed. A file that fails to parse is
//! counted in `errors` and left out of the sums.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use tsv_cli::cli::input::ParserType;
use tsv_lang::doc::arena::DocArena;
use tsv_lang::printing::normalize_carriage_returns;

struct Args {
    corpus: PathBuf,
    parser_type: ParserType,
    iterations: usize,
    parse_only: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut corpus = None;
    let mut parser_type = None;
    let mut iterations = 10;
    let mut parse_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lang" => {
                parser_type = Some(match args.next().as_deref() {
                    Some("ts") => ParserType::TypeScript,
                    Some("svelte") => ParserType::Svelte,
                    Some("css") => ParserType::Css,
                    other => return Err(format!("--lang takes ts|svelte|css, got {other:?}")),
                });
            }
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
        parser_type: parser_type.ok_or("missing --lang")?,
        iterations,
        parse_only,
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!(
                "perf_ab: {message}\nusage: perf_ab <corpus-dir> --lang ts|svelte|css --iterations N [--parse-only]"
            );
            return ExitCode::from(2);
        }
    };
    if !args.corpus.is_dir() {
        eprintln!(
            "perf_ab: corpus is not a directory: {}",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }
    // The stack reservation every tsv binary states — a deep real file must not overflow here
    // when it formats fine through `profile`.
    tsv_cli::cli::stack::run_on_sized_stack(move || run(&args))
}

fn run(args: &Args) -> ExitCode {
    let mut files = Vec::new();
    collect_files(&args.corpus, &mut files);
    files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    files.retain(|p| {
        !p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("input_invalid"))
            && ParserType::from_extension(&p.to_string_lossy()) == args.parser_type
    });

    let mut arena = bumpalo::Bump::new();
    let mut doc_arena = DocArena::new();
    let mut ok_files = 0usize;
    let mut bytes = 0usize;
    let mut errors = 0usize;
    let mut parse_us = 0.0;
    let mut format_us = 0.0;
    let mut output_bytes = 0usize;
    let mut parse_times = Vec::with_capacity(args.iterations);
    let mut format_times = Vec::with_capacity(args.iterations);

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("perf_ab: read error {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        parse_times.clear();
        format_times.clear();
        let mut failed = false;
        let mut file_output_bytes = 0;
        for iteration in 0..args.iterations {
            let timed = time_once(
                &source,
                args.parser_type,
                args.parse_only,
                &arena,
                &doc_arena,
            );
            // Teardown outside the timed regions, as `profile` does.
            arena.reset();
            doc_arena.reset();
            match timed {
                Ok((parse, format, output_len)) => {
                    if iteration == 0 {
                        file_output_bytes = output_len;
                    }
                    parse_times.push(parse);
                    format_times.push(format);
                }
                Err(e) => {
                    eprintln!("perf_ab: {}: {e}", path.display());
                    failed = true;
                    break;
                }
            }
        }
        if failed {
            errors += 1;
            continue;
        }
        ok_files += 1;
        bytes += source.len();
        output_bytes += file_output_bytes;
        parse_us += median_us(&mut parse_times);
        format_us += median_us(&mut format_times);
    }

    let lang = match args.parser_type {
        ParserType::TypeScript => "ts",
        ParserType::Svelte => "svelte",
        ParserType::Css => "css",
    };
    println!(
        "{{\"lang\":\"{lang}\",\"files\":{ok_files},\"bytes\":{bytes},\"errors\":{errors},\"iterations\":{},\"parse_us\":{parse_us},\"format_us\":{format_us},\"output_bytes\":{output_bytes}}}",
        args.iterations
    );
    ExitCode::SUCCESS
}

/// One iteration of `profile`'s format mode: fold + parse timed as parse, then the format
/// (skipped under `--parse-only`, reading zero); the third value is the formatted length.
fn time_once(
    source: &str,
    parser_type: ParserType,
    parse_only: bool,
    arena: &bumpalo::Bump,
    doc_arena: &DocArena,
) -> Result<(Duration, Duration, usize), String> {
    let parse_error = |e: tsv_lang::ParseError| format!("parse error: {e}");
    let t0 = Instant::now();
    let folded = normalize_carriage_returns(source);
    match parser_type {
        ParserType::TypeScript => {
            let ast = tsv_ts::__private::parse_folded(&folded, None, arena).map_err(parse_error)?;
            let parse = t0.elapsed();
            if parse_only {
                return Ok((parse, Duration::ZERO, 0));
            }
            let t1 = Instant::now();
            let len = formatted_len(tsv_ts::__private::format_folded_in(
                &ast, &folded, doc_arena,
            ));
            Ok((parse, t1.elapsed(), len))
        }
        ParserType::Svelte => {
            let ast = tsv_svelte::__private::parse_folded(&folded, arena).map_err(parse_error)?;
            let parse = t0.elapsed();
            if parse_only {
                return Ok((parse, Duration::ZERO, 0));
            }
            let t1 = Instant::now();
            let len = formatted_len(tsv_svelte::__private::format_folded_in(
                &ast, &folded, doc_arena,
            ));
            Ok((parse, t1.elapsed(), len))
        }
        ParserType::Css => {
            let ast = tsv_css::__private::parse_folded(&folded, arena).map_err(parse_error)?;
            let parse = t0.elapsed();
            if parse_only {
                return Ok((parse, Duration::ZERO, 0));
            }
            let t1 = Instant::now();
            let len = formatted_len(tsv_css::__private::format_folded_in(
                &ast, &folded, doc_arena,
            ));
            Ok((parse, t1.elapsed(), len))
        }
    }
}

/// The formatted output's length, the output kept from the optimizer and dropped here — inside
/// the format timer, where `profile`'s `let _ =` drops it.
fn formatted_len(output: String) -> usize {
    std::hint::black_box(output).len()
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

/// `profile`'s directory walk: recurse, pruning the discovery safety nets, hidden directories
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
