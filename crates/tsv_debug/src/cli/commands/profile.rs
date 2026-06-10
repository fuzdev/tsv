use argh::FromArgs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tsv_cli::cli::input::ParserType;

/// Profile parse + format timing on files or directories.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "profile")]
pub struct ProfileCommand {
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

impl ProfileCommand {
    pub fn run(self) {
        let (files, skipped) = resolve_profile_files(&self.paths, |_| false);

        let mut results = Vec::new();

        for path in &files {
            match profile_file(path, self.iterations) {
                Ok(result) => results.push(result),
                Err(err) => {
                    eprintln!("Error profiling {}: {err}", path.display());
                }
            }
        }

        if results.is_empty() {
            eprintln!("No files profiled successfully.");
            return;
        }

        // Sort by total time descending — slowest files first
        results.sort_by(|a, b| {
            b.total_us
                .partial_cmp(&a.total_us)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if self.json {
            print_json(&results, self.iterations, skipped);
        } else {
            print_table(&results, self.iterations, skipped);
        }
    }
}

/// Timing results for a single file
struct FileResult {
    path: PathBuf,
    size: usize,
    parser_type: ParserType,
    parse_us: f64,
    format_us: f64,
    total_us: f64,
}

/// Aggregate timing over a set of file results (whole run or one language).
///
/// Exposes per-KB and per-file rates alongside wall totals — wall-clock
/// totals on a moving corpus carry no drift signal on their own (corpus
/// growth/shrink and machine state both move them), so the rates are the
/// portable numbers to compare across runs.
struct Aggregate {
    files: usize,
    size_bytes: usize,
    parse_us: f64,
    format_us: f64,
}

impl Aggregate {
    fn from_results<'a>(results: impl Iterator<Item = &'a FileResult>) -> Self {
        let mut agg = Self {
            files: 0,
            size_bytes: 0,
            parse_us: 0.0,
            format_us: 0.0,
        };
        for r in results {
            agg.files += 1;
            agg.size_bytes += r.size;
            agg.parse_us += r.parse_us;
            agg.format_us += r.format_us;
        }
        agg
    }

    fn total_us(&self) -> f64 {
        self.parse_us + self.format_us
    }

    fn parse_pct(&self) -> f64 {
        let total = self.total_us();
        if total > 0.0 {
            self.parse_us / total * 100.0
        } else {
            0.0
        }
    }

    fn us_per_kb(&self, us: f64) -> f64 {
        us_per_kb(self.size_bytes, us)
    }

    #[allow(clippy::cast_precision_loss)]
    fn us_per_file(&self, us: f64) -> f64 {
        if self.files == 0 {
            return 0.0;
        }
        us / self.files as f64
    }
}

#[allow(clippy::cast_precision_loss)]
fn us_per_kb(size_bytes: usize, us: f64) -> f64 {
    if size_bytes == 0 {
        return 0.0;
    }
    us / (size_bytes as f64 / 1024.0)
}

/// Per-language aggregates in fixed order, skipping absent languages.
fn lang_groups(results: &[FileResult]) -> Vec<(&'static str, Aggregate)> {
    [ParserType::TypeScript, ParserType::Svelte, ParserType::Css]
        .into_iter()
        .filter_map(|pt| {
            let agg = Aggregate::from_results(results.iter().filter(|r| r.parser_type == pt));
            (agg.files > 0).then(|| (lang_label(pt), agg))
        })
        .collect()
}

fn files_label(n: usize) -> String {
    if n == 1 {
        "(1 file)".to_string()
    } else {
        format!("({n} files)")
    }
}

/// Profile a single file: parse and format N times, return median timing
fn profile_file(path: &Path, iterations: usize) -> Result<FileResult, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let parser_type = ParserType::from_extension(&path.to_string_lossy());

    let mut parse_times = Vec::with_capacity(iterations);
    let mut format_times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let (parse_dur, format_dur) = profile_once(&source, parser_type)?;
        parse_times.push(parse_dur);
        format_times.push(format_dur);
    }

    parse_times.sort();
    format_times.sort();

    let parse_us = median_us(&parse_times);
    let format_us = median_us(&format_times);

    Ok(FileResult {
        path: path.to_path_buf(),
        size: source.len(),
        parser_type,
        parse_us,
        format_us,
        total_us: parse_us + format_us,
    })
}

