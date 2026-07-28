//! tsc_conformance command — ad-hoc queries over the TypeScript-Go conformance
//! baselines (`*.errors.txt`). Pure Rust, no typechecker: tool #1 of the
//! typechecker conformance harness (the "ask important questions" tool). Reads
//! only the committed tsgo baselines — the corpus *inputs* live in a git
//! submodule that is often unmaterialized.

mod gates;
mod pins;
mod report;
#[cfg(test)]
mod test_support;

use crate::cli::CliError;
use crate::tsc_conformance::{
    RunFilter, RunOptions, baselines_dir, check_one, corpus_materialized, denominators,
    discover_baselines, dump_flow_dot, histogram, run_index, run_roundtrip, run_skeleton,
    tests_by_code,
};
use argh::FromArgs;
use std::path::{Path, PathBuf};

use gates::{
    enforce_index_pins, enforce_pin, enforce_run_gates, parse_family_filter,
    refuse_narrowed_update, refuse_red_update,
};
use pins::{
    PRETTY_PATH_PIN, ROUNDTRIP_PASS_PIN, load_pin_snapshot, measured_pins, require_pinned_oracle,
    update_pin_snapshot,
};
use report::{may_write_report, print_json, write_diff_artifacts, write_manifest, write_report};

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
    Index(IndexCommand),
    Run(RunCommand),
    CheckTest(CheckTestCommand),
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

/// Corpus-input self-check (the index gates): index the tsc corpus, expand every
/// test's varyBy variants, and prove three invariants against the on-disk
/// baselines — the join (every baseline maps to one non-skipped variant), the
/// unit-text round-trip (units reproduce the `====` section bodies), and the
/// denominator pins. Zero checker code. Exit 0 only when all three pass and the
/// pins hold (two-sided); filters are not offered — the pins need the full run.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "index")]
pub struct IndexCommand {
    /// path to the typescript-go checkout (default: ../typescript-go)
    #[argh(option, default = "PathBuf::from(\"../typescript-go\")")]
    path: PathBuf,

    /// emit a JSON report instead of the human summary
    #[argh(switch)]
    json: bool,

    /// list every unmatched baseline, mismatch, and unknown directive
    #[argh(switch)]
    verbose: bool,
}

/// Conformance sweep: drive `tsv_check` over every in-scope variant
/// (single-file, non-JSX, non-JS-flavored, not skipped, not an
/// unsupported-option variant) and grade it against tsgo's baselines. The
/// gates: every expect-clean in-scope variant grades clean (zero diagnostics);
/// the bind/merge duplicate-conflict family matches as codes+spans multisets
/// (extra = 0 hard, missing classified by deferred cause); the related-info
/// and lib-base channels stay clean; zero panics; and (on a full run) the
/// pinned denominators + parse-divergence census hold. Runs on a
/// generous-stack worker thread; each test's check is `catch_unwind`-contained.
/// Exit 0 only when the invariants hold and (on a full run) the pins match.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "run")]
pub struct RunCommand {
    /// path to the typescript-go checkout (default: ../typescript-go)
    #[argh(option, default = "PathBuf::from(\"../typescript-go\")")]
    path: PathBuf,

    /// emit a JSON report instead of the human summary
    #[argh(switch)]
    json: bool,

    /// triage filter: keep only tests whose relative path contains this substring
    /// (SKIPS the pins)
    #[argh(option)]
    test: Option<String>,

    /// triage filter: keep only variants whose baseline carries this TS code
    /// (SKIPS the pins)
    #[argh(option)]
    code: Option<u32>,

    /// triage filter: keep only variants whose config has this `key=value`
    /// (SKIPS the pins)
    #[argh(option)]
    variant: Option<String>,

    /// triage filter: keep only variants whose baseline carries a code in this
    /// sub-family — `dup`, `flow`, or `all` (SKIPS the pins)
    #[argh(option)]
    family: Option<String>,

    /// write a JSON manifest of every graded variant (per-variant verdict + buckets
    /// + census + pins) to this path — the tsc analog of `test262 --emit-manifest`
    #[argh(option)]
    emit_manifest: Option<PathBuf>,

    /// write the committed compact report to `<path>.json` + `<path>.md` (full runs
    /// only; deterministic, wall-clock excluded)
    #[argh(option)]
    report: Option<PathBuf>,

    /// rewrite the committed pin snapshot from this run's measured counts (full,
    /// unfiltered runs only; refuses to pin a run whose invariant gates are red)
    #[argh(switch)]
    update: bool,
}

/// Inner dev loop: run one corpus test (optionally one variant) through
/// `tsv_check` and print our diagnostics vs the baseline summary.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "check-test")]
pub struct CheckTestCommand {
    /// path to the typescript-go checkout (default: ../typescript-go)
    #[argh(option, default = "PathBuf::from(\"../typescript-go\")")]
    path: PathBuf,

    /// select one variant, `key=value` (e.g. `target=es2015`)
    #[argh(option)]
    variant: Option<String>,

    /// emit a JSON report instead of the human diff
    #[argh(switch)]
    json: bool,

    /// dump the first unit's control-flow graph as Graphviz DOT (F1) to stdout,
    /// instead of the diagnostic diff
    #[argh(switch)]
    dump_flow: bool,

    /// the test to run (exact relative path or basename)
    #[argh(positional)]
    name: String,
}

