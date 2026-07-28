//! The run's gate/invariant logic: triage-filter parsing, the always-on invariant
//! checks, the zero-valued and snapshot pin comparisons, and the index/query
//! denominator gates. Split out of `tsc_conformance.rs` for navigability.

use crate::cli::CliError;
use crate::tsc_conformance::index::IndexReport;
use crate::tsc_conformance::runner::SkeletonReport;
use crate::tsc_conformance::{CRASH_EXCLUDED_PIN, FAMILIES, FamilyFilter, MissingCause, RunFilter};

use super::pins::{
    BASELINE_COUNT_PIN, INDEX_BASENAME_COLLISIONS_PIN, INDEX_CAP_EXCEEDED_PIN,
    INDEX_EXPECT_CLEAN_PIN, INDEX_JOIN_MATCHED_PIN, INDEX_JS_FLAVORED_PIN, INDEX_JS_PIN,
    INDEX_JSX_SCOPED_PIN, INDEX_MULTI_FILE_PIN, INDEX_NONSKIP_VARIANTS_PIN, INDEX_PRETTY_TESTS_PIN,
    INDEX_SINGLE_FILE_PIN, INDEX_SKIPPED_TESTS_PIN, INDEX_SKIPPED_VARIANTS_PIN,
    INDEX_TOTAL_SCANNED_PIN, INDEX_TS_PIN, INDEX_TSX_PIN, INDEX_UNIT_ROUNDTRIP_PIN,
    INDEX_UNIT_ROUNDTRIP_PRETTY_PIN, INDEX_UNKNOWN_INCLUDES_PIN, INDEX_VARIANT_TOTAL_PIN,
    PIN_SNAPSHOT_FILE, PIN_TABLE, RUN_FAMILY_EXTRA_PIN, RUN_FAMILY_SPAN_MISMATCH_PIN,
    RUN_MISSING_OTHER_PIN, RUN_RELATED_EXTRA_PIN, RUN_RELATED_MISSING_PIN,
    RUN_RELATED_SPAN_MISMATCH_PIN, RunPins,
};

/// Parse a `--family` value into a [`FamilyFilter`] (a `FAMILIES` key or `all`).
pub(super) fn parse_family_filter(arg: &str) -> Result<FamilyFilter, String> {
    let lower = arg.to_lowercase();
    FamilyFilter::parse(&lower).ok_or_else(|| {
        format!(
            "Error: --family expects {}, got {lower:?}",
            FamilyFilter::tokens()
        )
    })
}

/// The triage flags active on this run, by name — `--update` refuses when any is set.
pub(super) fn active_filter_flags(filter: &RunFilter) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if filter.test.is_some() {
        flags.push("--test");
    }
    if filter.code.is_some() {
        flags.push("--code");
    }
    if filter.variant.is_some() {
        flags.push("--variant");
    }
    if filter.family.is_some() {
        flags.push("--family");
    }
    flags
}

/// Refuse `--update` on a narrowed run: the snapshot describes the FULL sweep, so
/// pinning a triage slice's counts would silently unpin everything outside it.
pub(super) fn refuse_narrowed_update(update: bool, filter: &RunFilter) -> Result<(), CliError> {
    let flags = active_filter_flags(filter);
    if !update || flags.is_empty() {
        return Ok(());
    }
    eprintln!(
        "Error: --update pins the FULL sweep. This run is narrowed by {}, so its counts are a \
         slice of what the snapshot means — writing them would silently unpin the rest. Re-run \
         without {}.",
        flags.join(" / "),
        flags.join(" / ")
    );
    Err(CliError::Failed)
}

