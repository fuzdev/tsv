//! tsc_conformance command — ad-hoc queries over the TypeScript-Go conformance
//! baselines (`*.errors.txt`). Pure Rust, no typechecker: tool #1 of the
//! typechecker conformance harness (the "ask important questions" tool). Reads
//! only the committed tsgo baselines — the corpus *inputs* live in a git
//! submodule that is often unmaterialized.

use crate::cli::CliError;
use crate::tsc_conformance::index::IndexReport;
use crate::tsc_conformance::runner::SkeletonReport;
use crate::tsc_conformance::{
    RunFilter, RunOptions, baselines_dir, check_one, corpus_materialized, denominators,
    discover_baselines, histogram, run_index, run_roundtrip, run_skeleton, tests_by_code,
};
use argh::FromArgs;
use std::path::{Path, PathBuf};

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

/// REGRESSION PINS (exact, two-sided) for the family grading (the bind + merge
/// gate). Measured 2026-07-10 vs pin 168e7015. `family_extra` is gated to 0
/// (hard); the rest pin the buckets so any move (a cascade change, a merge
/// change, a tsv parser change, a typescript-go pull) forces a deliberate re-pin.
/// The missing bucket is classified: `merge` (merge-phase family — **0**, S4
/// closed the single-file merge path: TS2397 globalThis/undefined + TS2664
/// augmentation-not-found), `lib` (absent-lib conflicts — **now 0**, S5 closed the
/// four classified lib-conflict misses: TS2300 eval/Symbol/Promise/ElementTagNameMap
/// against the standard-library globals), and `check-time` (checker-emitted
/// TS2300/2451 the bind+merge slice can't produce — duplicate members, type
/// parameters, computed/private names). A drop in `check-time` (matches gained) is a
/// real improvement that re-pins; a rise anywhere is a regression to explain.
const RUN_FAMILY_GRADED_PIN: usize = 4066;
const RUN_FAMILY_POSITIVE_PIN: usize = 125;
const RUN_FAMILY_MATCH_PIN: usize = 425;
const RUN_FAMILY_MISSING_PIN: usize = 125;
const RUN_MISSING_MERGE_PIN: usize = 0;
const RUN_MISSING_LIB_PIN: usize = 0;
const RUN_MISSING_CHECKTIME_PIN: usize = 125;
const RUN_FAMILY_SPAN_MISMATCH_PIN: usize = 0;
const RUN_CARVE_OUT_RULE_A_PIN: usize = 380;
const RUN_CARVE_OUT_RULE_A_FAMILY_PIN: usize = 9;
const RUN_MODULE_DETECTION_PIN: usize = 1;

/// REGRESSION PINS (exact, two-sided) for the lib base (S5). Measured 2026-07-10 vs
/// pin 168e7015 (`_submodules/TypeScript` corpus materialized): the distinct lib
/// `.d.ts` files parsed+bound and the distinct resolved lib sets folded across the
/// in-scope variants. A move is a deliberate re-pin (a harness-port change, a lib
/// set change, or a typescript-go pull). The three error channels are gated to
/// empty (a lib parse-reject, a missing referenced lib, or an unrecognized
/// `@lib`/reference name — all expected never on the pinned checkout).
const RUN_LIB_FILES_BOUND_PIN: usize = 107;
const RUN_LIB_SETS_PIN: usize = 50;

