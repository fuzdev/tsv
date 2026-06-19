use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Codebase metrics — line counts by crate and phase.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "metrics")]
pub struct MetricsCommand {
    /// emit JSON for scripting
    #[argh(switch)]
    json: bool,
}

/// Line count results for a single crate
struct CrateMetrics {
    name: String,
    total: usize,
    phases: BTreeMap<String, usize>,
}

/// Crate group for summary reporting
struct CrateGroup {
    name: &'static str,
    crates: &'static [&'static str],
}

const GROUPS: &[CrateGroup] = &[
    CrateGroup {
        name: "foundation",
        crates: &["tsv_lang", "tsv_html", "tsv_ignore"],
    },
    CrateGroup {
        name: "languages",
        crates: &["tsv_ts", "tsv_css", "tsv_svelte"],
    },
    CrateGroup {
        name: "tooling",
        crates: &["tsv_cli", "tsv_debug", "tsv_ffi", "tsv_wasm"],
    },
];

impl MetricsCommand {
    pub fn run(self) {
        let Some(crates_dir) = find_crates_dir() else {
            eprintln!("Error: Could not find crates/ directory. Run from the workspace root.");
            std::process::exit(1);
        };

        let mut results = Vec::new();

        let Ok(entries) = std::fs::read_dir(&crates_dir) else {
            eprintln!("Error: Could not read {}", crates_dir.display());
            std::process::exit(1);
        };

        let mut crate_dirs: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
        crate_dirs.sort();

        for crate_dir in &crate_dirs {
            let src_dir = crate_dir.join("src");
            if !src_dir.is_dir() {
                continue;
            }

            let crate_name = crate_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let metrics = measure_crate(&crate_name, &src_dir);
            results.push(metrics);
        }

        if results.is_empty() {
            eprintln!("No crates found.");
            return;
        }

        if self.json {
            print_json(&results);
        } else {
            print_table(&results);
        }
    }
}

/// Measure a single crate's line counts by phase
fn measure_crate(crate_name: &str, src_dir: &Path) -> CrateMetrics {
    let mut phases: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;

    let mut files = Vec::new();
    collect_rs_files(src_dir, &mut files);

    for file_path in &files {
        let lines = count_lines(file_path);
        total += lines;

        let phase = classify_phase(crate_name, file_path, src_dir);
        *phases.entry(phase).or_insert(0) += lines;
    }

    CrateMetrics {
        name: crate_name.to_string(),
        total,
        phases,
    }
}

/// Classify a file into a phase based on its directory within src/
fn classify_phase(crate_name: &str, file_path: &Path, src_dir: &Path) -> String {
    let relative = file_path.strip_prefix(src_dir).unwrap_or(file_path);

    // Get the first directory component after src/
    let first_dir = relative
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or("");

    // Known phase directories
    match first_dir {
        "lexer" => return "lexer".to_string(),
        "parser" => return "parser".to_string(),
        "ast" => return "ast".to_string(),
        "printer" => return "printer".to_string(),
        _ => {}
    }

    // Special cases per crate
    match crate_name {
        "tsv_lang" => {
            if first_dir == "doc" {
                return "doc".to_string();
            }
        }
        "tsv_svelte" => {
            // lexer.rs is a top-level file, not a directory
            if let Some(name) = file_path.file_name().and_then(|n| n.to_str())
                && name == "lexer.rs"
            {
                return "lexer".to_string();
            }
        }
        _ => {}
    }

    // Classify top-level escapes.rs
    if let Some(name) = file_path.file_name().and_then(|n| n.to_str())
        && name == "escapes.rs"
        && relative.components().count() == 1
    {
        return "escapes".to_string();
    }

    // Deno sidecar directory
    if first_dir == "deno" {
        return "deno".to_string();
    }

    // Fixture/test infrastructure directories
    if first_dir == "fixtures" || first_dir == "test262" {
        return first_dir.to_string();
    }

    // CLI directory
    if first_dir == "cli" {
        return "cli".to_string();
    }

    "other".to_string()
}

