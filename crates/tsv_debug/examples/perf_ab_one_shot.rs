//! The one-shot driver behind `npm run perf:ab -- --one-shot` (`scripts/perf_ab.ts`): one
//! language of a corpus timed through the public free functions a library caller reaches, each
//! a whole call over fresh stores, printed as one JSON line in the shape of the warm driver for
//! that language (`perf_ab.rs`, `perf_ab_strip.rs`).
//!
//! **Why an example of its own.** The warm drivers measure the warm path, and the task grades
//! them; these rows are informational. Linking the public functions into a warm driver's binary
//! moves the code the warm path runs, since the whole program is optimized as one unit and the
//! library functions both reach are then shaped for more callers. Kept apart, the warm drivers
//! build the same binaries with or without these rows. Its calls have the same signatures in
//! every tree the task measures, so it has no arm per generation; a tree without the public
//! strip or import records fails its build, loudly.
//!
//! Usage: `perf_ab_one_shot <corpus-dir> --lang ts|svelte|css|strip --iterations N [--parse-only]`.
//!
//! - `ts`, `svelte`, `css`: the format driver's file set; `tsv_<lang>::parse_to_json` timed as
//!   the parse column and `tsv_<lang>::format` as the format column, each output dropped inside
//!   its timer. TypeScript's `parse_to_json` parses a module and never retries as a script, so a
//!   file only a script parses counts in `errors` here, where the warm path's format parse
//!   retries it.
//! - `strip`: the strip driver's file set; `tsv_ts::parse_imports` timed as the parse column and
//!   `tsv_ts::strip` as the strip column (their `_with_goal` forms where the extension names the
//!   goal), each output dropped inside its timer. The strip column is the whole call, its own
//!   parse included, so its `instructions:u` is full − parse-only. A refusal counts as a strip
//!   and adds no output bytes.
//! - `--parse-only` skips the second call; `--iterations 0` reads the files and times nothing.
//!   A file that fails to parse is counted in `errors` and left out of the sums.
//!
//! Output (stdout): the warm driver's line for the language — `{"lang","files","bytes","errors",
//! "iterations","parse_us","format_us","output_bytes"}`, or for `strip` `{"lang":"strip",
//! "files","bytes","errors","refused","iterations","parse_us","strip_us","output_bytes"}`.

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use tsv_cli::cli::input::ParserType;

/// A language of the format driver, or the type strip.
#[derive(Clone, Copy)]
enum Lang {
    Format(ParserType),
    Strip,
}

struct Args {
    corpus: PathBuf,
    lang: Lang,
    iterations: usize,
    parse_only: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut corpus = None;
    let mut lang = None;
    let mut iterations = 10;
    let mut parse_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lang" => {
                lang = Some(match args.next().as_deref() {
                    Some("ts") => Lang::Format(ParserType::TypeScript),
                    Some("svelte") => Lang::Format(ParserType::Svelte),
                    Some("css") => Lang::Format(ParserType::Css),
                    Some("strip") => Lang::Strip,
                    other => {
                        return Err(format!("--lang takes ts|svelte|css|strip, got {other:?}"));
                    }
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
        lang: lang.ok_or("missing --lang")?,
        iterations,
        parse_only,
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!(
                "perf_ab_one_shot: {message}\nusage: perf_ab_one_shot <corpus-dir> --lang ts|svelte|css|strip --iterations N [--parse-only]"
            );
            return ExitCode::from(2);
        }
    };
    if !args.corpus.is_dir() {
        eprintln!(
            "perf_ab_one_shot: corpus is not a directory: {}",
            args.corpus.display()
        );
        return ExitCode::from(2);
    }
    // The stack reservation every tsv binary states, as the warm drivers take it.
    tsv_cli::cli::stack::run_on_sized_stack(move || match args.lang {
        Lang::Format(parser_type) => run_format(&args, parser_type),
        Lang::Strip => run_strip(&args),
    })
}

/// The files the warm driver for the language reads: `perf_ab.rs`'s walk, filtered as it
/// filters it, or as `perf_ab_strip.rs` does for the strip.
fn corpus_files(args: &Args) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(&args.corpus, &mut files);
    files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    files.retain(|p| {
        let name = p.file_name().and_then(|n| n.to_str());
        match args.lang {
            Lang::Format(parser_type) => {
                !name.is_some_and(|n| n.starts_with("input_invalid"))
                    && ParserType::from_extension(&p.to_string_lossy()) == parser_type
            }
            Lang::Strip => {
                name.is_some_and(|n| !n.starts_with("input_invalid") && is_strippable(n))
            }
        }
    });
    files
}

/// One iteration's parse and work walls and the output's length (both zero under
/// `--parse-only`), or why the file failed.
type Timed = Result<(Duration, Duration, usize), String>;