/// Enforce the skeleton gates: the clean-grade + empty-channel invariants and zero
/// panics (always), plus — on a full run (`enforce_pins`) — the zero-valued invariant
/// consts and the exact snapshot counts. A filtered (triage) run skips both pin
/// blocks.
pub(super) fn enforce_run_gates(
    report: &SkeletonReport,
    pins: &RunPins,
    enforce_pins: bool,
) -> Result<(), CliError> {
    let mut errs = invariant_failures(report);
    if enforce_pins {
        errs.extend(zero_pin_failures(report));
        errs.extend(snapshot_pin_failures(report, pins));
    }

    if errs.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "\nError: {}. If deliberate (a harness-port change, a tsv parser change, or a \
             typescript-go pull), re-pin with `deno task conformance:tsc-check:update` — it \
             rewrites {PIN_SNAPSHOT_FILE} and never touches the zero-valued invariant consts or \
             the crash-exclusion ledger (docs/typechecker.md §Pins & re-pinning).",
            errs.join("; ")
        );
        Err(CliError::Failed)
    }
}

/// Gate an `--update` run on everything EXCEPT the snapshot counts (an update run is
/// unfiltered, so the zero-valued pins apply too). A red run is never pinnable — the
/// whole point of the snapshot is that only *drift* is machine-writable.
pub(super) fn refuse_red_update(report: &SkeletonReport) -> Result<(), CliError> {
    let mut errs = invariant_failures(report);
    errs.extend(zero_pin_failures(report));
    if errs.is_empty() {
        return Ok(());
    }
    eprintln!(
        "\nError: refusing to re-pin a RED run — {}. Fix the failure first; --update writes \
         drift, never a broken state (docs/typechecker.md §Pins & re-pinning).",
        errs.join("; ")
    );
    Err(CliError::Failed)
}

/// The always-on invariant gates: clean grading, no panics, no stale crash exclusion,
/// no family over-emission, no unclassified miss, and the four empty lib channels.
/// These hold on a filtered triage run too.
fn invariant_failures(report: &SkeletonReport) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();

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
    if report.family_extra != RUN_FAMILY_EXTRA_PIN {
        errs.push(format!(
            "family EXTRA {} != 0 (a bind-time over-emission — fix the cascade), e.g. {}",
            report.family_extra,
            report.extra_samples.first().map_or("", String::as_str)
        ));
    }
    // The honest-residual gate: every missing must be explained by merge / lib /
    // late-bound / cfa. An `other` miss is a same-table cascade bug — a HARD zero (an
    // invariant, so a filtered triage run catches it too), not just a full-run pin.
    if report.missing(MissingCause::Other) != RUN_MISSING_OTHER_PIN {
        errs.push(format!(
            "missing OTHER {} != 0 (an unclassified family miss — a same-table cascade bug), e.g. {}",
            report.missing(MissingCause::Other),
            report.missing_other_samples.first().map_or("", String::as_str)
        ));
    }
    // The lib error channels must stay empty (a lib parse-reject, a missing referenced
    // lib, an unrecognized `@lib`/reference name, or a lib binding external with no
    // `declare global` block).
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
    // A lib that binds external with no `declare global` block folds its globals to
    // nothing — the census can't otherwise see the no-op, so gate it to zero.
    if !report.lib_external_no_globals.is_empty() {
        errs.push(format!(
            "{} lib file(s) bound external with no `declare global` block, e.g. {}",
            report.lib_external_no_globals.len(),
            report
                .lib_external_no_globals
                .first()
                .map_or("", String::as_str)
        ));
    }

    errs
}

/// Push `{label} {got} != pinned {want}` when a count misses its pin.
fn pin_failure(errs: &mut Vec<String>, label: &str, got: usize, want: usize) {
    if got != want {
        errs.push(format!("{label} {got} != pinned {want}"));
    }
}

