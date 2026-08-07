use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::audit::sweep::{PristineSweep, sweep_pristine_armed};
use crate::audit::vacuity::{FIXTURES_FORMATTED_MIN, check_formatted_min, check_graded_nonzero};
use crate::cli::CliError;
use tsv_lang::comment_ledger::{self, CommentFinding, CommentFindingKind};

use super::profile::resolve_seed_files;

/// Audit that every parsed comment is printed exactly once.
///
/// Enables the print-once comment ledger (`tsv_lang::comment_ledger`) and formats each
/// file, reporting every comment the format DROPPED (parsed, never emitted — silent
/// content loss) or DOUBLE-PRINTED. Pure Rust — no Deno. Defaults to `tests/fixtures`
/// when no paths are given.
///
/// The corpus walk is the shared [`sweep_pristine_armed`], as in its twin
/// `swallow_audit` — so the skip buckets mean the same thing here as in the other
/// as-authored audits, and a formatter panic mid-walk is caught and named rather
/// than killing the run. Panics are counted, not gated: the panic gates own that class.
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
/// default (`tests/fixtures`) run — a corpus that SHRANK, or a registration that partly
/// collapsed, still reports "no findings" and would otherwise pass. A minimum, not a
/// two-sided pin, because the fixtures tree is COMMITTED and grows with ordinary fixture
/// PRs (`deno task check` must not fail per added fixture); shrinkage/collapse fails.
/// Re-pin to current when it trips. Same ritual as [`FIXTURES_FORMATTED_MIN`] and
/// `benches/js/lib/gate_counts.ts`.
///
/// It answers a question the shared file pin cannot: **registration** collapsing without
/// the file count moving (a format entry point that stops registering a carrier still
/// formats every file). The converse holds too, which is why this audit passes BOTH —
/// sharing the sweep means its `formatted` is the other four's by construction, so a skip
/// policy that diverged here would drop below [`FIXTURES_FORMATTED_MIN`] and say so
/// instead of hiding in this pin's slack. That slack is the reason to keep re-pinning
/// tight: left at its first measured value it drifted 27% below the live count, a gap
/// wide enough to swallow a quarter of the corpus in silence.
///
/// Only a default run can be held to a number, so both pins stay `default_paths`-gated;
/// the floor *under* them — zero graded files, vacuous at any scope — is
/// [`check_graded_nonzero`], called unconditionally above.
const REGISTERED_MIN: usize = 33_138;

impl CommentAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let default_paths = self.paths.is_empty();
        let files = resolve_seed_files(&self.paths, 0)?;

        // Arm the ledger for the whole run: the format entry points register each
        // document's comments and the printers' comment seams record each emit.
        // Single-threaded so the thread-local state collects everything.
        comment_ledger::set_comment_check(true);

        let mut violations: Vec<Violation> = Vec::new();
        let mut registered = 0usize;
        let mut unregistered_emits = 0usize;
        let sweep = sweep_pristine_armed(
            &files,
            // Drain before each format, so a ledger left behind by a seed the sweep
            // skipped (rejected, panicked) can't be attributed to the next file.
            || {
                let _ = comment_ledger::take_comment_ledger();
            },
            |path, _parser, _source, _output| {
                let ledger = comment_ledger::take_comment_ledger();
                registered += ledger.parsed;
                unregistered_emits += ledger.unregistered_emits;
                for finding in ledger.findings {
                    violations.push(Violation {
                        path: path.to_path_buf(),
                        finding,
                    });
                }
            },
        );

        comment_ledger::set_comment_check(false);

        let stats = Stats {
            sweep,
            registered,
            unregistered_emits,
        };
        if self.json {
            print_json(&violations, &stats);
        } else {
            print_report(&violations, &stats);
        }
        stats.sweep.print_panic_sample();

        check_graded_nonzero(stats.sweep.formatted, "files formatted")?;
        if default_paths {
            check_formatted_min(stats.sweep.formatted, FIXTURES_FORMATTED_MIN)?;
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
    /// The shared skip/format bookkeeping (the [`check_graded_nonzero`] vacuity
    /// floor reads `formatted`; the report tail reads `skipped_note`).
    sweep: PristineSweep,
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
        sweep,
        registered,
        unregistered_emits,
    } = stats;
    let formatted = sweep.formatted;
    let skipped = sweep.skipped_note();

    if violations.is_empty() {
        println!(
            "✓ every comment printed exactly once — {registered} comments across {formatted} \
             files ({skipped}, {unregistered_emits} unregistered emits)"
        );
        return;
    }

    let dropped = violations
        .iter()
        .filter(|v| v.finding.kind == CommentFindingKind::Dropped)
        .count();
    println!(
        "✗ {} finding(s) across {} file(s) — {dropped} dropped, {} double-printed \
         ({registered} comments, {formatted} formatted, {skipped}, \
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
        // The skip-∧-dropped join: the `BlockOnly`-filtered builder call site(s) that
        // passed over this comment — the responsible licence holder, named at the site.
        for site in &v.finding.skip_sites {
            println!("    skipped by:    {site}");
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
                "skip_sites": v.finding.skip_sites,
            })
        })
        .collect();
    let output = stats.sweep.json_report(
        serde_json::json!({}),
        serde_json::json!({
            "registered": stats.registered,
            "unregistered_emits": stats.unregistered_emits,
            "findings": violations.len(),
            "violations": items,
        }),
    );
    #[allow(clippy::unwrap_used)]
    let s = serde_json::to_string_pretty(&output).unwrap();
    println!("{s}");
}
