use argh::FromArgs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::profile::{format_duration, format_size, median_us, resolve_files};
use crate::cli::CliError;
use tsv_svelte_compile::{CompileError, CompileOptions, compile};

/// Profile Svelte compile timing against the format wall.
///
/// For every `.svelte` file that compiles (refusals and parse failures are
/// counted, not timed), measures three medians over N iterations:
///
/// - **compile** — `tsv_svelte_compile::compile`, the whole cold shape: every
///   call pays its own internal arenas (AST bump, per-program `DocArena`s, the
///   validation-reparse bump). There is no warm entry point, so this IS the
///   production shape today.
/// - **parse** / **format** — `tsv_svelte::parse` + `format_in` on the same
///   file with run-shared reset arenas, the product shape `tsv_cli format`
///   pays (mirroring the `profile` command).
///
/// The headline is the **ratio** column: compile ÷ (parse + format) — the
/// compile-multiple over the format wall. The design frame expects ~2–3×
/// (all-linear pipeline); a drifting multiple is the cheap tripwire for
/// super-linear or rebuilt work. The two rows deliberately keep their own
/// production shapes (cold compile vs warm format), so compare ratios only
/// against ratios measured by this same command.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_profile")]
pub struct CompileProfileCommand {
    /// number of iterations (default: 10)
    #[argh(option, default = "10")]
    iterations: usize,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

impl CompileProfileCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if self.paths.is_empty() {
            eprintln!("Error: No files provided. Use file paths, directories, or glob patterns.");
            return Err(CliError::Failed);
        }
        let mut files = match resolve_files(&self.paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };
        files.retain(|p| p.extension().and_then(|e| e.to_str()) == Some("svelte"));
        if files.is_empty() {
            eprintln!("Error: No .svelte files found");
            return Err(CliError::Failed);
        }

        let mut results = Vec::new();
        let mut refused = 0usize;
        let mut parse_failed = 0usize;
        let mut corrupt = 0usize;

        // The parse/format reference rows reuse one AST `Bump` and one
        // `DocArena` across the run with `reset()` between iterations — the
        // `tsv_cli format` worker lifecycle, same as the `profile` command.
        // The compile row gets no such treatment on purpose: `compile()` has
        // no caller-owned-arena variant, so its cold per-call allocations are
        // part of what this command measures.
        let mut arena = bumpalo::Bump::new();
        let mut doc_arena = tsv_lang::doc::arena::DocArena::new();

        for path in &files {
            match profile_compile_file(path, self.iterations, &mut arena, &mut doc_arena) {
                Ok(Outcome::Timed(result)) => results.push(result),
                Ok(Outcome::Refused) => refused += 1,
                Ok(Outcome::ParseFailed) => parse_failed += 1,
                Ok(Outcome::CorruptOutput(err)) => {
                    corrupt += 1;
                    eprintln!("COMPILER BUG (CorruptOutput) {}: {err}", path.display());
                }
                Err(err) => {
                    eprintln!("Error profiling {}: {err}", path.display());
                    arena.reset();
                    doc_arena.reset();
                }
            }
        }

        if results.is_empty() {
            eprintln!(
                "No files compiled (refused: {refused}, parse failures: {parse_failed}, corrupt: {corrupt})."
            );
            return if corrupt > 0 {
                Err(CliError::Failed)
            } else {
                Ok(())
            };
        }

        // Slowest compiles first.
        results.sort_by(|a, b| {
            b.compile_us
                .partial_cmp(&a.compile_us)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let skips = Skips {
            refused,
            parse_failed,
            corrupt,
        };
        if self.json {
            print_json(&results, self.iterations, &skips);
        } else {
            print_table(&results, self.iterations, &skips);
        }

        if corrupt > 0 {
            return Err(CliError::Failed);
        }
        Ok(())
    }
}

/// Untimed-bucket counts for the run summary.
struct Skips {
    refused: usize,
    parse_failed: usize,
    corrupt: usize,
}

/// Per-file classification: timed result or an untimed bucket.
enum Outcome {
    Timed(FileResult),
    Refused,
    ParseFailed,
    CorruptOutput(tsv_lang::ParseError),
}

/// Timing results for one compiled file.
struct FileResult {
    path: PathBuf,
    size: usize,
    compile_us: f64,
    parse_us: f64,
    format_us: f64,
}

impl FileResult {
    /// compile ÷ (parse + format): the compile-multiple over the format wall.
    fn ratio(&self) -> f64 {
        let wall = self.parse_us + self.format_us;
        if wall > 0.0 {
            self.compile_us / wall
        } else {
            0.0
        }
    }
}

/// Classify one file, then (when it compiles) measure median compile /
/// parse / format timings over N iterations.
fn profile_compile_file(
    path: &Path,
    iterations: usize,
    arena: &mut bumpalo::Bump,
    doc_arena: &mut tsv_lang::doc::arena::DocArena,
) -> Result<Outcome, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let options = CompileOptions::default();

    // Classification pass, untimed: only compiling files are worth timing.
    match compile(&source, &options) {
        Ok(_) => {}
        Err(CompileError::Unsupported(_)) => return Ok(Outcome::Refused),
        Err(CompileError::Parse(_)) => return Ok(Outcome::ParseFailed),
        Err(CompileError::CorruptOutput(err)) => return Ok(Outcome::CorruptOutput(err)),
    }

    let mut compile_times = Vec::with_capacity(iterations);
    let mut parse_times = Vec::with_capacity(iterations);
    let mut format_times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let t0 = Instant::now();
        let _ = compile(&source, &options);
        compile_times.push(t0.elapsed());

