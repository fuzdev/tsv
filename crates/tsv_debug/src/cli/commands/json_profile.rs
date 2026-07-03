//! Profile the parse→JSON emission path (the FFI parse path).
//!
//! `tsv_ffi`'s `tsv_parse_<lang>` runs `parse` + `convert_ast_json_bytes`.
//! This command times those two phases per file across a corpus. The writer
//! (`convert_ast_json_bytes`) is the sole emission path — it walks the internal
//! AST once and emits the final char-space wire JSON directly, so there are no
//! sub-steps to decompose: just `parse` and `write`.
//!
//! Run with `--release`; debug-build numbers aren't meaningful.

use argh::FromArgs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tsv_cli::cli::input::ParserType;
use tsv_lang::{ByteToCharMap, estimated_ast_arena_capacity};

use super::profile::{format_duration, format_size, lang_label, median_us, resolve_profile_files};
use crate::cli::CliError;

/// Bench-corpus exclusions, mirrored from `benches/js/lib/corpus.ts`:
/// declaration files and build output add noise without exercising new
/// code paths.
fn is_bench_corpus_excluded(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.ends_with(".d.ts") || s.contains("/build/") || s.contains("/dist/")
}

/// Profile the parse→JSON emission path (`parse` + `convert_ast_json_bytes`).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "json_profile")]
pub struct JsonProfileCommand {
    /// number of iterations (default: 5)
    #[argh(option, default = "5")]
    iterations: usize,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

impl JsonProfileCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let (files, skipped) = resolve_profile_files(&self.paths, is_bench_corpus_excluded)?;

        let mut results = Vec::new();
        let mut parse_errors = 0usize;

        for path in &files {
            match profile_file(path, self.iterations) {
                Ok(result) => results.push(result),
                Err(_) => parse_errors += 1,
            }
        }

        if results.is_empty() {
            eprintln!("No files profiled successfully ({parse_errors} parse errors).");
            return Ok(());
        }

        let aggregates = aggregate(&results);

        if self.json {
            print_json(
                &aggregates,
                &results,
                self.iterations,
                parse_errors,
                skipped,
            );
        } else {
            print_report(&aggregates, self.iterations, parse_errors, skipped);
        }

        Ok(())
    }
}

/// Per-file medians for each phase (µs).
struct FileResult {
    path: PathBuf,
    size: usize,
    wire_bytes: usize,
    parser_type: ParserType,
    multibyte: bool,
    parse_us: f64,
    write_us: f64,
}

/// Raw per-iteration durations for each phase.
#[derive(Default)]
struct StepDurations {
    parse: Vec<Duration>,
    write: Vec<Duration>,
}

/// Per-file facts recorded on the first iteration only.
struct IterMeta {
    multibyte: bool,
    wire_bytes: usize,
}

fn profile_file(path: &Path, iterations: usize) -> Result<FileResult, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let parser_type = ParserType::from_extension(&path.to_string_lossy());

    let mut steps = StepDurations::default();
    let mut meta: Option<IterMeta> = None;

    for _ in 0..iterations {
        profile_once(&source, parser_type, &mut steps, &mut meta)?;
    }

    let meta = meta.ok_or("no iterations ran")?;
    let median = |mut v: Vec<Duration>| {
        v.sort();
        median_us(&v)
    };
    Ok(FileResult {
        path: path.to_path_buf(),
        size: source.len(),
        wire_bytes: meta.wire_bytes,
        parser_type,
        multibyte: meta.multibyte,
        parse_us: median(steps.parse),
        write_us: median(steps.write),
    })
}