/// Run one parse + format iteration, return (parse_duration, format_duration)
fn profile_once(source: &str, parser_type: ParserType) -> Result<(Duration, Duration), String> {
    match parser_type {
        ParserType::TypeScript => {
            let t0 = Instant::now();
            let ast = tsv_ts::parse(source).map_err(|e| format!("parse error: {e}"))?;
            let parse_dur = t0.elapsed();

            let t1 = Instant::now();
            let _ = tsv_ts::format(&ast, source);
            let format_dur = t1.elapsed();

            Ok((parse_dur, format_dur))
        }
        ParserType::Svelte => {
            let t0 = Instant::now();
            let ast = tsv_svelte::parse(source).map_err(|e| format!("parse error: {e}"))?;
            let parse_dur = t0.elapsed();

            let t1 = Instant::now();
            let _ = tsv_svelte::format(&ast, source);
            let format_dur = t1.elapsed();

            Ok((parse_dur, format_dur))
        }
        ParserType::Css => {
            let t0 = Instant::now();
            let ast = tsv_css::parse(source).map_err(|e| format!("parse error: {e}"))?;
            let parse_dur = t0.elapsed();

            let t1 = Instant::now();
            let _ = tsv_css::format(&ast, source);
            let format_dur = t1.elapsed();

            Ok((parse_dur, format_dur))
        }
    }
}

pub(crate) fn median_us(durations: &[Duration]) -> f64 {
    let len = durations.len();
    if len == 0 {
        return 0.0;
    }
    if len % 2 == 1 {
        duration_to_us(durations[len / 2])
    } else {
        let a = duration_to_us(durations[len / 2 - 1]);
        let b = duration_to_us(durations[len / 2]);
        f64::midpoint(a, b)
    }
}

fn duration_to_us(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000_000.0
}

pub(crate) fn format_duration(us: f64) -> String {
    if us >= 1000.0 {
        format!("{:.2}ms", us / 1000.0)
    } else {
        format!("{us:.0}us")
    }
}

#[allow(clippy::cast_precision_loss)]
pub(crate) fn format_size(bytes: usize) -> String {
    if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes}B")
    }
}

pub(crate) fn lang_label(parser_type: ParserType) -> &'static str {
    match parser_type {
        ParserType::TypeScript => "ts",
        ParserType::Svelte => "svelte",
        ParserType::Css => "css",
    }
}

fn print_table(results: &[FileResult], iterations: usize, skipped: usize) {
    let total = Aggregate::from_results(results.iter());
    let langs = lang_groups(results);
    let total_label = files_label(total.files);

    // Calculate column widths
    let name_width = results
        .iter()
        .map(|r| display_path(&r.path).len())
        .chain(std::iter::once(total_label.len()))
        .max()
        .unwrap_or(4)
        .max(8); // "per file"

    let row = |file: &str,
               lang: &str,
               size: &str,
               parse: &str,
               format: &str,
               total: &str,
               split: &str,
               rate: &str| {
        let line = format!(
            "{file:>name_width$}  {lang:>6}  {size:>7}  {parse:>10}  {format:>10}  {total:>10}  {split:>5}  {rate:>7}"
        );
        eprintln!("{}", line.trim_end());
    };

    row(
        "file", "lang", "size", "parse", "format", "total", "split", "us/KB",
    );
    row(
        "----", "----", "----", "-----", "------", "-----", "-----", "-----",
    );

    for r in results {
        let parse_pct = if r.total_us > 0.0 {
            r.parse_us / r.total_us * 100.0
        } else {
            0.0
        };
        row(
            &display_path(&r.path),
            lang_label(r.parser_type),
            &format_size(r.size),
            &format_duration(r.parse_us),
            &format_duration(r.format_us),
            &format_duration(r.total_us),
            &format!("{parse_pct:.0}%"),
            &format!("{:.1}", us_per_kb(r.size, r.total_us)),
        );
    }

    // Totals — per-language rows first (when mixed), then the grand total
    row("", "", "----", "-----", "------", "-----", "", "");
    if langs.len() > 1 {
        for (label, agg) in &langs {
            row(
                &files_label(agg.files),
                label,
                &format_size(agg.size_bytes),
                &format_duration(agg.parse_us),
                &format_duration(agg.format_us),
                &format_duration(agg.total_us()),
                &format!("{:.0}%", agg.parse_pct()),
                &format!("{:.1}", agg.us_per_kb(agg.total_us())),
            );
        }
    }
    row(
        &total_label,
        "",
        &format_size(total.size_bytes),
        &format_duration(total.parse_us),
        &format_duration(total.format_us),
        &format_duration(total.total_us()),
        &format!("{:.0}%", total.parse_pct()),
        &format!("{:.1}", total.us_per_kb(total.total_us())),
    );

    // Normalized rates — the portable metrics across corpus changes
    row(
        "per file",
        "",
        &format_size(total.size_bytes / total.files.max(1)),
        &format_duration(total.us_per_file(total.parse_us)),
        &format_duration(total.us_per_file(total.format_us)),
        &format_duration(total.us_per_file(total.total_us())),
        "",
        "",
    );
    row(
        "per KB",
        "",
        "",
        &format!("{:.1}us", total.us_per_kb(total.parse_us)),
        &format!("{:.1}us", total.us_per_kb(total.format_us)),
        &format!("{:.1}us", total.us_per_kb(total.total_us())),
        "",
        "",
    );

    eprintln!();
    let skip_msg = if skipped > 0 {
        format!(", {skipped} invalid skipped")
    } else {
        String::new()
    };
    eprintln!("iterations: {iterations} (median shown{skip_msg})");
}