        let t1 = Instant::now();
        let ast = tsv_svelte::parse(&source, arena).map_err(|e| format!("parse error: {e}"))?;
        parse_times.push(t1.elapsed());

        let t2 = Instant::now();
        let _ = tsv_svelte::format_in(&ast, &source, doc_arena);
        format_times.push(t2.elapsed());

        // Reference-row arena teardown outside the timed regions, mirroring
        // the `profile` command.
        arena.reset();
        doc_arena.reset();
    }

    compile_times.sort();
    parse_times.sort();
    format_times.sort();

    Ok(Outcome::Timed(FileResult {
        path: path.to_path_buf(),
        size: source.len(),
        compile_us: median_us(&compile_times),
        parse_us: median_us(&parse_times),
        format_us: median_us(&format_times),
    }))
}

/// Aggregate timing over the compiled set (sums; rates derived).
struct Aggregate {
    files: usize,
    size_bytes: usize,
    compile_us: f64,
    parse_us: f64,
    format_us: f64,
}

impl Aggregate {
    fn from_results(results: &[FileResult]) -> Self {
        let mut agg = Self {
            files: 0,
            size_bytes: 0,
            compile_us: 0.0,
            parse_us: 0.0,
            format_us: 0.0,
        };
        for r in results {
            agg.files += 1;
            agg.size_bytes += r.size;
            agg.compile_us += r.compile_us;
            agg.parse_us += r.parse_us;
            agg.format_us += r.format_us;
        }
        agg
    }

    fn ratio(&self) -> f64 {
        let wall = self.parse_us + self.format_us;
        if wall > 0.0 {
            self.compile_us / wall
        } else {
            0.0
        }
    }

    fn us_per_kb(&self, us: f64) -> f64 {
        us_per_kb(self.size_bytes, us)
    }
}

#[allow(clippy::cast_precision_loss)]
fn us_per_kb(size_bytes: usize, us: f64) -> f64 {
    if size_bytes == 0 {
        return 0.0;
    }
    us / (size_bytes as f64 / 1024.0)
}

fn print_table(results: &[FileResult], iterations: usize, skips: &Skips) {
    let total = Aggregate::from_results(results);

    let name_width = results
        .iter()
        .map(|r| display_path(&r.path).len())
        .max()
        .unwrap_or(4)
        .max(8);

    let row = |file: &str,
               size: &str,
               compile: &str,
               parse: &str,
               format: &str,
               ratio: &str,
               rate: &str| {
        let line = format!(
            "{file:>name_width$}  {size:>7}  {compile:>10}  {parse:>10}  {format:>10}  {ratio:>6}  {rate:>7}"
        );
        eprintln!("{}", line.trim_end());
    };

    row(
        "file", "size", "compile", "parse", "format", "ratio", "us/KB",
    );
    row(
        "----", "----", "-------", "-----", "------", "-----", "-----",
    );

    for r in results {
        row(
            &display_path(&r.path),
            &format_size(r.size),
            &format_duration(r.compile_us),
            &format_duration(r.parse_us),
            &format_duration(r.format_us),
            &format!("{:.2}x", r.ratio()),
            &format!("{:.1}", us_per_kb(r.size, r.compile_us)),
        );
    }

    row("", "----", "-------", "-----", "------", "-----", "-----");
    let files = total.files;
    row(
        &format!("({files} files)"),
        &format_size(total.size_bytes),
        &format_duration(total.compile_us),
        &format_duration(total.parse_us),
        &format_duration(total.format_us),
        &format!("{:.2}x", total.ratio()),
        &format!("{:.1}", total.us_per_kb(total.compile_us)),
    );

    eprintln!();
    eprintln!(
        "iterations: {iterations} (median shown); compiled: {}, refused: {}, parse failures: {}, corrupt: {}",
        total.files, skips.refused, skips.parse_failed, skips.corrupt
    );
    eprintln!(
        "compile = cold per-call shape; parse/format = warm reset-reuse arenas (tsv_cli shape); ratio = compile / (parse + format)"
    );
}

fn print_json(results: &[FileResult], iterations: usize, skips: &Skips) {
    let total = Aggregate::from_results(results);

    let files: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "path": r.path.to_string_lossy(),
                "size_bytes": r.size,
                "compile_us": r.compile_us,
                "parse_us": r.parse_us,
                "format_us": r.format_us,
                "ratio": r.ratio(),
                "compile_us_per_kb": us_per_kb(r.size, r.compile_us),
            })
        })
        .collect();

    let output = serde_json::json!({
        "iterations": iterations,
        "compiled": total.files,
        "refused": skips.refused,
        "parse_failed": skips.parse_failed,
        "corrupt": skips.corrupt,
        "files": files,
        "totals": {
            "files": total.files,
            "size_bytes": total.size_bytes,
            "compile_us": total.compile_us,
            "parse_us": total.parse_us,
            "format_us": total.format_us,
            "ratio": total.ratio(),
            "compile_us_per_kb": total.us_per_kb(total.compile_us),
            "parse_us_per_kb": total.us_per_kb(total.parse_us),
            "format_us_per_kb": total.us_per_kb(total.format_us),
        },
    });

    // SAFETY: serde_json Value types always serialize successfully
    #[allow(clippy::unwrap_used)]
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{json_str}");
}

/// Shorten path for display (show last 3 components)
fn display_path(path: &Path) -> String {
    let components: Vec<_> = path.components().collect();
    if components.len() <= 3 {
        return path.to_string_lossy().to_string();
    }
    let last_3: PathBuf = components[components.len() - 3..].iter().collect();
    format!(".../{}", last_3.display())
}