/// Count lines in a file
fn count_lines(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

/// Recursively collect all .rs files
fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

/// Find the crates/ directory by walking up from the current directory
fn find_crates_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let crates = dir.join("crates");
        let cargo = dir.join("Cargo.toml");
        if crates.is_dir() && cargo.is_file() {
            return Some(crates);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// All phase column names in display order
const PHASE_ORDER: &[&str] = &[
    "doc", "lexer", "parser", "ast", "printer", "escapes", "cli", "deno", "fixtures", "test262",
    "other",
];

fn print_table(results: &[CrateMetrics]) {
    // Collect all phases that actually appear
    let mut active_phases: Vec<&str> = Vec::new();
    for phase in PHASE_ORDER {
        if results.iter().any(|r| r.phases.contains_key(*phase)) {
            active_phases.push(phase);
        }
    }

    // Calculate column widths
    let name_width = results
        .iter()
        .map(|r| r.name.len())
        .max()
        .unwrap_or(5)
        .max(5);

    let col_width = 8;

    // Header
    eprint!("{:>name_width$}  {:>col_width$}", "crate", "total");
    for phase in &active_phases {
        eprint!("  {phase:>col_width$}");
    }
    eprintln!();

    eprint!("{:>name_width$}  {:>col_width$}", "-----", "-----");
    for _ in &active_phases {
        eprint!("  {:>col_width$}", "-----");
    }
    eprintln!();

    // Rows
    for r in results {
        eprint!(
            "{:>name_width$}  {:>col_width$}",
            r.name,
            format_num(r.total)
        );
        for phase in &active_phases {
            let count = r.phases.get(*phase).copied().unwrap_or(0);
            if count > 0 {
                eprint!("  {:>col_width$}", format_num(count));
            } else {
                eprint!("  {:>col_width$}", "");
            }
        }
        eprintln!();
    }

    // Group summaries
    eprintln!();
    let grand_total: usize = results.iter().map(|r| r.total).sum();

    for group in GROUPS {
        let group_total: usize = results
            .iter()
            .filter(|r| group.crates.contains(&r.name.as_str()))
            .map(|r| r.total)
            .sum();

        if group_total > 0 {
            #[allow(clippy::cast_precision_loss)]
            let pct = if grand_total > 0 {
                (group_total as f64 / grand_total as f64) * 100.0
            } else {
                0.0
            };
            eprintln!(
                "{:>name_width$}: {} ({pct:.0}%)",
                group.name,
                format_num(group_total),
            );
        }
    }

    eprintln!("{:>name_width$}: {}", "total", format_num(grand_total));

    // Derived metrics
    let language_total: usize = results
        .iter()
        .filter(|r| ["tsv_ts", "tsv_css", "tsv_svelte"].contains(&r.name.as_str()))
        .map(|r| r.total)
        .sum();
    let printer_total: usize = results
        .iter()
        .filter(|r| ["tsv_ts", "tsv_css", "tsv_svelte"].contains(&r.name.as_str()))
        .filter_map(|r| r.phases.get("printer"))
        .sum();

    if language_total > 0 {
        #[allow(clippy::cast_precision_loss)]
        let printer_pct = (printer_total as f64 / language_total as f64) * 100.0;
        eprintln!();
        eprintln!("printer % of language code: {printer_pct:.0}%");
    }
}

fn format_num(n: usize) -> String {
    if n >= 100 {
        // Round to nearest 0.1K
        let tenths = (n + 50) / 100;
        let k = tenths / 10;
        let r = tenths % 10;
        if r > 0 {
            format!("{k}.{r}K")
        } else {
            format!("{k}K")
        }
    } else {
        n.to_string()
    }
}

fn print_json(results: &[CrateMetrics]) {
    let grand_total: usize = results.iter().map(|r| r.total).sum();

    let crates: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            let phases: serde_json::Map<String, serde_json::Value> = r
                .phases
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::from(*v)))
                .collect();
            serde_json::json!({
                "name": r.name,
                "total": r.total,
                "phases": phases,
            })
        })
        .collect();

    let mut groups = serde_json::Map::new();
    for group in GROUPS {
        let group_crates: Vec<&str> = results
            .iter()
            .filter(|r| group.crates.contains(&r.name.as_str()))
            .map(|r| r.name.as_str())
            .collect();
        let group_total: usize = results
            .iter()
            .filter(|r| group.crates.contains(&r.name.as_str()))
            .map(|r| r.total)
            .sum();
        groups.insert(
            group.name.to_string(),
            serde_json::json!({
                "crates": group_crates,
                "total": group_total,
            }),
        );
    }

    let output = serde_json::json!({
        "crates": crates,
        "groups": groups,
        "total": grand_total,
    });

    // SAFETY: serde_json Value types always serialize successfully
    #[allow(clippy::unwrap_used)]
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{json_str}");
}