/// REGRESSION PINS (exact, two-sided) for the related-info channel — graded on the
/// matched family primaries only (the primary code gates the per-variant verdict;
/// related info is its own pinned channel). Measured 2026-07-10 vs pin 168e7015:
/// the 42 multiple-default-export chains (TS2752/2753/2528's TS6204) plus the 9
/// lib-conflict related infos S5 added (TS6203/6204 pointing at the masked lib
/// files: eval 1, Symbol 3, Promise 4, ElementTagNameMap 1). `missing`/`extra`/
/// `span_mismatch` are 0 (the lib relateds match the baseline's masked
/// `lib.x.d.ts:--:--` entries by (code, file), loc-agnostic). A rise in
/// `missing`/`extra` is a regression to explain.
const RUN_RELATED_MATCH_PIN: usize = 51;
const RUN_RELATED_MISSING_PIN: usize = 0;
const RUN_RELATED_EXTRA_PIN: usize = 0;
const RUN_RELATED_SPAN_MISMATCH_PIN: usize = 0;

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

    /// write a JSON manifest of every graded variant (per-variant verdict + buckets
    /// + census + pins) to this path — the tsc analog of `test262 --emit-manifest`
    #[argh(option)]
    emit_manifest: Option<PathBuf>,

    /// write the committed compact report to `<path>.json` + `<path>.md` (full runs
    /// only; deterministic, wall-clock excluded)
    #[argh(option)]
    report: Option<PathBuf>,
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

        let filter = self.build_filter()?;
        let filtered = filter.is_active();
        // The committed report is the full-run artifact; refuse to write a partial one.
        if self.report.is_some() && filtered {
            eprintln!(
                "Error: --report writes the committed full report; it cannot be combined with \
                 --test/--code/--variant filters."
            );
            return Err(CliError::Failed);
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
        // gates still hold. Committed artifacts land only when the gates pass (so a
        // pin miss never writes a bad manifest/report), while a failure dumps per-test
        // diff artifacts for triage.
        match enforce_run_gates(&report, !filtered) {
            Ok(()) => {
                if let Some(path) = &self.emit_manifest {
                    write_manifest(&report, path)?;
                }
                if let Some(path) = &self.report {
                    write_report(&report, path)?;
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
        Ok(RunFilter {
            test: self.test.clone(),
            code: self.code,
            variant,
        })
    }
}

/// Enforce the skeleton gates: the clean-grade + empty-channel invariants and zero
/// panics (always), plus — on a full run (`enforce_pins`) — the exact denominator,
/// family, related, and census pins. A filtered (triage) run skips the pins.
fn enforce_run_gates(report: &SkeletonReport, enforce_pins: bool) -> Result<(), CliError> {
    let mut errs: Vec<String> = Vec::new();

    // --- Invariant gates (always, even on a filtered run) ---
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
    // The lib error channels must stay empty (a lib parse-reject, a missing referenced
    // lib, or an unrecognized `@lib`/reference name).
    if !report.lib_parse_errors.is_empty() {
        errs.push(format!(
            "{} lib file(s) failed to parse, e.g. {}",
            report.lib_parse_errors.len(),
            report.lib_parse_errors.first().map_or("", String::as_str)
        ));
    }
    if !report.lib_missing_files.is_empty() {
        errs.push(format!(
            "{} referenced lib file(s) missing, e.g. {}",
            report.lib_missing_files.len(),
            report.lib_missing_files.first().map_or("", String::as_str)
        ));
    }
    if !report.lib_unknown_names.is_empty() {
        errs.push(format!(
            "{} unrecognized @lib/reference name(s), e.g. {}",
            report.lib_unknown_names.len(),
            report.lib_unknown_names.first().map_or("", String::as_str)
        ));
    }

    // --- Exact two-sided pins (full run only; filters skip them) ---
    if enforce_pins {
        let pin = |errs: &mut Vec<String>, label: &str, got: usize, want: usize| {
            if got != want {
                errs.push(format!("{label} {got} != pinned {want}"));
            }
        };
        pin(
            &mut errs,
            "in-scope tests",
            report.in_scope_tests,
            RUN_IN_SCOPE_TESTS_PIN,
        );
        pin(
            &mut errs,
            "in-scope variants",
            report.in_scope_variants,
            RUN_IN_SCOPE_VARIANTS_PIN,
        );
        pin(
            &mut errs,
            "expect-clean graded",
            report.expect_clean_graded,
            RUN_EXPECT_CLEAN_PIN,
        );
        pin(
            &mut errs,
            "clean pass",
            report.clean_pass,
            RUN_EXPECT_CLEAN_PIN,
        );
        pin(
            &mut errs,
            "baselined parsed",
            report.baselined_parsed,
            RUN_BASELINED_PARSED_PIN,
        );
        pin(
            &mut errs,
            "parse-rejected",
            report.parse_rejected_total,
            RUN_PARSE_REJECTED_PIN,
        );
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
        pin(
            &mut errs,
            "script retries",
            report.script_retry,
            RUN_SCRIPT_RETRY_PIN,
        );
        pin(
            &mut errs,
            "crash-excluded",
            report.excluded_crashes,
            RUN_CRASH_EXCLUDED_PIN,
        );

        // Lib-base (S5) sizing pins.
        pin(
            &mut errs,
            "lib files bound",
            report.lib_files_bound,
            RUN_LIB_FILES_BOUND_PIN,
        );
        pin(
            &mut errs,
            "lib sets folded",
            report.lib_sets_built,
            RUN_LIB_SETS_PIN,
        );

        // Family grading pins.
        pin(
            &mut errs,
            "family graded",
            report.family_graded_variants,
            RUN_FAMILY_GRADED_PIN,
        );
        pin(
            &mut errs,
            "family positive",
            report.family_positive_variants,
            RUN_FAMILY_POSITIVE_PIN,
        );
        pin(
            &mut errs,
            "family match",
            report.family_match,
            RUN_FAMILY_MATCH_PIN,
        );
        pin(
            &mut errs,
            "family missing",
            report.family_missing,
            RUN_FAMILY_MISSING_PIN,
        );
        pin(
            &mut errs,
            "missing merge",
            report.missing_merge,
            RUN_MISSING_MERGE_PIN,
        );
        pin(
            &mut errs,
            "missing lib",
            report.missing_lib,
            RUN_MISSING_LIB_PIN,
        );
        pin(
            &mut errs,
            "missing check-time",
            report.missing_other,
            RUN_MISSING_CHECKTIME_PIN,
        );
        pin(
            &mut errs,
            "family span_mismatch",
            report.family_span_mismatch,
            RUN_FAMILY_SPAN_MISMATCH_PIN,
        );

        // Related-info channel pins (two-sided; does not gate the primary verdict).
        pin(
            &mut errs,
            "related match",
            report.related_match,
            RUN_RELATED_MATCH_PIN,
        );
        pin(
            &mut errs,
            "related missing",
            report.related_missing,
            RUN_RELATED_MISSING_PIN,
        );
        pin(
            &mut errs,
            "related extra",
            report.related_extra,
            RUN_RELATED_EXTRA_PIN,
        );
        pin(
            &mut errs,
            "related span_mismatch",
            report.related_span_mismatch,
            RUN_RELATED_SPAN_MISMATCH_PIN,
        );

        pin(
            &mut errs,
            "carve-out rule (a)",
            report.carve_out_rule_a,
            RUN_CARVE_OUT_RULE_A_PIN,
        );
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
    }

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

/// The exact `RUN_*` pins this run is held to — recorded in the committed report and
/// the manifest so the artifact states what it was measured against.
#[derive(serde::Serialize)]
struct RunPins {
    in_scope_tests: usize,
    in_scope_variants: usize,
    expect_clean: usize,
    baselined_parsed: usize,
    parse_rejected: usize,
    family_graded: usize,
    family_positive: usize,
    family_match: usize,
    family_missing: usize,
    missing_merge: usize,
    missing_lib: usize,
    missing_check_time: usize,
    family_extra: usize,
    family_span_mismatch: usize,
    related_match: usize,
    related_missing: usize,
    related_extra: usize,
    related_span_mismatch: usize,
    carve_out_rule_a: usize,
    carve_out_rule_a_family: usize,
    module_detection: usize,
    script_retry: usize,
    crash_excluded: usize,
    lib_files_bound: usize,
    lib_sets: usize,
}

/// Snapshot the `RUN_*` pin constants (the hard family gate `family_extra` is a fixed
/// zero, folded in for a complete record).
fn run_pins() -> RunPins {
    RunPins {
        in_scope_tests: RUN_IN_SCOPE_TESTS_PIN,
        in_scope_variants: RUN_IN_SCOPE_VARIANTS_PIN,
        expect_clean: RUN_EXPECT_CLEAN_PIN,
        baselined_parsed: RUN_BASELINED_PARSED_PIN,
        parse_rejected: RUN_PARSE_REJECTED_PIN,
        family_graded: RUN_FAMILY_GRADED_PIN,
        family_positive: RUN_FAMILY_POSITIVE_PIN,
        family_match: RUN_FAMILY_MATCH_PIN,
        family_missing: RUN_FAMILY_MISSING_PIN,
        missing_merge: RUN_MISSING_MERGE_PIN,
        missing_lib: RUN_MISSING_LIB_PIN,
        missing_check_time: RUN_MISSING_CHECKTIME_PIN,
        family_extra: 0,
        family_span_mismatch: RUN_FAMILY_SPAN_MISMATCH_PIN,
        related_match: RUN_RELATED_MATCH_PIN,
        related_missing: RUN_RELATED_MISSING_PIN,
        related_extra: RUN_RELATED_EXTRA_PIN,
        related_span_mismatch: RUN_RELATED_SPAN_MISMATCH_PIN,
        carve_out_rule_a: RUN_CARVE_OUT_RULE_A_PIN,
        carve_out_rule_a_family: RUN_CARVE_OUT_RULE_A_FAMILY_PIN,
        module_detection: RUN_MODULE_DETECTION_PIN,
        script_retry: RUN_SCRIPT_RETRY_PIN,
        crash_excluded: RUN_CRASH_EXCLUDED_PIN,
        lib_files_bound: RUN_LIB_FILES_BOUND_PIN,
        lib_sets: RUN_LIB_SETS_PIN,
    }
}

/// The `--emit-manifest` wrapper: the full per-variant report plus the pins snapshot.
#[derive(serde::Serialize)]
struct RunManifest<'a> {
    pins: RunPins,
    report: &'a SkeletonReport,
}

/// Write the `--emit-manifest` JSON (per-variant verdicts + buckets + census + pins).
/// Called only after the gates pass, so a bad manifest never lands.
fn write_manifest(report: &SkeletonReport, path: &Path) -> Result<(), CliError> {
    let manifest = RunManifest {
        pins: run_pins(),
        report,
    };
    let file = std::fs::File::create(path).map_err(|e| {
        eprintln!("Error creating manifest {}: {e}", path.display());
        CliError::Failed
    })?;
    serde_json::to_writer(std::io::BufWriter::new(file), &manifest).map_err(|e| {
        eprintln!("Error writing manifest: {e}");
        CliError::Failed
    })?;
    println!(
        "Wrote manifest ({} variant rows) to {}",
        report.manifest_entries.len(),
        path.display()
    );
    Ok(())
}

/// Build the committed compact report as a JSON value — deterministic (sorted
/// per-code maps, wall-clock excluded) so re-runs are diff-clean.
fn build_report_value(report: &SkeletonReport) -> serde_json::Value {
    serde_json::json!({
        "oracle": "tsgo committed .errors.txt baselines (bind + merge family)",
        "denominators": {
            "in_scope_tests": report.in_scope_tests,
            "in_scope_variants": report.in_scope_variants,
            "expect_clean_graded": report.expect_clean_graded,
            "clean_pass": report.clean_pass,
            "baselined_parsed": report.baselined_parsed,
            "family_graded_variants": report.family_graded_variants,
            "family_positive_variants": report.family_positive_variants,
        },
        "family": {
            "match": report.family_match,
            "missing": {
                "total": report.family_missing,
                "merge_path": report.missing_merge,
                "lib_conflict": report.missing_lib,
                "check_time": report.missing_other,
            },
            "extra": report.family_extra,
            "span_mismatch": report.family_span_mismatch,
        },
        "per_code": {
            "match": report.family_match_by_code,
            "missing": report.family_missing_by_code,
        },
        "related": {
            "match": report.related_match,
            "missing": report.related_missing,
            "extra": report.related_extra,
            "span_mismatch": report.related_span_mismatch,
        },
        "carve_outs": {
            "recovery_ast_rule_a": report.carve_out_rule_a,
            "recovery_ast_rule_a_family": report.carve_out_rule_a_family,
            "module_detection_variants": report.module_detection_variants,
        },
        "census": {
            "parse_rejected_total": report.parse_rejected_total,
            "parse_rejected_no_baseline": report.parse_rejected_no_baseline,
            "parse_rejected_ts1xxx_only": report.parse_rejected_ts1xxx_only,
            "parse_rejected_other": report.parse_rejected_other,
            "script_retry": report.script_retry,
            "crash_excluded": report.excluded_crashes,
        },
        "lib": {
            "files_bound": report.lib_files_bound,
            "sets_folded": report.lib_sets_built,
        },
        "pins": run_pins(),
    })
}

/// Render the committed report's compact Markdown (the same deterministic data as
/// [`build_report_value`], for readers).
fn render_report_md(report: &SkeletonReport) -> String {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;
    let mut s = String::new();
    s.push_str("# tsc_conformance run — committed report\n\n");
    s.push_str(
        "Oracle: tsgo committed `.errors.txt` baselines (bind + merge family). \
         Deterministic — wall-clock excluded.\n\n",
    );

    s.push_str("## Denominators\n\n");
    let _ = writeln!(s, "- in-scope tests: {}", report.in_scope_tests);
    let _ = writeln!(s, "- in-scope variants: {}", report.in_scope_variants);
    let _ = writeln!(
        s,
        "- expect-clean graded / clean pass: {} / {}",
        report.expect_clean_graded, report.clean_pass
    );
    let _ = writeln!(s, "- baselined + parsed: {}", report.baselined_parsed);
    let _ = writeln!(
        s,
        "- family graded / family-positive: {} / {}\n",
        report.family_graded_variants, report.family_positive_variants
    );

    s.push_str("## Family (2300 / 2451 / 2567 / 2528 + merge 2397 / 2649 / 2664 / 2671)\n\n");
    let _ = writeln!(s, "- match: {}", report.family_match);
    let _ = writeln!(
        s,
        "- missing: {} (merge-path {}, lib-conflict {}, check-time {})",
        report.family_missing, report.missing_merge, report.missing_lib, report.missing_other
    );
    let _ = writeln!(s, "- extra (GATE=0): {}", report.family_extra);
    let _ = writeln!(s, "- span mismatch: {}\n", report.family_span_mismatch);

    s.push_str("## Per-code table\n\n");
    s.push_str("| code | match | missing |\n| --- | --- | --- |\n");
    let codes: BTreeSet<u32> = report
        .family_match_by_code
        .keys()
        .chain(report.family_missing_by_code.keys())
        .copied()
        .collect();
    for code in codes {
        let m = report.family_match_by_code.get(&code).copied().unwrap_or(0);
        let miss = report
            .family_missing_by_code
            .get(&code)
            .copied()
            .unwrap_or(0);
        let _ = writeln!(s, "| TS{code} | {m} | {miss} |");
    }
    s.push('\n');

    s.push_str("## Related-info channel (matched primaries)\n\n");
    let _ = writeln!(
        s,
        "- match / missing / extra / span-mismatch: {} / {} / {} / {}\n",
        report.related_match,
        report.related_missing,
        report.related_extra,
        report.related_span_mismatch
    );

    s.push_str("## Carve-outs\n\n");
    let _ = writeln!(
        s,
        "- recovery-AST rule (a): {} (family-positive {})",
        report.carve_out_rule_a, report.carve_out_rule_a_family
    );
    let _ = writeln!(
        s,
        "- moduleDetection variants (inert for family): {}\n",
        report.module_detection_variants
    );

    s.push_str("## Parse-divergence census\n\n");
    let _ = writeln!(
        s,
        "- parse-rejected: {} (no baseline {}, TS1xxx-only {}, other {})",
        report.parse_rejected_total,
        report.parse_rejected_no_baseline,
        report.parse_rejected_ts1xxx_only,
        report.parse_rejected_other
    );
    let _ = writeln!(s, "- script-goal retries: {}", report.script_retry);
    let _ = writeln!(
        s,
        "- crash-excluded (tracked): {}\n",
        report.excluded_crashes
    );

    s.push_str("## Lib base\n\n");
    let _ = writeln!(
        s,
        "- lib files bound / sets folded: {} / {}",
        report.lib_files_bound, report.lib_sets_built
    );

    s
}

/// Write the committed compact report to `<base>.json` + `<base>.md` (full runs only;
/// deterministic). Called only after the gates pass.
fn write_report(report: &SkeletonReport, base: &Path) -> Result<(), CliError> {
    let json_path = PathBuf::from(format!("{}.json", base.display()));
    let md_path = PathBuf::from(format!("{}.md", base.display()));
    let value = build_report_value(report);
    let mut json = serde_json::to_string_pretty(&value).map_err(|e| {
        eprintln!("Error serializing report JSON: {e}");
        CliError::Failed
    })?;
    json.push('\n');
    std::fs::write(&json_path, json).map_err(|e| {
        eprintln!("Error writing {}: {e}", json_path.display());
        CliError::Failed
    })?;
    std::fs::write(&md_path, render_report_md(report)).map_err(|e| {
        eprintln!("Error writing {}: {e}", md_path.display());
        CliError::Failed
    })?;
    println!(
        "Wrote committed report to {} + {}",
        json_path.display(),
        md_path.display()
    );
    Ok(())
}

/// Dump each failing variant's ours-vs-baseline diff under
/// `target/tsc_conformance/diffs/` (a regression aid; a no-op when the run is green).
fn write_diff_artifacts(report: &SkeletonReport) {
    if report.failing_variants.is_empty() {
        return;
    }
    let dir = Path::new("target/tsc_conformance/diffs");
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("  (could not create {}: {e})", dir.display());
        return;
    }
    eprintln!(
        "\nWrote {} failure diff artifact(s) under {}/:",
        report.failing_variants.len(),
        dir.display()
    );
    for fv in &report.failing_variants {
        let path = dir.join(format!(
            "{}__{}.diff",
            fv.suite,
            sanitize_artifact_name(&fv.config)
        ));
        match std::fs::write(&path, &fv.diff) {
            Ok(()) => eprintln!("  {} ({})", path.display(), fv.reason),
            Err(e) => eprintln!("  (failed to write {}: {e})", path.display()),
        }
    }
}

/// Replace path-hostile characters so a baseline identity is a safe artifact basename.
fn sanitize_artifact_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_whitespace() {
                '_'
            } else {
                c
            }
        })
        .collect()
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
    pin(
        &mut errs,
        "total scanned",
        report.total_scanned,
        INDEX_TOTAL_SCANNED_PIN,
    );
    pin(&mut errs, ".ts count", report.ts_count, INDEX_TS_PIN);
    pin(&mut errs, ".tsx count", report.tsx_count, INDEX_TSX_PIN);
    pin(&mut errs, ".js count", report.js_count, INDEX_JS_PIN);
    pin(
        &mut errs,
        "skipped tests",
        report.skipped_tests,
        INDEX_SKIPPED_TESTS_PIN,
    );
    pin(
        &mut errs,
        "single-file",
        report.single_file,
        INDEX_SINGLE_FILE_PIN,
    );
    pin(
        &mut errs,
        "multi-file",
        report.multi_file,
        INDEX_MULTI_FILE_PIN,
    );
    pin(
        &mut errs,
        "jsx-scoped",
        report.jsx_scoped,
        INDEX_JSX_SCOPED_PIN,
    );
    pin(
        &mut errs,
        "js-flavored",
        report.js_flavored,
        INDEX_JS_FLAVORED_PIN,
    );
    pin(
        &mut errs,
        "pretty tests",
        report.pretty_tests,
        INDEX_PRETTY_TESTS_PIN,
    );
    pin(
        &mut errs,
        "basename collisions",
        report.basename_collisions,
        INDEX_BASENAME_COLLISIONS_PIN,
    );
    pin(
        &mut errs,
        "cap-exceeded",
        report.cap_exceeded,
        INDEX_CAP_EXCEEDED_PIN,
    );
    pin(
        &mut errs,
        "unknown includes",
        report.unknown_includes,
        INDEX_UNKNOWN_INCLUDES_PIN,
    );
    pin(
        &mut errs,
        "variant total",
        report.variant_total,
        INDEX_VARIANT_TOTAL_PIN,
    );
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
    pin(
        &mut errs,
        "expect-clean",
        report.expect_clean,
        INDEX_EXPECT_CLEAN_PIN,
    );

    // Gate 1: baseline join.
    pin(
        &mut errs,
        "baselines total",
        report.baselines_total,
        INDEX_JOIN_MATCHED_PIN,
    );
    pin(
        &mut errs,
        "join matched",
        report.join_matched,
        INDEX_JOIN_MATCHED_PIN,
    );
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
            report
                .join_skipped_with_baseline
                .first()
                .map_or("", String::as_str)
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
