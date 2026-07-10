//! tsc_conformance command — ad-hoc queries over the TypeScript-Go conformance
//! baselines (`*.errors.txt`). Pure Rust, no typechecker: tool #1 of the
//! typechecker conformance harness (the "ask important questions" tool). Reads
//! only the committed tsgo baselines — the corpus *inputs* live in a git
//! submodule that is often unmaterialized.

use crate::cli::CliError;
use crate::tsc_conformance::index::IndexReport;
use crate::tsc_conformance::runner::SkeletonReport;
use crate::tsc_conformance::{
    baselines_dir, check_one, corpus_materialized, denominators, discover_baselines, histogram,
    run_index, run_roundtrip, run_skeleton, tests_by_code,
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

/// REGRESSION PINS (exact, two-sided) for the `index` corpus-input self-checks.
/// Measured 2026-07-10, ../typescript-go at 168e7015 (`_submodules/TypeScript`
/// corpus materialized). Every move is a deliberate re-pin (a harness-port change,
/// or a typescript-go pull). The corpus files:
const INDEX_TOTAL_SCANNED_PIN: usize = 12445;
const INDEX_TS_PIN: usize = 12114;
const INDEX_TSX_PIN: usize = 330;
const INDEX_JS_PIN: usize = 1;
/// Static test-level skips (`skippedTests`) and per-directory sizing.
const INDEX_SKIPPED_TESTS_PIN: usize = 45;
const INDEX_SINGLE_FILE_PIN: usize = 10388;
const INDEX_MULTI_FILE_PIN: usize = 2012;
/// Selection-predicate denominators.
const INDEX_JSX_SCOPED_PIN: usize = 379;
const INDEX_JS_FLAVORED_PIN: usize = 934;
const INDEX_PRETTY_TESTS_PIN: usize = 14;
const INDEX_BASENAME_COLLISIONS_PIN: usize = 0;
const INDEX_CAP_EXCEEDED_PIN: usize = 0;
/// varyBy include values with no normalized identity (tsgo hard-fails on each; the
/// harness keeps them as graceful `Other` variants). Zero on the pinned corpus — a
/// nonzero count is a phantom-variant signal from a corpus pull, not a clean move.
const INDEX_UNKNOWN_INCLUDES_PIN: usize = 0;
/// Variant sizing: total variants, the variant-level (unsupported-option) skips,
/// the non-skipped variants, and the expect-clean count.
const INDEX_VARIANT_TOTAL_PIN: usize = 14916;
const INDEX_SKIPPED_VARIANTS_PIN: usize = 2068;
const INDEX_NONSKIP_VARIANTS_PIN: usize = 12848;
const INDEX_EXPECT_CLEAN_PIN: usize = 5815;
/// Gate 1 (baseline join): every on-disk baseline matches one non-skipped variant.
const INDEX_JOIN_MATCHED_PIN: usize = 7033;
/// Gate 2 (unit-text round-trip): non-pretty baselined tests whose units reproduce
/// their section bodies, and the pretty baselines carved out.
const INDEX_UNIT_ROUNDTRIP_PIN: usize = 7019;
const INDEX_UNIT_ROUNDTRIP_PRETTY_PIN: usize = 14;

/// REGRESSION PINS (exact, two-sided) for the walking-skeleton sweep (`run`).
/// Measured 2026-07-10, ../typescript-go at 168e7015 (`_submodules/TypeScript`
/// corpus materialized). The checker emits nothing yet, so the meaningful gate
/// is `clean_pass == expect_clean_graded` with zero panics; the counts below pin
/// the in-scope denominators + parse-divergence census so any drift (a
/// harness-port change, a tsv parser change, or a typescript-go pull) forces a
/// deliberate re-pin.
const RUN_IN_SCOPE_TESTS_PIN: usize = 9388;
const RUN_IN_SCOPE_VARIANTS_PIN: usize = 9887;
const RUN_EXPECT_CLEAN_PIN: usize = 4435;
const RUN_BASELINED_PARSED_PIN: usize = 4446;
const RUN_PARSE_REJECTED_PIN: usize = 1006;
const RUN_PARSE_REJECTED_NO_BASELINE_PIN: usize = 45;
const RUN_PARSE_REJECTED_TS1XXX_PIN: usize = 451;
const RUN_PARSE_REJECTED_OTHER_PIN: usize = 510;
const RUN_SCRIPT_RETRY_PIN: usize = 25;
/// Tracked parser crashes carved out of the sweep (the `CRASH_EXCLUSIONS`
/// ledger). Pinned so the ledger can't grow or shrink silently — a move means a
/// tsv parser robustness change (a fix removes an entry; a regression adds one).
const RUN_CRASH_EXCLUDED_PIN: usize = 1;

/// REGRESSION PINS (exact, two-sided) for the family grading (the S3 gate).
/// Measured 2026-07-10 vs pin 168e7015. `family_extra` is gated to 0 (hard); the
/// rest pin the buckets so any move (a cascade change, a tsv parser change, a
/// typescript-go pull) forces a deliberate re-pin. The missing bucket is
/// classified: `merge` (merge-phase family, S4), `lib` (absent-lib conflicts,
/// S5), and `check-time` (checker-emitted TS2300/2451 the bind-only slice can't
/// produce — duplicate members, type parameters, computed/private names). A drop
/// in `check-time` (matches gained) or `merge`/`lib` is a real improvement that
/// re-pins; a rise is a regression to explain.
const RUN_FAMILY_GRADED_PIN: usize = 4066;
const RUN_FAMILY_POSITIVE_PIN: usize = 125;
const RUN_FAMILY_MATCH_PIN: usize = 414;
const RUN_FAMILY_MISSING_PIN: usize = 136;
const RUN_MISSING_MERGE_PIN: usize = 7;
const RUN_MISSING_LIB_PIN: usize = 4;
const RUN_MISSING_CHECKTIME_PIN: usize = 125;
const RUN_FAMILY_SPAN_MISMATCH_PIN: usize = 0;
const RUN_CARVE_OUT_RULE_A_PIN: usize = 380;
const RUN_CARVE_OUT_RULE_A_FAMILY_PIN: usize = 9;
const RUN_MODULE_DETECTION_PIN: usize = 1;

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

/// Corpus-input self-check (the S1 gates): index the tsc corpus, expand every
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

/// Walking-skeleton sweep (the S2 gate): drive `tsv_check` over every in-scope
/// variant (single-file, non-JSX, non-JS-flavored, not skipped, not an
/// unsupported-option variant) and grade the checker plumbing end-to-end. The
/// checker emits nothing yet, so the gate is: every expect-clean in-scope
/// variant grades clean (zero diagnostics), zero panics, and the pinned
/// denominators + parse-divergence census hold. Runs on a generous-stack worker
/// thread; each test's check is `catch_unwind`-contained. Exit 0 only when the
/// invariants hold and (on a full run) the pins match.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "run")]
pub struct RunCommand {
    /// path to the typescript-go checkout (default: ../typescript-go)
    #[argh(option, default = "PathBuf::from(\"../typescript-go\")")]
    path: PathBuf,

    /// emit a JSON report instead of the human summary
    #[argh(switch)]
    json: bool,
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
        let report = run_skeleton(&self.path).map_err(|e| {
            eprintln!("Error running skeleton sweep: {e}");
            CliError::Failed
        })?;
        if self.json {
            print_json(&report)?;
        } else {
            report.print();
        }
        enforce_run_gates(&report)
    }
}

