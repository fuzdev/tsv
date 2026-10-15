use crate::cli::CliError;
use crate::strip_fixtures::{REPIN_HINT, known_path, run_gate, write_known};
use argh::FromArgs;

/// Grade `tsv_ts::strip` against the strip fixtures (`tests/fixtures_strip/`).
///
/// Per fixture: the output equals the committed `expected.*` (or the refusal matches
/// `refusal.txt`), the output keeps every surviving character's line and UTF-16 column and
/// every line terminator, and stripping it again returns it borrowed. Failures are graded
/// against the known-failure ratchet `strip_fixtures_known.txt` beside this command: a new
/// one fails, and so does a pinned one that stopped failing. A broken fixture or a panic
/// always fails. The same gate runs in `cargo test` (`tests/strip_fixtures_tests.rs`); the
/// committed files' agreement with the oracles is `deno task strip:fixtures:oracle`'s.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "strip_fixtures_validate")]
pub struct StripFixturesValidateCommand {
    /// re-pin the known-failure snapshot from this run (refused when patterns narrow it)
    #[argh(switch)]
    update: bool,

    /// print every failing check, pinned or not
    #[argh(switch, short = 'v')]
    verbose: bool,

    /// fixture path patterns (case-insensitive substrings; multiple = OR)
    #[argh(positional)]
    patterns: Vec<String>,
}

impl StripFixturesValidateCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if self.update && !self.patterns.is_empty() {
            eprintln!(
                "Error: --update pins the FULL fixture tree; patterns narrow this run to a \
                 subset, and writing it would silently unpin real failures. Re-run without \
                 patterns."
            );
            return Err(CliError::Failed);
        }
        let report = run_gate(&self.patterns).map_err(|e| {
            eprintln!("Error: {e}");
            CliError::Failed
        })?;
        let new: std::collections::BTreeSet<_> = report.new.iter().collect();
        for finding in &report.findings {
            let pinned = finding.key.check.is_pinnable() && !new.contains(&finding.key);
            if self.verbose || !pinned {
                let mark = if pinned { "○ pinned" } else { "✗" };
                println!("{mark} {}: {}", finding.key, finding.detail);
            }
        }
        if self.update {
            write_known(&report.pinnable).map_err(|e| {
                eprintln!("Error: {e}");
                CliError::Failed
            })?;
            println!(
                "✓ wrote {} known failure(s) to {}",
                report.pinnable.len(),
                known_path().display()
            );
            if !report.unpinnable.is_empty() {
                eprintln!(
                    "✗ {} unpinnable failure(s) (FIXTURE / PANIC) — never written, always failing",
                    report.unpinnable.len()
                );
                return Err(CliError::Failed);
            }
            return Ok(());
        }
        for key in &report.stale {
            eprintln!("✗ STALE pin (no longer fails — re-pin): {key}");
        }
        if report.holds() {
            println!(
                "\n✓ {} strip fixture(s): ratchet holds — {} known failure(s), no new ones",
                report.fixtures, report.known
            );
            return Ok(());
        }
        eprintln!(
            "\n✗ {} strip fixture(s): {} new, {} stale, {} unpinnable — re-pin with `{REPIN_HINT}` \
             once the change is deliberate",
            report.fixtures,
            report.new.len(),
            report.stale.len(),
            report.unpinnable.len()
        );
        Err(CliError::Failed)
    }
}