/// The zero-valued pins: semantically-zero contracts that stay Rust consts (never
/// snapshot values) but, unlike the always-on invariants above, are checked on full
/// runs only — a narrowed triage slice can legitimately trip a span comparison it
/// wasn't meant to grade.
fn zero_pin_failures(report: &SkeletonReport) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();
    pin_failure(
        &mut errs,
        "family span_mismatch",
        report.family_span_mismatch,
        RUN_FAMILY_SPAN_MISMATCH_PIN,
    );
    pin_failure(
        &mut errs,
        "related missing",
        report.related_missing,
        RUN_RELATED_MISSING_PIN,
    );
    pin_failure(
        &mut errs,
        "related extra",
        report.related_extra,
        RUN_RELATED_EXTRA_PIN,
    );
    pin_failure(
        &mut errs,
        "related span_mismatch",
        report.related_span_mismatch,
        RUN_RELATED_SPAN_MISMATCH_PIN,
    );
    // The crash-exclusion count moves with the `CRASH_EXCLUSIONS` ledger, one
    // deliberate edit — so it is graded here rather than from the snapshot.
    pin_failure(
        &mut errs,
        "crash-excluded",
        report.excluded_crashes,
        CRASH_EXCLUDED_PIN,
    );
    errs
}

/// The committed snapshot counts (`tsc_conformance_pins.txt`), all exact and
/// two-sided. These are the ones `--update` rewrites — they shift by construction when
/// tsv's parser advances or the harness port changes. Driven entirely by
/// [`PIN_TABLE`], so a row cannot be pinned-but-ungraded; the message names the
/// snapshot key so the offending line in the file is one search away.
fn snapshot_pin_failures(report: &SkeletonReport, pins: &RunPins) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();
    // Structural guard, computed rather than declared: every graded family must own its
    // `{key}_match` / `{key}_missing` rows, so a family added without pins fails the run
    // instead of grading silently unpinned.
    for family in &FAMILIES {
        for suffix in ["match", "missing"] {
            let key = format!("{}_{suffix}", family.key);
            if !PIN_TABLE.iter().any(|r| r.key == key) {
                errs.push(format!("graded family `{}` has no `{key}` pin", family.key));
            }
        }
    }
    for row in &PIN_TABLE {
        let (got, want) = ((row.measured)(report), pins.get(row.field));
        if got != want {
            errs.push(format!(
                "{} {got} != pinned {want} [{}]",
                row.label, row.key
            ));
        }
    }
    // The one comparison that is not a table row: `clean_pass` is a SECOND measured
    // value graded against the `expect_clean` pin (the invariant block already asserts
    // the two agree, so this only fires alongside a denominator move).
    if report.clean_pass != pins.expect_clean {
        errs.push(format!(
            "clean pass {} != pinned {} [expect_clean]",
            report.clean_pass, pins.expect_clean
        ));
    }
    errs
}