/// Enforce the skeleton gates: the clean-grade invariant, zero panics, and (a
/// full run's) exact denominator + census pins.
fn enforce_run_gates(report: &SkeletonReport) -> Result<(), CliError> {
    let mut errs: Vec<String> = Vec::new();

    // Invariant gates (always).
    if report.clean_pass != report.expect_clean_graded {
        errs.push(format!(
            "clean pass {} != expect-clean graded {} ({} non-clean)",
            report.clean_pass,
            report.expect_clean_graded,
            report.clean_fail.len()
        ));
    }
    if !report.panics.is_empty() {
        errs.push(format!(
            "{} test(s) panicked, e.g. {}",
            report.panics.len(),
            report.panics.first().map_or("", |p| p.test.as_str())
        ));
    }
    // A stale crash-exclusion (a fixed defect that no longer panics) must be dropped.
    if !report.stale_exclusions.is_empty() {
        errs.push(format!(
            "{} crash-exclusion(s) no longer panic — drop from CRASH_EXCLUSIONS: {}",
            report.stale_exclusions.len(),
            report.stale_exclusions.join(", ")
        ));
    }
    // The hard family gate: never emit a family diagnostic the baseline lacks.
    if report.family_extra != 0 {
        errs.push(format!(
            "family EXTRA {} != 0 (a bind-time over-emission — fix the cascade), e.g. {}",
            report.family_extra,
            report.extra_samples.first().map_or("", String::as_str)
        ));
    }

    // Exact two-sided pins.
    let pin = |errs: &mut Vec<String>, label: &str, got: usize, want: usize| {
        if got != want {
            errs.push(format!("{label} {got} != pinned {want}"));
        }
    };
    pin(&mut errs, "in-scope tests", report.in_scope_tests, RUN_IN_SCOPE_TESTS_PIN);
    pin(&mut errs, "in-scope variants", report.in_scope_variants, RUN_IN_SCOPE_VARIANTS_PIN);
    pin(&mut errs, "expect-clean graded", report.expect_clean_graded, RUN_EXPECT_CLEAN_PIN);
    pin(&mut errs, "clean pass", report.clean_pass, RUN_EXPECT_CLEAN_PIN);
    pin(&mut errs, "baselined parsed", report.baselined_parsed, RUN_BASELINED_PARSED_PIN);
    pin(&mut errs, "parse-rejected", report.parse_rejected_total, RUN_PARSE_REJECTED_PIN);
    pin(
        &mut errs,
        "parse-rejected (no baseline)",
        report.parse_rejected_no_baseline,
        RUN_PARSE_REJECTED_NO_BASELINE_PIN,
    );
    pin(
        &mut errs,
        "parse-rejected (TS1xxx-only)",
        report.parse_rejected_ts1xxx_only,
        RUN_PARSE_REJECTED_TS1XXX_PIN,
    );
    pin(
        &mut errs,
        "parse-rejected (other)",
        report.parse_rejected_other,
        RUN_PARSE_REJECTED_OTHER_PIN,
    );
    pin(&mut errs, "script retries", report.script_retry, RUN_SCRIPT_RETRY_PIN);
    pin(&mut errs, "crash-excluded", report.excluded_crashes, RUN_CRASH_EXCLUDED_PIN);

    // Family grading pins.
    pin(&mut errs, "family graded", report.family_graded_variants, RUN_FAMILY_GRADED_PIN);
    pin(&mut errs, "family positive", report.family_positive_variants, RUN_FAMILY_POSITIVE_PIN);
    pin(&mut errs, "family match", report.family_match, RUN_FAMILY_MATCH_PIN);
    pin(&mut errs, "family missing", report.family_missing, RUN_FAMILY_MISSING_PIN);
    pin(&mut errs, "missing merge", report.missing_merge, RUN_MISSING_MERGE_PIN);
    pin(&mut errs, "missing lib", report.missing_lib, RUN_MISSING_LIB_PIN);
    pin(&mut errs, "missing check-time", report.missing_other, RUN_MISSING_CHECKTIME_PIN);
    pin(
        &mut errs,
        "family span_mismatch",
        report.family_span_mismatch,
        RUN_FAMILY_SPAN_MISMATCH_PIN,
    );
    pin(&mut errs, "carve-out rule (a)", report.carve_out_rule_a, RUN_CARVE_OUT_RULE_A_PIN);
    pin(
        &mut errs,
        "carve-out rule (a) family",
        report.carve_out_rule_a_family,
        RUN_CARVE_OUT_RULE_A_FAMILY_PIN,
    );
    pin(
        &mut errs,
        "moduleDetection variants",
        report.module_detection_variants,
        RUN_MODULE_DETECTION_PIN,
    );

    if errs.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "\nError: {}. If deliberate (a harness-port change, a tsv parser change, or a \
             typescript-go pull), re-pin the RUN_* constants.",
            errs.join("; ")
        );
        Err(CliError::Failed)
    }
}

