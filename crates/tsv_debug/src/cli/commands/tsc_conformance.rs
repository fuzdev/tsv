//! tsc_conformance command — ad-hoc queries over the TypeScript-Go conformance
//! baselines (`*.errors.txt`). Pure Rust, no typechecker: tool #1 of the
//! typechecker conformance harness (the "ask important questions" tool). Reads
//! only the committed tsgo baselines — the corpus *inputs* live in a git
//! submodule that is often unmaterialized.

use crate::cli::CliError;
use crate::tsc_conformance::{
    baselines_dir, corpus_materialized, denominators, discover_baselines, histogram, tests_by_code,
};
use argh::FromArgs;
use std::path::PathBuf;

/// REGRESSION PIN (exact): total tsgo .errors.txt baselines. Measured
/// 2026-07-09, ../typescript-go at 168e7015 (_submodules/TypeScript corpus pin
/// 4d4f005c, may be unmaterialized). The checkout is updated deliberately, so any
/// move (a discovery bug, or a typescript-go pull) must be re-pinned here.
const BASELINE_COUNT_PIN: usize = 7033;

/// Query the tsgo TypeScript conformance baselines.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "tsc_conformance")]
pub struct TscConformanceCommand {
    #[argh(subcommand)]
    nested: TscConformanceSub,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
enum TscConformanceSub {
    Query(QueryCommand),
}

/// Answer an ad-hoc question over the baselines.
///
/// Queries: `histogram` (per-code instance counts + totals), `tests-by-code
/// <CODE>` (baselines mentioning a code), `denominators` (test-identity sizing).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "query")]
pub struct QueryCommand {
    /// path to the typescript-go checkout (default: ../typescript-go)
    #[argh(option, default = "PathBuf::from(\"../typescript-go\")")]
    path: PathBuf,

    /// emit JSON instead of a human table
    #[argh(switch)]
    json: bool,

    /// which query: `histogram`, `tests-by-code`, or `denominators`
    #[argh(positional)]
    kind: String,

    /// query arguments (e.g. the error code for `tests-by-code`)
    #[argh(positional)]
    args: Vec<String>,
}

impl TscConformanceCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        match self.nested {
            TscConformanceSub::Query(query) => query.run(),
        }
    }
}

impl QueryCommand {
    fn run(self) -> Result<(), CliError> {
        let dir = baselines_dir(&self.path);
        if !dir.exists() {
            eprintln!(
                "Error: tsgo baselines directory not found: {}",
                dir.display()
            );
            eprintln!();
            eprintln!("Expected a typescript-go checkout with committed baselines. To set it up:");
            eprintln!("  cd .. && git clone https://github.com/microsoft/typescript-go");
            eprintln!("  cd typescript-go && git submodule update --init");
            eprintln!();
            eprintln!("Or specify a custom path:");
            eprintln!(
                "  cargo run -p tsv_debug tsc_conformance query {} --path /path/to/typescript-go",
                self.kind
            );
            return Err(CliError::Failed);
        }

        let baselines = match discover_baselines(&dir) {
            Ok(baselines) => baselines,
            Err(e) => {
                eprintln!("Error discovering baselines: {e}");
                return Err(CliError::Failed);
            }
        };

        match self.kind.as_str() {
            "histogram" => {
                enforce_pin(baselines.len())?;
                let report = histogram(&baselines);
                if self.json {
                    print_json(&report)
                } else {
                    report.print_table();
                    Ok(())
                }
            }
            "denominators" => {
                enforce_pin(baselines.len())?;
                let report = denominators(&baselines);
                if self.json {
                    print_json(&report)
                } else {
                    report.print_summary(corpus_materialized(&self.path));
                    Ok(())
                }
            }
            "tests-by-code" => {
                let Some(code_arg) = self.args.first() else {
                    eprintln!(
                        "Error: `tests-by-code` requires an error code, e.g. `tests-by-code 2454`"
                    );
                    return Err(CliError::Failed);
                };
                let code = parse_code(code_arg)?;
                let report = tests_by_code(&baselines, code);
                if self.json {
                    print_json(&report)
                } else {
                    report.print();
                    Ok(())
                }
            }
            // TODO(tsc_conformance): pin-diff subquery — "what moved between two
            // tsgo refs" (which codes/tests appeared or vanished). Answered
            // manually for this pin; needs two baseline snapshots to diff, so it's
            // deferred to a later slice rather than stubbed with fake data.
            other => {
                eprintln!(
                    "Error: unknown query `{other}`. Valid queries: histogram, tests-by-code <CODE>, denominators."
                );
                Err(CliError::Failed)
            }
        }
    }
}

/// Enforce the baseline-count regression pin (unfiltered `histogram` /
/// `denominators` runs), mirroring `test262`'s hard-fail on a pin mismatch.
fn enforce_pin(count: usize) -> Result<(), CliError> {
    if count != BASELINE_COUNT_PIN {
        eprintln!(
            "Error: pinned count mismatch — discovered {count} .errors.txt baselines ≠ pinned {BASELINE_COUNT_PIN}. \
             If deliberate (a typescript-go pull, a discovery change), re-pin BASELINE_COUNT_PIN."
        );
        return Err(CliError::Failed);
    }
    Ok(())
}

/// Parse an error code, accepting a bare number (`2454`) or a `TS`-prefixed form
/// (`TS2454`, case-insensitive).
fn parse_code(arg: &str) -> Result<u32, CliError> {
    let digits = arg
        .strip_prefix("TS")
        .or_else(|| arg.strip_prefix("ts"))
        .unwrap_or(arg);
    digits.parse().map_err(|_| {
        eprintln!("Error: invalid error code `{arg}` — expected a number like 2454 or TS2454.");
        CliError::Failed
    })
}

/// Serialize a report to pretty JSON on stdout.
fn print_json<T: serde::Serialize>(report: &T) -> Result<(), CliError> {
    match serde_json::to_string_pretty(report) {
        Ok(json) => {
            println!("{json}");
            Ok(())
        }
        Err(e) => {
            eprintln!("Error serializing JSON: {e}");
            Err(CliError::Failed)
        }
    }
}