fn run_format(args: &Args, parser_type: ParserType) -> ExitCode {
    let mut ok_files = 0usize;
    let mut bytes = 0usize;
    let mut errors = 0usize;
    let mut parse_us = 0.0;
    let mut format_us = 0.0;
    let mut output_bytes = 0usize;
    let mut parse_times = Vec::with_capacity(args.iterations);
    let mut format_times = Vec::with_capacity(args.iterations);
    for path in &corpus_files(args) {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("perf_ab_one_shot: read error {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        parse_times.clear();
        format_times.clear();
        let mut failed = false;
        let mut file_output_bytes = 0;
        for iteration in 0..args.iterations {
            match time_format(&source, parser_type, args.parse_only) {
                Ok((parse, format, output_len)) => {
                    if iteration == 0 {
                        file_output_bytes = output_len;
                    }
                    parse_times.push(parse);
                    format_times.push(format);
                }
                Err(e) => {
                    eprintln!("perf_ab_one_shot: {}: {e}", path.display());
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
    let lang = match parser_type {
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

/// One iteration: `parse_to_json` timed as the parse, then `format` (skipped under
/// `parse_only`) as the format.
fn time_format(source: &str, parser_type: ParserType, parse_only: bool) -> Timed {
    match parser_type {
        ParserType::TypeScript => {
            time_calls(source, parse_only, tsv_ts::parse_to_json, tsv_ts::format)
        }
        ParserType::Svelte => time_calls(
            source,
            parse_only,
            tsv_svelte::parse_to_json,
            tsv_svelte::format,
        ),
        ParserType::Css => time_calls(source, parse_only, tsv_css::parse_to_json, tsv_css::format),
    }
}

/// Each call timed whole, its output dropped inside its timer and its error rendered outside it.
fn time_calls<E: Display>(
    source: &str,
    parse_only: bool,
    json: impl FnOnce(&str) -> Result<String, E>,
    format: impl FnOnce(&str) -> Result<String, E>,
) -> Timed {
    let t0 = Instant::now();
    let parsed = std::hint::black_box(json(source)).map(drop);
    let parse = t0.elapsed();
    parsed.map_err(|e| format!("parse error: {e}"))?;
    if parse_only {
        return Ok((parse, Duration::ZERO, 0));
    }
    let t1 = Instant::now();
    let formatted = format(source).map(|output| std::hint::black_box(output).len());
    let format_wall = t1.elapsed();
    let len = formatted.map_err(|e| format!("format error: {e}"))?;
    Ok((parse, format_wall, len))
}

fn run_strip(args: &Args) -> ExitCode {
    let mut ok_files = 0usize;
    let mut bytes = 0usize;
    let mut errors = 0usize;
    let mut refused = 0usize;
    let mut parse_us = 0.0;
    let mut strip_us = 0.0;
    let mut output_bytes = 0usize;
    let mut parse_times = Vec::with_capacity(args.iterations);
    let mut strip_times = Vec::with_capacity(args.iterations);
    for path in &corpus_files(args) {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("perf_ab_one_shot: read error {}: {e}", path.display());
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
            let (parse, parsed) = timed(|| {
                match goal {
                    Some(goal) => tsv_ts::parse_imports_with_goal(&source, goal),
                    None => tsv_ts::parse_imports(&source),
                }
                .is_ok()
            });
            if !parsed {
                failed = true;
                break;
            }
            parse_times.push(parse);
            if args.parse_only {
                continue;
            }
            let (strip, stripped) = timed(|| {
                match goal {
                    Some(goal) => tsv_ts::strip_with_goal(&source, goal),
                    None => tsv_ts::strip(&source),
                }
                .map(|stripped| stripped.code.len())
            });
            let output = match stripped {
                Ok(len) => Some(len),
                Err(tsv_ts::StripError::Parse(_)) => {
                    failed = true;
                    break;
                }
                Err(_) => None,
            };
            if iteration == 0 {
                file_output = Some(output);
            }
            strip_times.push(strip);
        }
        if failed {
            eprintln!("perf_ab_one_shot: {}: parse error", path.display());
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

/// `perf_ab_strip.rs`'s filter: TypeScript a module loader strips — `.ts` / `.mts` / `.cts`,
/// `.svelte.ts` included — and not a declaration file.
fn is_strippable(name: &str) -> bool {
    [".ts", ".mts", ".cts"]
        .iter()
        .any(|ext| name.ends_with(ext))
        && ![".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

/// The median of `durations` in microseconds (the mean of the middle two for an even count;
/// zero for none) — the warm drivers' per-file statistic.
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

/// The warm drivers' directory walk: recurse, pruning the discovery safety nets, hidden
/// directories and the build-output heuristic; keep the extensions `tsv format` discovers.
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
