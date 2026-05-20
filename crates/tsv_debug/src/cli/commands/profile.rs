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
        if self.paths.is_empty() {
            eprintln!("Error: No files provided. Use file paths, directories, or glob patterns.");
            std::process::exit(1);
        }

        let files = match resolve_files(&self.paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };
        if files.is_empty() {
            eprintln!("Error: No supported files found (.ts, .svelte, .css)");
            std::process::exit(1);
        }

        let mut results = Vec::new();
        let mut skipped = 0usize;

        for path in &files {
            // Skip input_invalid_* files — they're expected to fail parsing
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with("input_invalid")
            {
                skipped += 1;
                continue;
            }
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

fn median_us(durations: &[Duration]) -> f64 {
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

fn format_duration(us: f64) -> String {
    if us >= 1000.0 {
        format!("{:.2}ms", us / 1000.0)
    } else {
        format!("{us:.0}us")
    }
}

#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: usize) -> String {
    if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes}B")
    }
}

fn lang_label(parser_type: ParserType) -> &'static str {
    match parser_type {
        ParserType::TypeScript => "ts",
        ParserType::Svelte => "svelte",
        ParserType::Css => "css",
    }
}

fn print_table(results: &[FileResult], iterations: usize, skipped: usize) {
    // Calculate column widths
    let name_width = results
        .iter()
        .map(|r| display_path(&r.path).len())
        .max()
        .unwrap_or(4)
        .max(4);

    // Header
    eprintln!(
        "{:>name_width$}  {:>5}  {:>4}  {:>10}  {:>10}  {:>10}  {:>5}",
        "file", "lang", "size", "parse", "format", "total", "split"
    );
    eprintln!(
        "{:>name_width$}  {:>5}  {:>4}  {:>10}  {:>10}  {:>10}  {:>5}",
        "----", "----", "----", "-----", "------", "-----", "-----"
    );

    // Rows
    for r in results {
        let parse_pct = if r.total_us > 0.0 {
            r.parse_us / r.total_us * 100.0
        } else {
            0.0
        };
        eprintln!(
            "{:>name_width$}  {:>5}  {:>4}  {:>10}  {:>10}  {:>10}  {:>4.0}%",
            display_path(&r.path),
            lang_label(r.parser_type),
            format_size(r.size),
            format_duration(r.parse_us),
            format_duration(r.format_us),
            format_duration(r.total_us),
            parse_pct
        );
    }

    // Totals
    let total_size: usize = results.iter().map(|r| r.size).sum();
    let total_parse: f64 = results.iter().map(|r| r.parse_us).sum();
    let total_format: f64 = results.iter().map(|r| r.format_us).sum();
    let total: f64 = total_parse + total_format;
    let parse_pct = if total > 0.0 {
        total_parse / total * 100.0
    } else {
        0.0
    };

    eprintln!(
        "{:>name_width$}  {:>5}  {:>4}  {:>10}  {:>10}  {:>10}  {:>4.0}%",
        "", "", "----", "-----", "------", "-----", ""
    );
    eprintln!(
        "{:>name_width$}  {:>5}  {:>4}  {:>10}  {:>10}  {:>10}  {:>4.0}%",
        format!("({} files)", results.len()),
        "",
        format_size(total_size),
        format_duration(total_parse),
        format_duration(total_format),
        format_duration(total),
        parse_pct
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
    let total_parse: f64 = results.iter().map(|r| r.parse_us).sum();
    let total_format: f64 = results.iter().map(|r| r.format_us).sum();
    let total: f64 = total_parse + total_format;

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
            })
        })
        .collect();

    let output = serde_json::json!({
        "iterations": iterations,
        "skipped": skipped,
        "files": files,
        "totals": {
            "files": results.len(),
            "size_bytes": results.iter().map(|r| r.size).sum::<usize>(),
            "parse_us": total_parse,
            "format_us": total_format,
            "total_us": total,
            "parse_pct": if total > 0.0 { total_parse / total * 100.0 } else { 0.0 },
        }
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

/// Resolve paths to files, expanding directories
fn resolve_files(paths: &[String]) -> Result<Vec<PathBuf>, String> {
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