impl CheckTestCommand {
    fn run(self) -> Result<(), CliError> {
        require_corpus(&self.path)?;
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
fn require_corpus(path: &std::path::Path) -> Result<(), CliError> {
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
        if !corpus_materialized(&self.path) {
            eprintln!(
                "Error: the tsc corpus inputs are not materialized under {}.",
                self.path.display()
            );
            eprintln!("Run `git submodule update --init` in ../typescript-go to materialize them.");
            return Err(CliError::Failed);
        }
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

/// Enforce the `index` gates and denominator pins (all two-sided). Any failure
/// prints the offending checks and exits non-zero.
fn enforce_index_pins(report: &IndexReport) -> Result<(), CliError> {
    let mut errs: Vec<String> = Vec::new();
    let pin = |errs: &mut Vec<String>, label: &str, got: usize, want: usize| {
        if got != want {
            errs.push(format!("{label} {got} != pinned {want}"));
        }
    };

    // Denominators (gate 3).
    pin(&mut errs, "total scanned", report.total_scanned, INDEX_TOTAL_SCANNED_PIN);
    pin(&mut errs, ".ts count", report.ts_count, INDEX_TS_PIN);
    pin(&mut errs, ".tsx count", report.tsx_count, INDEX_TSX_PIN);
    pin(&mut errs, ".js count", report.js_count, INDEX_JS_PIN);
    pin(&mut errs, "skipped tests", report.skipped_tests, INDEX_SKIPPED_TESTS_PIN);
    pin(&mut errs, "single-file", report.single_file, INDEX_SINGLE_FILE_PIN);
    pin(&mut errs, "multi-file", report.multi_file, INDEX_MULTI_FILE_PIN);
    pin(&mut errs, "jsx-scoped", report.jsx_scoped, INDEX_JSX_SCOPED_PIN);
    pin(&mut errs, "js-flavored", report.js_flavored, INDEX_JS_FLAVORED_PIN);
    pin(&mut errs, "pretty tests", report.pretty_tests, INDEX_PRETTY_TESTS_PIN);
    pin(
        &mut errs,
        "basename collisions",
        report.basename_collisions,
        INDEX_BASENAME_COLLISIONS_PIN,
    );
    pin(&mut errs, "cap-exceeded", report.cap_exceeded, INDEX_CAP_EXCEEDED_PIN);
    pin(&mut errs, "unknown includes", report.unknown_includes, INDEX_UNKNOWN_INCLUDES_PIN);
    pin(&mut errs, "variant total", report.variant_total, INDEX_VARIANT_TOTAL_PIN);
    pin(
        &mut errs,
        "skipped variants",
        report.skipped_variants,
        INDEX_SKIPPED_VARIANTS_PIN,
    );
    pin(
        &mut errs,
        "non-skipped variants",
        report.nonskip_variants,
        INDEX_NONSKIP_VARIANTS_PIN,
    );
    pin(&mut errs, "expect-clean", report.expect_clean, INDEX_EXPECT_CLEAN_PIN);

    // Gate 1: baseline join.
    pin(&mut errs, "baselines total", report.baselines_total, INDEX_JOIN_MATCHED_PIN);
    pin(&mut errs, "join matched", report.join_matched, INDEX_JOIN_MATCHED_PIN);
    if !report.join_unmatched.is_empty() {
        errs.push(format!(
            "{} unmatched baseline(s), e.g. {}",
            report.join_unmatched.len(),
            report.join_unmatched.first().map_or("", String::as_str)
        ));
    }
    if !report.join_skipped_with_baseline.is_empty() {
        errs.push(format!(
            "{} baseline(s) map only to skipped variants, e.g. {}",
            report.join_skipped_with_baseline.len(),
            report.join_skipped_with_baseline.first().map_or("", String::as_str)
        ));
    }
    if !report.join_ambiguous.is_empty() {
        errs.push(format!(
            "{} ambiguous baseline(s), e.g. {}",
            report.join_ambiguous.len(),
            report.join_ambiguous.first().map_or("", String::as_str)
        ));
    }

    // Gate 2: unit-text round-trip.
    pin(
        &mut errs,
        "unit round-trip checked",
        report.unit_roundtrip_checked,
        INDEX_UNIT_ROUNDTRIP_PIN,
    );
    pin(
        &mut errs,
        "unit round-trip pretty",
        report.unit_roundtrip_pretty_skipped,
        INDEX_UNIT_ROUNDTRIP_PRETTY_PIN,
    );
    if !report.unit_roundtrip_mismatches.is_empty() {
        errs.push(format!(
            "{} unit round-trip mismatch(es), e.g. {}",
            report.unit_roundtrip_mismatches.len(),
            report
                .unit_roundtrip_mismatches
                .first()
                .map_or(String::new(), |m| m.baseline.clone())
        ));
    }

    // Directive universe.
    if !report.unknown_directives.is_empty() {
        errs.push(format!(
            "{} unknown directive(s): {}",
            report.unknown_directives.len(),
            report.unknown_directives.join(", ")
        ));
    }

    if errs.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "\nError: {}. If deliberate (a harness-port change, or a typescript-go pull), \
             re-pin the INDEX_* constants.",
            errs.join("; ")
        );
        Err(CliError::Failed)
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