impl TscConformanceCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        match self.nested {
            TscConformanceSub::Query(query) => query.run(),
            TscConformanceSub::Roundtrip(rt) => rt.run(),
            TscConformanceSub::Index(index) => index.run(),
            TscConformanceSub::Run(run) => run.run(),
            TscConformanceSub::CheckTest(check) => check.run(),
        }
    }
}

impl RunCommand {
    fn run(self) -> Result<(), CliError> {
        require_corpus(&self.path)?;
        // Parse the snapshot up front: a malformed pin file must fail before the sweep,
        // not after it (an `--update` run recovers instead — see `load_pin_snapshot`).
        let pins = load_pin_snapshot(self.update)?;

        let filter = self.build_filter()?;
        let filtered = filter.is_active();
        // The committed report is the full-run artifact; refuse to write a partial one.
        if !may_write_report(self.report.is_some(), filtered) {
            eprintln!(
                "Error: --report writes the committed full report; it cannot be combined with \
                 --test/--code/--variant filters."
            );
            return Err(CliError::Failed);
        }
        refuse_narrowed_update(self.update, &filter)?;
        if self.update {
            require_pinned_oracle(&self.path)?;
        }

        let options = RunOptions {
            filter,
            collect_manifest: self.emit_manifest.is_some(),
        };
        let report = run_skeleton(&self.path, &options).map_err(|e| {
            eprintln!("Error running skeleton sweep: {e}");
            CliError::Failed
        })?;
        if self.json {
            print_json(&report)?;
        } else {
            report.print();
        }

        // Filters skip the exact pins (the roundtrip/query convention); the invariant
        // gates still hold. An `--update` run is unfiltered by construction and grades
        // against everything EXCEPT the snapshot counts — it may re-pin drift, never a
        // red run. Committed artifacts land only when the gates pass (so a pin miss
        // never writes a bad manifest/report), while a failure dumps per-test diff
        // artifacts for triage.
        let gates = if self.update {
            refuse_red_update(&report)
        } else {
            enforce_run_gates(&report, &pins, !filtered)
        };
        match gates {
            Ok(()) => {
                // Post-update artifacts state the pins the run MEASURED — the values
                // just written — so an artifact never quotes a stale snapshot.
                let effective = if self.update {
                    update_pin_snapshot(&pins, &report)?;
                    measured_pins(&report)
                } else {
                    pins
                };
                if let Some(path) = &self.emit_manifest {
                    write_manifest(&report, &options.filter, &effective, path)?;
                }
                if let Some(path) = &self.report {
                    write_report(&report, &effective, path)?;
                }
                Ok(())
            }
            Err(e) => {
                write_diff_artifacts(&report);
                Err(e)
            }
        }
    }

    /// Build the triage filter from the CLI flags (lowercasing the `--variant` key,
    /// which the config maps store lowercased).
    fn build_filter(&self) -> Result<RunFilter, CliError> {
        let variant = match self.variant.as_deref().map(parse_variant_filter) {
            Some(Ok((k, v))) => Some((k.to_lowercase(), v)),
            Some(Err(e)) => {
                eprintln!("{e}");
                return Err(CliError::Failed);
            }
            None => None,
        };
        let family = match self.family.as_deref().map(parse_family_filter) {
            Some(Ok(f)) => Some(f),
            Some(Err(e)) => {
                eprintln!("{e}");
                return Err(CliError::Failed);
            }
            None => None,
        };
        Ok(RunFilter {
            test: self.test.clone(),
            code: self.code,
            variant,
            family,
        })
    }
}

impl CheckTestCommand {
    fn run(self) -> Result<(), CliError> {
        require_corpus(&self.path)?;
        if self.dump_flow {
            let dot = dump_flow_dot(&self.path, &self.name).map_err(|e| {
                eprintln!("Error: {e}");
                CliError::Failed
            })?;
            print!("{dot}");
            return Ok(());
        }
        let variant = match self.variant.as_deref().map(parse_variant_filter) {
            Some(Ok(v)) => Some(v),
            Some(Err(e)) => {
                eprintln!("{e}");
                return Err(CliError::Failed);
            }
            None => None,
        };
        let report = check_one(&self.path, &self.name, variant).map_err(|e| {
            eprintln!("Error: {e}");
            CliError::Failed
        })?;
        if self.json {
            print_json(&report)
        } else {
            report.print();
            Ok(())
        }
    }
}

/// Parse a `key=value` variant filter.
fn parse_variant_filter(arg: &str) -> Result<(String, String), String> {
    arg.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("Error: --variant expects key=value, got {arg:?}"))
}

/// Fail (with the submodule hint) when the corpus inputs are not materialized —
/// both `run` and `check-test` need them, unlike the baseline-only tools.
fn require_corpus(path: &Path) -> Result<(), CliError> {
    if corpus_materialized(path) {
        return Ok(());
    }
    eprintln!(
        "Error: the tsc corpus inputs are not materialized under {}.",
        path.display()
    );
    eprintln!("Run `git submodule update --init` in ../typescript-go to materialize them.");
    Err(CliError::Failed)
}

impl IndexCommand {
    fn run(self) -> Result<(), CliError> {
        // The corpus inputs must be materialized (unlike the baseline-only query
        // and roundtrip tools).
        require_corpus(&self.path)?;
        let report = run_index(&self.path).map_err(|e| {
            eprintln!("Error indexing corpus: {e}");
            CliError::Failed
        })?;

        if self.json {
            print_json(&report)?;
        } else {
            report.print(self.verbose);
        }

        enforce_index_pins(&report)
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
    checkout: &Path,
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
