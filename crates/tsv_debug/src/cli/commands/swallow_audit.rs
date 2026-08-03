use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::audit::sweep::{
    FIXTURES_FORMATTED_MIN, PristineSweep, check_formatted_min, sweep_pristine_armed,
};
use crate::cli::CliError;
use tsv_lang::doc::swallow::{self, SwallowReport};

use super::profile::resolve_seed_files;

/// Audit for line comments that swallow the following token.
///
/// Enables the render-time swallow check (`tsv_lang::doc::swallow`) and formats
/// each file, reporting every spot where a `//` line comment is followed by
/// content on the same physical output line (silent content loss). Pure Rust —
/// no Deno. Defaults to `tests/fixtures` when no paths are given.
///
/// The corpus walk is the shared [`sweep_pristine_armed`], so the skip buckets
/// (and the vacuity pin they feed) mean the same thing here as in the other
/// as-authored audits. Panics are counted, not gated — the panic gates own that
/// class, and over this corpus `roundtrip:audit` formats the same tree without a
/// `catch_unwind` at all.
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
    pub(crate) fn run(self) -> Result<(), CliError> {
        let default_paths = self.paths.is_empty();
        let files = resolve_seed_files(&self.paths, 0)?;

        // Enable the check for the whole run; the builder records line-comment
        // ids and the renderer flags swallows. Single-threaded so the
        // thread-local report sink collects everything.
        swallow::set_swallow_check(true);

        let mut violations: Vec<Violation> = Vec::new();
        let sweep = sweep_pristine_armed(
            &files,
            // Drain before each format, so a report left behind by a seed the
            // sweep skipped (rejected, panicked) can't be attributed to the
            // next file.
            || {
                let _ = swallow::take_swallow_reports();
            },
            |path, _parser, _source, _output| {
                for report in swallow::take_swallow_reports() {
                    violations.push(Violation {
                        path: path.to_path_buf(),
                        report,
                    });
                }
            },
        );

        swallow::set_swallow_check(false);

        if self.json {
            print_json(&violations, &sweep);
        } else {
            print_report(&violations, &sweep);
        }

        if default_paths {
            check_formatted_min(sweep.formatted, FIXTURES_FORMATTED_MIN)?;
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

fn print_report(violations: &[Violation], sweep: &PristineSweep) {
    let formatted = sweep.formatted;
    let skipped = sweep.skipped_note();
    if violations.is_empty() {
        println!("✓ no line-comment swallows across {formatted} files ({skipped})");
        return;
    }

    println!(
        "✗ {} swallow(s) across {} file(s) ({formatted} formatted, {skipped})\n",
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

fn print_json(violations: &[Violation], sweep: &PristineSweep) {
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
        "formatted": sweep.formatted,
        "parse_skipped": sweep.parse_errors,
        "read_skipped": sweep.read_errors,
        "panicked": sweep.panics,
        "swallows": violations.len(),
        "violations": items,
    });
    #[allow(clippy::unwrap_used)]
    let s = serde_json::to_string_pretty(&output).unwrap();
    println!("{s}");
}
