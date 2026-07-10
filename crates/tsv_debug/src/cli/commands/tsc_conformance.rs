//! tsc_conformance command — ad-hoc queries over the TypeScript-Go conformance
//! baselines (`*.errors.txt`). Pure Rust, no typechecker: tool #1 of the
//! typechecker conformance harness (the "ask important questions" tool). Reads
//! only the committed tsgo baselines — the corpus *inputs* live in a git
//! submodule that is often unmaterialized.

use crate::cli::CliError;
use crate::tsc_conformance::{
    baselines_dir, corpus_materialized, denominators, discover_baselines, histogram, run_roundtrip,
    tests_by_code,
};
use argh::FromArgs;
use std::path::PathBuf;

/// REGRESSION PIN (exact): total tsgo .errors.txt baselines. Measured
/// 2026-07-09, ../typescript-go at 168e7015 (_submodules/TypeScript corpus pin
/// 4d4f005c, may be unmaterialized). The checkout is updated deliberately, so any
/// move (a discovery bug, or a typescript-go pull) must be re-pinned here.
const BASELINE_COUNT_PIN: usize = 7033;

/// REGRESSION PIN (exact): baselines that round-trip byte-identically
/// (`parse → render == input`). Measured vs pin 168e7015: 7033 — the **full**
/// baseline set (100%, plain + pretty paths together, i.e. `BASELINE_COUNT_PIN`).
/// A move in either direction is a deliberate re-pin (a parser/renderer change,
/// or a typescript-go pull); pin two-sided so drift can't hide.
const ROUNDTRIP_PASS_PIN: usize = 7033;

/// REGRESSION PIN (exact): baselines that take the ANSI-colored `pretty=true`
/// path (its own model, parser, and colored renderer). In scope and folded into
/// the pass count; pinned so the pretty set can't grow or shrink silently on a
/// typescript-go pull.
const PRETTY_PATH_PIN: usize = 14;

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
    Roundtrip(RoundtripCommand),
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

/// Round-trip self-check (the P0 gate): parse → re-render → byte-compare every
/// tsgo baseline. Prints files checked, byte-identical count, pass rate, and a
/// failure-bucket taxonomy. Exit 0 only on the pinned pass count (two-sided).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "roundtrip")]
pub struct RoundtripCommand {
    /// path to the typescript-go checkout (default: ../typescript-go)
    #[argh(option, default = "PathBuf::from(\"../typescript-go\")")]
    path: PathBuf,

    /// emit a JSON report instead of the human summary
    #[argh(switch)]
    json: bool,

    /// list every failing baseline path
    #[argh(switch)]
    verbose: bool,

    /// baseline path substrings to include (OR); default: all baselines
    #[argh(positional)]
    filters: Vec<String>,
}

impl TscConformanceCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        match self.nested {
            TscConformanceSub::Query(query) => query.run(),
            TscConformanceSub::Roundtrip(rt) => rt.run(),
        }
    }
}

impl RoundtripCommand {
    fn run(self) -> Result<(), CliError> {
        let baselines = load_baselines(&self.path, "roundtrip")?;
        let filtered = filter_baselines(baselines, &self.filters);
        let unfiltered = self.filters.is_empty();

        // The pins only apply to a full (unfiltered) run.
        if unfiltered {
            enforce_pin(filtered.len())?;
        }

        let report = run_roundtrip(&filtered);
        if self.json {
            print_json(&report)?;
        } else {
            report.print(self.verbose);
        }

        // On a full run, gate three exact invariants (all two-sided):
        //  1. round-trip is 100% (no baseline regressed),
        //  2. the pass count matches its pin,
        //  3. the pretty-path count matches its pin (the colored set is stable).
        if unfiltered {
            let mut errs: Vec<String> = Vec::new();
            if report.byte_identical != report.files_checked {
                errs.push(format!(
                    "round-trip not 100% — {} of {} passed",
                    report.byte_identical, report.files_checked
                ));
            }
            if report.byte_identical != ROUNDTRIP_PASS_PIN {
                errs.push(format!(
                    "pass count {} != pinned {ROUNDTRIP_PASS_PIN}",
                    report.byte_identical
                ));
            }
            if report.pretty_path != PRETTY_PATH_PIN {
                errs.push(format!(
                    "pretty-path count {} != pinned {PRETTY_PATH_PIN}",
                    report.pretty_path
                ));
            }
            if !errs.is_empty() {
                eprintln!(
                    "\nError: {}. If deliberate (a parser/renderer change, or a typescript-go \
                     pull), re-pin ROUNDTRIP_PASS_PIN / PRETTY_PATH_PIN.",
                    errs.join("; ")
                );
                return Err(CliError::Failed);
            }
        }
        Ok(())
    }
}

/// Keep only baselines whose relative path contains any filter substring (OR);
/// an empty filter list keeps everything.
fn filter_baselines(
    baselines: Vec<crate::tsc_conformance::discovery::Baseline>,
    filters: &[String],
) -> Vec<crate::tsc_conformance::discovery::Baseline> {
    if filters.is_empty() {
        return baselines;
    }
    baselines
        .into_iter()
        .filter(|b| filters.iter().any(|f| b.relative_path.contains(f.as_str())))
        .collect()
}

/// Discover the tsgo baselines under `checkout`, printing the setup help and
/// failing if the checkout (or its baselines directory) is missing.
///
/// `example` names the subcommand for the "Or specify a custom path" hint.
fn load_baselines(
    checkout: &std::path::Path,
    example: &str,
) -> Result<Vec<crate::tsc_conformance::discovery::Baseline>, CliError> {
    let dir = baselines_dir(checkout);
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
            "  cargo run -p tsv_debug tsc_conformance {example} --path /path/to/typescript-go"
        );
        return Err(CliError::Failed);
    }
    discover_baselines(&dir).map_err(|e| {
        eprintln!("Error discovering baselines: {e}");
        CliError::Failed
    })
}

impl QueryCommand {
    fn run(self) -> Result<(), CliError> {
        let baselines = load_baselines(&self.path, &format!("query {}", self.kind))?;

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
