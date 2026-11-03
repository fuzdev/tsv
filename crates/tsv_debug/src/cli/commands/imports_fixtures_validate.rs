//! `imports_fixtures_validate` — grade the import fixtures: oracle freshness and tsv.

use super::imports_fixtures_update::{oracle_expected, walk_and_filter};
use crate::cli::CliError;
use crate::diff::{DiffOptions, diff_to_string};
use crate::imports_fixtures::{ImportsFixture, ours_json, render_expected};
use argh::FromArgs;
use futures_util::StreamExt;

/// Validate the import fixtures (`tests/fixtures_imports/`).
///
/// Per fixture, all gating: (a) **structure** — its input, `expected.json` and `README.md`,
/// at most an `oracle.txt`, nothing else; (b) **oracle freshness** — the oracle's records,
/// regenerated in memory, equal the committed `expected.json` (the only place freshness
/// is graded: `cargo test` compares tsv to the committed file alone); (c) **tsv** —
/// `parse_imports` + the JSON writer produce the committed records.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "imports_fixtures_validate")]
pub struct ImportsFixturesValidateCommand {
    /// print the diff of every failing check
    #[argh(switch, short = 'v')]
    verbose: bool,

    /// fixture path patterns (case-insensitive substrings; multiple = OR)
    #[argh(positional)]
    patterns: Vec<String>,
}

/// One fixture's verdicts.
struct Report {
    fixture: String,
    /// Structure problems, an unreadable committed file, or an oracle error.
    problems: Vec<String>,
    /// The oracle's records equal the committed ones (`false` also when it errored).
    oracle_fresh: bool,
    /// tsv's records equal the committed ones (`false` also when tsv errored).
    tsv_matches: bool,
    /// The diffs behind a stale oracle or a tsv mismatch, shown under `-v`.
    diffs: Vec<String>,
}

impl ImportsFixturesValidateCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let fixtures = walk_and_filter(&self.patterns)?;
        let rt = super::create_runtime();
        let verbose = self.verbose;
        rt.block_on(async move {
            let mut stream = super::spawn_work_stream(
                fixtures,
                super::ResultOrder::Input,
                |fixture| async move { validate_one(&fixture).await },
            );
            let mut reports = Vec::new();
            while let Some(joined) = stream.next().await {
                reports.push(super::task_result(joined, "imports-validate")?);
            }
            let (mut stale, mut mismatched, mut broken) = (0, 0, 0);
            for report in &reports {
                let passed =
                    report.problems.is_empty() && report.oracle_fresh && report.tsv_matches;
                let mark = if passed { "✓" } else { "✗" };
                let oracle = if report.oracle_fresh {
                    "fresh"
                } else {
                    "STALE"
                };
                let tsv = if report.tsv_matches {
                    "match"
                } else {
                    "MISMATCH"
                };
                println!("{mark} {} [oracle: {oracle}, tsv: {tsv}]", report.fixture);
                for problem in &report.problems {
                    eprintln!("    {problem}");
                }
                if verbose {
                    for diff in &report.diffs {
                        eprintln!("{diff}");
                    }
                }
                stale += usize::from(!report.oracle_fresh);
                mismatched += usize::from(!report.tsv_matches);
                broken += usize::from(!report.problems.is_empty());
            }
            println!(
                "\n{} import fixture(s): {stale} oracle-stale, {mismatched} tsv mismatch(es), \
                 {broken} with problems",
                reports.len()
            );
            if stale + mismatched + broken > 0 {
                Err(CliError::Failed)
            } else {
                Ok(())
            }
        })
    }
}

async fn validate_one(fixture: &ImportsFixture) -> Report {
    let mut report = Report {
        fixture: fixture.relative_path.clone(),
        problems: fixture.structure_problems(),
        oracle_fresh: false,
        tsv_matches: false,
        diffs: Vec::new(),
    };
    let committed = match std::fs::read_to_string(fixture.expected_path()) {
        Ok(text) => text,
        Err(e) => {
            report.problems.push(format!("reading expected.json: {e}"));
            return report;
        }
    };
    let committed_value: serde_json::Value = match crate::json::from_str(&committed) {
        Ok(value) => value,
        Err(e) => {
            report
                .problems
                .push(format!("expected.json does not parse: {e}"));
            return report;
        }
    };
    let diff_against = |label: &str, value: &serde_json::Value| {
        let rendered = render_expected(value).unwrap_or_default();
        format!(
            "  {} {label}:\n{}",
            fixture.relative_path,
            diff_to_string(&committed, &rendered, &DiffOptions::freshness())
        )
    };
    match oracle_expected(fixture).await {
        Ok(oracle) => {
            report.oracle_fresh = oracle == committed_value;
            if !report.oracle_fresh {
                report.diffs.push(diff_against("oracle", &oracle));
            }
        }
        Err(e) => report.problems.push(format!("oracle: {e}")),
    }
    match fixture
        .read_input()
        .and_then(|source| ours_json(fixture.language, &source))
    {
        Ok(ours) => {
            report.tsv_matches = ours == committed_value;
            if !report.tsv_matches {
                report.diffs.push(diff_against("tsv", &ours));
            }
        }
        Err(e) => report.problems.push(format!("tsv: {e}")),
    }
    report
}