/// One iteration of the FFI parse path: `parse` then `convert_ast_json_bytes`.
fn profile_once(
    source: &str,
    parser_type: ParserType,
    steps: &mut StepDurations,
    meta: &mut Option<IterMeta>,
) -> Result<(), String> {
    // Arena allocated outside the timed region so its setup isn't counted.
    let arena = bumpalo::Bump::with_capacity(estimated_ast_arena_capacity(source.len()));

    let wire = match parser_type {
        ParserType::TypeScript => {
            let t = Instant::now();
            let ast = tsv_ts::parse(source, &arena).map_err(|e| format!("parse error: {e}"))?;
            steps.parse.push(t.elapsed());
            let t = Instant::now();
            let wire = tsv_ts::convert_ast_json_bytes(&ast, source);
            steps.write.push(t.elapsed());
            wire
        }
        ParserType::Svelte => {
            let t = Instant::now();
            let ast = tsv_svelte::parse(source, &arena).map_err(|e| format!("parse error: {e}"))?;
            steps.parse.push(t.elapsed());
            let t = Instant::now();
            let wire = tsv_svelte::convert_ast_json_bytes(&ast, source);
            steps.write.push(t.elapsed());
            wire
        }
        ParserType::Css => {
            let t = Instant::now();
            let ast = tsv_css::parse(source, &arena).map_err(|e| format!("parse error: {e}"))?;
            steps.parse.push(t.elapsed());
            let t = Instant::now();
            let wire = tsv_css::convert_ast_json_bytes(&ast, source);
            steps.write.push(t.elapsed());
            wire
        }
    };

    if meta.is_none() {
        *meta = Some(IterMeta {
            multibyte: ByteToCharMap::new(source).has_multibyte(),
            wire_bytes: wire.len(),
        });
    }
    Ok(())
}

/// Sums of per-file medians for one language.
#[derive(Default)]
struct LangAggregate {
    files: usize,
    size: usize,
    wire_bytes: usize,
    multibyte_files: usize,
    parse_us: f64,
    write_us: f64,
}

fn aggregate(results: &[FileResult]) -> Vec<(ParserType, LangAggregate)> {
    let mut out: Vec<(ParserType, LangAggregate)> = Vec::new();
    for r in results {
        let agg = match out.iter_mut().find(|(t, _)| *t == r.parser_type) {
            Some((_, agg)) => agg,
            None => {
                out.push((r.parser_type, LangAggregate::default()));
                // SAFETY: just pushed
                #[allow(clippy::unwrap_used)]
                let last = out.last_mut().unwrap();
                &mut last.1
            }
        };
        agg.files += 1;
        agg.size += r.size;
        agg.wire_bytes += r.wire_bytes;
        agg.multibyte_files += usize::from(r.multibyte);
        agg.parse_us += r.parse_us;
        agg.write_us += r.write_us;
    }
    out
}

fn print_report(
    aggregates: &[(ParserType, LangAggregate)],
    iterations: usize,
    parse_errors: usize,
    skipped: usize,
) {
    for (parser_type, a) in aggregates {
        eprintln!(
            "{} — {} files, {} source, {} wire JSON, {} multibyte",
            lang_label(*parser_type),
            a.files,
            format_size(a.size),
            format_size(a.wire_bytes),
            a.multibyte_files,
        );
        eprintln!("  parse  {:>10}", format_duration(a.parse_us));
        eprintln!(
            "  write  {:>10}  (convert_ast_json_bytes — the sole emission path)",
            format_duration(a.write_us)
        );
        eprintln!();
    }

    let mut notes = vec![format!(
        "iterations: {iterations} (sums of per-file medians)"
    )];
    if parse_errors > 0 {
        notes.push(format!("{parse_errors} files skipped (parse errors)"));
    }
    if skipped > 0 {
        notes.push(format!("{skipped} input_invalid skipped"));
    }
    eprintln!("{}", notes.join(", "));
}

fn print_json(
    aggregates: &[(ParserType, LangAggregate)],
    results: &[FileResult],
    iterations: usize,
    parse_errors: usize,
    skipped: usize,
) {
    let languages: serde_json::Map<String, serde_json::Value> = aggregates
        .iter()
        .map(|(parser_type, a)| {
            let lang = serde_json::json!({
                "files": a.files,
                "size_bytes": a.size,
                "wire_bytes": a.wire_bytes,
                "multibyte_files": a.multibyte_files,
                "parse_us": a.parse_us,
                "write_us": a.write_us,
            });
            (lang_label(*parser_type).to_string(), lang)
        })
        .collect();

    let files: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "path": r.path.to_string_lossy(),
                "lang": lang_label(r.parser_type),
                "size_bytes": r.size,
                "wire_bytes": r.wire_bytes,
                "multibyte": r.multibyte,
                "parse_us": r.parse_us,
                "write_us": r.write_us,
            })
        })
        .collect();

    let output = serde_json::json!({
        "iterations": iterations,
        "parse_errors": parse_errors,
        "skipped": skipped,
        "languages": languages,
        "files": files,
    });

    // SAFETY: serde_json Value types always serialize successfully
    #[allow(clippy::unwrap_used)]
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{json_str}");
}
