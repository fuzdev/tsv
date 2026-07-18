use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::cli::CliError;
use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_lang::comment_ledger::{self, CommentFinding, CommentFindingKind};

use super::profile::resolve_files;

/// Audit that every parsed comment is printed exactly once.
///
/// Enables the print-once comment ledger (`tsv_lang::comment_ledger`) and formats each
/// file, reporting every comment the format DROPPED (parsed, never emitted — silent
/// content loss) or DOUBLE-PRINTED. Pure Rust — no Deno. Defaults to `tests/fixtures`
/// when no paths are given.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "comment_audit")]
pub struct CommentAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths, directories, or glob patterns (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// A finding plus the file it was found in.
struct Violation {
    path: PathBuf,
    finding: CommentFinding,
}

/// REGRESSION PIN (minimum, at the exact measured value): comments registered across a
/// default (`tests/fixtures`) run — with an empty or all-parse-failing corpus the audit
/// would pass vacuously ("0 findings across 0 comments"). A minimum, not a two-sided pin,
/// because the fixtures tree is COMMITTED and grows with ordinary fixture PRs (`deno task
/// check` must not fail per added fixture); shrinkage/collapse fails. Re-pin to current
/// when it trips. Same ritual as `swallow_audit`'s `FORMATTED_MIN` and
/// `benches/js/lib/gate_counts.ts`.
const REGISTERED_MIN: usize = 24_042;

impl CommentAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let default_paths = self.paths.is_empty();
        let paths = if default_paths {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths
        };
        let files = match resolve_files(&paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };

        // Arm the ledger for the whole run: the format entry points register each
        // document's comments and the printers' comment seams record each emit.
        // Single-threaded so the thread-local state collects everything.
        comment_ledger::set_comment_check(true);

        let mut violations: Vec<Violation> = Vec::new();
        let mut formatted = 0usize;
        let mut parse_errors = 0usize;
        let mut registered = 0usize;
        let mut unregistered_emits = 0usize;

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
            // Drain any stragglers, then format and finalize this document.
            let _ = comment_ledger::take_comment_ledger();
            if format_source(&source, ParserType::from_extension(&path.to_string_lossy())).is_err()
            {
                parse_errors += 1;
                let _ = comment_ledger::take_comment_ledger();
                continue;
            }
            formatted += 1;
            let ledger = comment_ledger::take_comment_ledger();
            registered += ledger.parsed;
            unregistered_emits += ledger.unregistered_emits;
            for finding in ledger.findings {
                violations.push(Violation {
                    path: path.clone(),
                    finding,
                });
            }
        }

        comment_ledger::set_comment_check(false);

        let stats = Stats {
            formatted,
            parse_errors,
            registered,
            unregistered_emits,
        };
        if self.json {
            print_json(&violations, &stats);
        } else {
            print_report(&violations, &stats);
        }

        if default_paths && registered < REGISTERED_MIN {
            eprintln!(
                "Error: pinned minimum — registered {registered} comments < pinned \
                 {REGISTERED_MIN}. The fixtures walk shrank (or registration collapsed); \
                 if deliberate, re-pin REGISTERED_MIN."
            );
            return Err(CliError::Failed);
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

struct Stats {
    formatted: usize,
    parse_errors: usize,
    registered: usize,
    unregistered_emits: usize,
}

fn kind_label(kind: CommentFindingKind) -> &'static str {
    match kind {
        CommentFindingKind::Dropped => "DROPPED",
        CommentFindingKind::DoublePrinted => "DOUBLE-PRINTED",
    }
}

/// One line of a comment's text, elided — a JSDoc block would otherwise flood the report.
fn preview(text: &str) -> String {
    let first = text.lines().next().unwrap_or("");
    let elided = text.contains('\n');
    let mut out: String = first.chars().take(72).collect();
    if first.chars().count() > 72 || elided {
        out.push('…');
    }
    out
}

fn print_report(violations: &[Violation], stats: &Stats) {
    let Stats {
        formatted,
        parse_errors,
        registered,
        unregistered_emits,
    } = *stats;

    if violations.is_empty() {
        println!(
            "✓ every comment printed exactly once — {registered} comments across {formatted} \
             files ({parse_errors} parse-skipped, {unregistered_emits} unregistered emits)"
        );
        return;
    }

    let dropped = violations
        .iter()
        .filter(|v| v.finding.kind == CommentFindingKind::Dropped)
        .count();
    println!(
        "✗ {} finding(s) across {} file(s) — {dropped} dropped, {} double-printed \
         ({registered} comments, {formatted} formatted, {parse_errors} parse-skipped, \
         {unregistered_emits} unregistered emits)\n",
        violations.len(),
        violations
            .iter()
            .map(|v| v.path.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        violations.len() - dropped,
    );

    for v in violations {
        println!(
            "  {} [{}..{}]",
            v.path.display(),
            v.finding.span.start,
            v.finding.span.end
        );
        println!(
            "    {:<14} {:?}",
            kind_label(v.finding.kind),
            v.finding.text
        );
        if v.finding.kind == CommentFindingKind::DoublePrinted {
            println!("    emitted:       {}", v.finding.emitted);
        }
        println!();
    }

    // Unique (kind, comment) shapes — the dedup'd worklist.
    let mut shapes: BTreeMap<(&'static str, String), usize> = BTreeMap::new();
    for v in violations {
        *shapes
            .entry((kind_label(v.finding.kind), preview(&v.finding.text)))
            .or_default() += 1;
    }
    println!("Unique comment shapes ({}):", shapes.len());
    for ((kind, text), count) in &shapes {
        println!("  {count:>4}×  {kind:<14} {text:?}");
    }
}

fn print_json(violations: &[Violation], stats: &Stats) {
    let items: Vec<serde_json::Value> = violations
        .iter()
        .map(|v| {
            serde_json::json!({
                "path": v.path.to_string_lossy(),
                "kind": kind_label(v.finding.kind),
                "text": v.finding.text,
                "start": v.finding.span.start,
                "end": v.finding.span.end,
                "emitted": v.finding.emitted,
            })
        })
        .collect();
    let output = serde_json::json!({
        "formatted": stats.formatted,
        "parse_skipped": stats.parse_errors,
        "registered": stats.registered,
        "unregistered_emits": stats.unregistered_emits,
        "findings": violations.len(),
        "violations": items,
    });
    #[allow(clippy::unwrap_used)]
    let s = serde_json::to_string_pretty(&output).unwrap();
    println!("{s}");
}