/// Enforce the `index` gates and denominator pins (all two-sided). Any failure
/// prints the offending checks and exits non-zero.
pub(super) fn enforce_index_pins(report: &IndexReport) -> Result<(), CliError> {
    let mut errs: Vec<String> = Vec::new();

    // Denominators (gate 3).
    pin_failure(
        &mut errs,
        "total scanned",
        report.total_scanned,
        INDEX_TOTAL_SCANNED_PIN,
    );
    pin_failure(&mut errs, ".ts count", report.ts_count, INDEX_TS_PIN);
    pin_failure(&mut errs, ".tsx count", report.tsx_count, INDEX_TSX_PIN);
    pin_failure(&mut errs, ".js count", report.js_count, INDEX_JS_PIN);
    pin_failure(
        &mut errs,
        "skipped tests",
        report.skipped_tests,
        INDEX_SKIPPED_TESTS_PIN,
    );
    pin_failure(
        &mut errs,
        "single-file",
        report.single_file,
        INDEX_SINGLE_FILE_PIN,
    );
    pin_failure(
        &mut errs,
        "multi-file",
        report.multi_file,
        INDEX_MULTI_FILE_PIN,
    );
    pin_failure(
        &mut errs,
        "jsx-scoped",
        report.jsx_scoped,
        INDEX_JSX_SCOPED_PIN,
    );
    pin_failure(
        &mut errs,
        "js-flavored",
        report.js_flavored,
        INDEX_JS_FLAVORED_PIN,
    );
    pin_failure(
        &mut errs,
        "pretty tests",
        report.pretty_tests,
        INDEX_PRETTY_TESTS_PIN,
    );
    pin_failure(
        &mut errs,
        "basename collisions",
        report.basename_collisions,
        INDEX_BASENAME_COLLISIONS_PIN,
    );
    pin_failure(
        &mut errs,
        "cap-exceeded",
        report.cap_exceeded,
        INDEX_CAP_EXCEEDED_PIN,
    );
    pin_failure(
        &mut errs,
        "unknown includes",
        report.unknown_includes,
        INDEX_UNKNOWN_INCLUDES_PIN,
    );
    pin_failure(
        &mut errs,
        "variant total",
        report.variant_total,
        INDEX_VARIANT_TOTAL_PIN,
    );
    pin_failure(
        &mut errs,
        "skipped variants",
        report.skipped_variants,
        INDEX_SKIPPED_VARIANTS_PIN,
    );
    pin_failure(
        &mut errs,
        "non-skipped variants",
        report.nonskip_variants,
        INDEX_NONSKIP_VARIANTS_PIN,
    );
    pin_failure(
        &mut errs,
        "expect-clean",
        report.expect_clean,
        INDEX_EXPECT_CLEAN_PIN,
    );

    // Gate 1: baseline join.
    pin_failure(
        &mut errs,
        "baselines total",
        report.baselines_total,
        INDEX_JOIN_MATCHED_PIN,
    );
    pin_failure(
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
    pin_failure(
        &mut errs,
        "unit round-trip checked",
        report.unit_roundtrip_checked,
        INDEX_UNIT_ROUNDTRIP_PIN,
    );
    pin_failure(
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

/// Enforce the baseline-count regression pin (unfiltered `histogram` /
/// `denominators` runs), mirroring `test262`'s hard-fail on a pin mismatch.
pub(super) fn enforce_pin(count: usize) -> Result<(), CliError> {
    if count != BASELINE_COUNT_PIN {
        eprintln!(
            "Error: pinned count mismatch — discovered {count} .errors.txt baselines ≠ pinned {BASELINE_COUNT_PIN}. \
             If deliberate (a typescript-go pull, a discovery change), re-pin BASELINE_COUNT_PIN."
        );
        return Err(CliError::Failed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_refuses_a_red_run() {
        // The slice's core safety claim: only DRIFT is machine-writable. A green report
        // pins; a broken one never does — including a zero-pin a filtered normal run
        // would skip.
        let green = SkeletonReport::default();
        assert!(refuse_red_update(&green).is_ok());

        let over_emitting = SkeletonReport {
            family_extra: 1,
            ..SkeletonReport::default()
        };
        assert!(refuse_red_update(&over_emitting).is_err());

        let span_disagreement = SkeletonReport {
            family_span_mismatch: 1,
            ..SkeletonReport::default()
        };
        assert!(refuse_red_update(&span_disagreement).is_err());

        // ... and an unclassified miss, the invariant that gates even a filtered run.
        let unclassified = SkeletonReport {
            family_missing: 1,
            missing_by_cause: std::collections::BTreeMap::from([(MissingCause::Other, 1usize)]),
            ..SkeletonReport::default()
        };
        assert!(refuse_red_update(&unclassified).is_err());
    }

    #[test]
    fn update_refuses_a_narrowed_run() {
        // `--update` pins the FULL sweep, so any triage filter refuses; without
        // `--update` the same filters are fine (they just skip the pins).
        let full = RunFilter::default();
        assert!(active_filter_flags(&full).is_empty());
        assert!(refuse_narrowed_update(true, &full).is_ok());

        let narrowed = RunFilter {
            code: Some(2300),
            family: Some(FamilyFilter::All),
            ..RunFilter::default()
        };
        assert_eq!(active_filter_flags(&narrowed), vec!["--code", "--family"]);
        assert!(refuse_narrowed_update(true, &narrowed).is_err());
        assert!(refuse_narrowed_update(false, &narrowed).is_ok());
    }
}