fn print_json(results: &[FileResult], iterations: usize, skipped: usize) {
    let total = Aggregate::from_results(results.iter());

    let files: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "path": r.path.to_string_lossy(),
                "lang": lang_label(r.parser_type),
                "size_bytes": r.size,
                "parse_us": r.parse_us,
                "format_us": r.format_us,
                "total_us": r.total_us,
                "total_us_per_kb": us_per_kb(r.size, r.total_us),
            })
        })
        .collect();

    let langs: serde_json::Map<String, serde_json::Value> = lang_groups(results)
        .iter()
        .map(|(label, agg)| ((*label).to_string(), aggregate_json(agg)))
        .collect();

    let output = serde_json::json!({
        "iterations": iterations,
        "skipped": skipped,
        "files": files,
        "langs": langs,
        "totals": aggregate_json(&total),
    });

    // SAFETY: serde_json Value types always serialize successfully
    #[allow(clippy::unwrap_used)]
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{json_str}");
}

fn aggregate_json(agg: &Aggregate) -> serde_json::Value {
    serde_json::json!({
        "files": agg.files,
        "size_bytes": agg.size_bytes,
        "parse_us": agg.parse_us,
        "format_us": agg.format_us,
        "total_us": agg.total_us(),
        "parse_pct": agg.parse_pct(),
        "parse_us_per_kb": agg.us_per_kb(agg.parse_us),
        "format_us_per_kb": agg.us_per_kb(agg.format_us),
        "total_us_per_kb": agg.us_per_kb(agg.total_us()),
        "parse_us_per_file": agg.us_per_file(agg.parse_us),
        "format_us_per_file": agg.us_per_file(agg.format_us),
        "total_us_per_file": agg.us_per_file(agg.total_us()),
    })
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

/// Resolve CLI path args to profileable files, exiting with a user-facing
/// message when nothing matches. `excluded` files are dropped after
/// resolution; `input_invalid_*` fixtures (expected to fail parsing) are
/// filtered out and returned as a skip count. Shared preamble of the
/// `profile` and `json_profile` commands.
pub(crate) fn resolve_profile_files(
    paths: &[String],
    excluded: impl Fn(&Path) -> bool,
) -> (Vec<PathBuf>, usize) {
    if paths.is_empty() {
        eprintln!("Error: No files provided. Use file paths, directories, or glob patterns.");
        std::process::exit(1);
    }
    let mut files = match resolve_files(paths) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    files.retain(|p| !excluded(p));
    if files.is_empty() {
        eprintln!("Error: No supported files found (.ts, .svelte, .css)");
        std::process::exit(1);
    }
    let total = files.len();
    files.retain(|p| {
        !p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("input_invalid"))
    });
    let skipped = total - files.len();
    (files, skipped)
}

/// Resolve paths to files, expanding directories
pub(crate) fn resolve_files(paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for path_str in paths {
        let path = PathBuf::from(path_str);
        if path.is_dir() {
            collect_files_recursive(&path, &mut files);
        } else if path.is_file() {
            if is_supported_file(&path) {
                files.push(path);
            }
        } else {
            // Try as glob pattern
            let matched = glob_files(path_str);
            if matched.is_empty() {
                return Err(format!("No files found matching: {path_str}"));
            }
            files.extend(matched);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories and node_modules
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && (name.starts_with('.') || name == "node_modules" || name == "target")
            {
                continue;
            }
            collect_files_recursive(&path, files);
        } else if is_supported_file(&path) {
            files.push(path);
        }
    }
}

fn is_supported_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext,
        "ts" | "svelte" | "css" | "js" | "mts" | "cts" | "mjs" | "cjs"
    )
}

/// Simple glob expansion (handles patterns like tests/fixtures/**/input.ts)
fn glob_files(pattern: &str) -> Vec<PathBuf> {
    // Use a simple approach: split at the first wildcard, list the directory, filter
    // For more complex globs, the user can pipe through find
    if !pattern.contains('*') {
        return Vec::new();
    }

    // Find the base directory (everything before the first *)
    let parts: Vec<&str> = pattern.splitn(2, '*').collect();
    let base = if parts[0].is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(parts[0].trim_end_matches('/'))
    };

    if !base.is_dir() {
        return Vec::new();
    }

    // Collect all files under base and filter by the full pattern suffix
    let suffix = if parts.len() > 1 {
        parts[1].trim_start_matches('*').trim_start_matches('/')
    } else {
        ""
    };

    let mut files = Vec::new();
    collect_files_recursive(&base, &mut files);

    // Filter by suffix if present
    if !suffix.is_empty() {
        files.retain(|f| f.to_string_lossy().ends_with(suffix));
    }

    files
}
