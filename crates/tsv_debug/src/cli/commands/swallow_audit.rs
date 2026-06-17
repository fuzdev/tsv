use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::PathBuf;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_lang::doc::swallow::{self, SwallowReport};

use super::profile::resolve_files;

/// Audit for line comments that swallow the following token.
///
/// Enables the render-time swallow check (`tsv_lang::doc::swallow`) and formats
/// each file, reporting every spot where a `//` line comment is followed by
/// content on the same physical output line (silent content loss). Pure Rust —
/// no Deno. Defaults to `tests/fixtures` when no paths are given.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "swallow_audit")]
pub struct SwallowAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// A swallow plus the file it was found in.
struct Violation {
    path: PathBuf,
    report: SwallowReport,
}

impl SwallowAuditCommand {
    pub fn run(self) {
        let paths = if self.paths.is_empty() {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths
        };
        let files = match resolve_files(&paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };

        // Enable the check for the whole run; the builder records line-comment
        // ids and the renderer flags swallows. Single-threaded so the
        // thread-local report sink collects everything.
        swallow::set_swallow_check(true);

        let mut violations: Vec<Violation> = Vec::new();
        let mut formatted = 0usize;
        let mut parse_errors = 0usize;

        for path in &files {
            // Skip fixtures expected to fail parsing.
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("input_invalid"))
            {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(path) else {
                continue;
            };
            // Drain any stragglers, then format and collect.
            let _ = swallow::take_swallow_reports();
            if format_source(&source, ParserType::from_extension(&path.to_string_lossy())).is_err()
            {
                parse_errors += 1;
                continue;
            }
            formatted += 1;
            for report in swallow::take_swallow_reports() {
                violations.push(Violation {
                    path: path.clone(),
                    report,
                });
            }
        }

        swallow::set_swallow_check(false);

        if self.json {
            print_json(&violations, formatted, parse_errors);
        } else {
            print_report(&violations, formatted, parse_errors);
        }

        if !violations.is_empty() {
            std::process::exit(1);
        }
    }
}

fn print_report(violations: &[Violation], formatted: usize, parse_errors: usize) {
    if violations.is_empty() {
        println!(
            "✓ no line-comment swallows across {formatted} files ({parse_errors} parse-skipped)"
        );
        return;
    }

    println!(
        "✗ {} swallow(s) across {} file(s) ({formatted} formatted, {parse_errors} parse-skipped)\n",
        violations.len(),
        violations
            .iter()
            .map(|v| v.path.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
    );

    for v in violations {
        println!("  {}", v.path.display());
        println!("    comment:   {:?}", v.report.comment);
        println!("    swallows:  {:?}", v.report.following);
        println!("    line:      {:?}", v.report.line_context.trim_start());
        println!();
    }

    // Unique (comment, swallowed) shapes — the dedup'd worklist.
    let mut shapes: BTreeMap<(String, String), usize> = BTreeMap::new();
    for v in violations {
        *shapes
            .entry((v.report.comment.clone(), v.report.following.clone()))
            .or_default() += 1;
    }
    println!("Unique swallow shapes ({}):", shapes.len());
    for ((comment, following), count) in &shapes {
        println!("  {count:>4}×  {comment:?} ⊐ {following:?}");
    }
}

fn print_json(violations: &[Violation], formatted: usize, parse_errors: usize) {
    let items: Vec<serde_json::Value> = violations
        .iter()
        .map(|v| {
            serde_json::json!({
                "path": v.path.to_string_lossy(),
                "comment": v.report.comment,
                "following": v.report.following,
                "line_context": v.report.line_context,
            })
        })
        .collect();
    let output = serde_json::json!({
        "formatted": formatted,
        "parse_skipped": parse_errors,
        "swallows": violations.len(),
        "violations": items,
    });
    #[allow(clippy::unwrap_used)]
    let s = serde_json::to_string_pretty(&output).unwrap();
    println!("{s}");
}
