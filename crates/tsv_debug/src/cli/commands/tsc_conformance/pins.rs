//! The pin snapshot machinery: the regression-pin consts (oracle-side baseline/index
//! counts, the always-zero invariants), the [`PIN_TABLE`] row schema, [`RunPins`], and
//! the snapshot parse/render/measure/update functions. Split out of `tsc_conformance.rs`
//! for navigability.

use crate::cli::CliError;
use crate::tsc_conformance::runner::SkeletonReport;
use crate::tsc_conformance::{CRASH_EXCLUDED_PIN, MissingCause};
use std::path::{Path, PathBuf};

use super::load_baselines;

/// REGRESSION PIN (exact): total tsgo .errors.txt baselines. Measured
/// 2026-07-09, ../typescript-go at 168e7015 (_submodules/TypeScript corpus pin
/// 4d4f005c, may be unmaterialized). The checkout is updated deliberately, so any
/// move (a discovery bug, or a typescript-go pull) must be re-pinned here.
pub(super) const BASELINE_COUNT_PIN: usize = 7033;

/// REGRESSION PIN (exact): baselines that round-trip byte-identically
/// (`parse → render == input`). Measured vs pin 168e7015: 7033 — the **full**
/// baseline set (100%, plain + pretty paths together, i.e. `BASELINE_COUNT_PIN`).
/// A move in either direction is a deliberate re-pin (a parser/renderer change,
/// or a typescript-go pull); pin two-sided so drift can't hide.
pub(super) const ROUNDTRIP_PASS_PIN: usize = 7033;

/// REGRESSION PIN (exact): baselines that take the ANSI-colored `pretty=true`
/// path (its own model, parser, and colored renderer). In scope and folded into
/// the pass count; pinned so the pretty set can't grow or shrink silently on a
/// typescript-go pull.
pub(super) const PRETTY_PATH_PIN: usize = 14;

/// REGRESSION PINS (exact, two-sided) for the `index` corpus-input self-checks.
/// Measured 2026-07-10, ../typescript-go at 168e7015 (`_submodules/TypeScript`
/// corpus materialized). Every move is a deliberate re-pin (a harness-port change,
/// or a typescript-go pull). The corpus files:
pub(super) const INDEX_TOTAL_SCANNED_PIN: usize = 12445;
pub(super) const INDEX_TS_PIN: usize = 12114;
pub(super) const INDEX_TSX_PIN: usize = 330;
pub(super) const INDEX_JS_PIN: usize = 1;
/// Static test-level skips (`skippedTests`) and per-directory sizing.
pub(super) const INDEX_SKIPPED_TESTS_PIN: usize = 45;
pub(super) const INDEX_SINGLE_FILE_PIN: usize = 10388;
pub(super) const INDEX_MULTI_FILE_PIN: usize = 2012;
/// Selection-predicate denominators.
pub(super) const INDEX_JSX_SCOPED_PIN: usize = 379;
pub(super) const INDEX_JS_FLAVORED_PIN: usize = 934;
pub(super) const INDEX_PRETTY_TESTS_PIN: usize = 14;
pub(super) const INDEX_BASENAME_COLLISIONS_PIN: usize = 0;
pub(super) const INDEX_CAP_EXCEEDED_PIN: usize = 0;
/// varyBy include values with no normalized identity (tsgo hard-fails on each; the
/// harness keeps them as graceful `Other` variants). Zero on the pinned corpus — a
/// nonzero count is a phantom-variant signal from a corpus pull, not a clean move.
pub(super) const INDEX_UNKNOWN_INCLUDES_PIN: usize = 0;
/// Variant sizing: total variants, the variant-level (unsupported-option) skips,
/// the non-skipped variants, and the expect-clean count.
pub(super) const INDEX_VARIANT_TOTAL_PIN: usize = 14916;
pub(super) const INDEX_SKIPPED_VARIANTS_PIN: usize = 2068;
pub(super) const INDEX_NONSKIP_VARIANTS_PIN: usize = 12848;
pub(super) const INDEX_EXPECT_CLEAN_PIN: usize = 5815;
/// Gate 1 (baseline join): every on-disk baseline matches one non-skipped variant.
pub(super) const INDEX_JOIN_MATCHED_PIN: usize = 7033;
/// Gate 2 (unit-text round-trip): non-pretty baselined tests whose units reproduce
/// their section bodies, and the pretty baselines carved out.
pub(super) const INDEX_UNIT_ROUNDTRIP_PIN: usize = 7019;
pub(super) const INDEX_UNIT_ROUNDTRIP_PRETTY_PIN: usize = 14;

/// INVARIANT GATES (semantically zero, never snapshot values). Each of these is a
/// *contract* rather than a measurement — an unclassified family miss, a family
/// diagnostic the baseline lacks, a span that disagrees — so it stays a Rust const
/// that `run --update` cannot move. A red one means the run is broken, not that the
/// corpus shifted.
///
/// [`RUN_MISSING_OTHER_PIN`] is the strictest: it gates unconditionally (a filtered
/// triage run too), since a same-table cascade / flow-construction regression is a
/// bug at any scope. The rest are checked on full runs beside the snapshot pins.
pub(super) const RUN_MISSING_OTHER_PIN: usize = 0;
pub(super) const RUN_FAMILY_EXTRA_PIN: usize = 0;
pub(super) const RUN_FAMILY_SPAN_MISMATCH_PIN: usize = 0;
/// The related-info channel's zero contracts. The lib relateds match the baseline's
/// masked `lib.x.d.ts:--:--` entries by (code, file), loc-agnostic, so a missing /
/// extra / span-mismatched related is a real defect to explain, not drift. (The
/// channel's `match` count IS drift, and lives in the snapshot.)
pub(super) const RUN_RELATED_MISSING_PIN: usize = 0;
pub(super) const RUN_RELATED_EXTRA_PIN: usize = 0;
pub(super) const RUN_RELATED_SPAN_MISMATCH_PIN: usize = 0;

/// The committed pin snapshot, compiled in — the counts a full `run` is graded
/// against. Machine-regenerated by `run --update` (`deno task
/// conformance:tsc-check:update`), never hand-edited: a tsv parser change or a
/// typescript-go pull shifts several of these at once, which is exactly what a
/// multi-const hand edit gets wrong.
const PIN_SNAPSHOT: &str = include_str!("../tsc_conformance_pins.txt");

/// Repo-relative path of [`PIN_SNAPSHOT`], for user-facing messages.
pub(super) const PIN_SNAPSHOT_FILE: &str =
    "crates/tsv_debug/src/cli/commands/tsc_conformance_pins.txt";

/// The header `--update` writes above the pin lines — the file's own account of what
/// it is and what deliberately stays out of it. Kept here so the renderer reproduces
/// the committed file byte-for-byte (the round-trip is unit-tested).
const PIN_SNAPSHOT_HEADER: &str = "\
# Generated by `deno task conformance:tsc-check:update` — do NOT hand-edit.
#
# The exact, two-sided counts a FULL `tsv_debug tsc_conformance run` is held to: the
# tsv-side census and denominators, which shift by construction when tsv's parser
# advances (more corpus files parse) or the harness port changes. A move in either
# direction fails the run, and re-pinning is the ordinary ritual — so these are
# machine-written rather than hand-edited.
#
# NOT here, deliberately:
#   - the semantically-ZERO gates (family extra, unclassified misses, span mismatches,
#     the lib error channels, panics) — a zero is a contract, not a measurement, so it
#     stays a Rust const no `--update` can move;
#   - the crash-exclusion count, which lives beside the `CRASH_EXCLUSIONS` ledger it
#     describes and moves with it in one deliberate edit;
#   - the oracle-side pins (baseline / roundtrip / pretty / `INDEX_*`), which move only
#     on a deliberate typescript-go bump.
#
# Format: `key = value`, one per line; `#` comments and blank lines are ignored. Every
# key below is required and may appear exactly once. Full reference:
# docs/typechecker.md §Pins & re-pinning.
";

/// How one snapshot key reaches its field. The accessor is `&mut`-shaped so a single
/// table row serves both the parser and the renderer — they cannot disagree about
/// which field a key names.
pub(super) type PinField = fn(&mut RunPins) -> &mut usize;

/// What the run measured for a pin — the report side of every comparison.
pub(super) type PinMeasure = fn(&SkeletonReport) -> usize;

/// One row of the pin table: the snapshot key, the label gate messages use, the
/// [`RunPins`] field it lives in, the report value it grades against, and — on a row
/// that opens a section — the section comment written above it.
pub(super) struct PinRow {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) field: PinField,
    pub(super) measured: PinMeasure,
    section: Option<&'static str>,
}

/// A pin row that continues the current section.
const fn pin_row(
    key: &'static str,
    label: &'static str,
    field: PinField,
    measured: PinMeasure,
) -> PinRow {
    PinRow {
        key,
        label,
        field,
        measured,
        section: None,
    }
}

/// A pin row that opens a new section (its comment is written above the key).
const fn pin_section(
    section: &'static str,
    key: &'static str,
    label: &'static str,
    field: PinField,
    measured: PinMeasure,
) -> PinRow {
    PinRow {
        key,
        label,
        field,
        measured,
        section: Some(section),
    }
}

/// The pin table — the single source of truth for the snapshot: its key set, file
/// order, section shape, gate labels, AND which report value each key grades. Parsing
/// ([`parse_pin_snapshot`]), rendering ([`render_pin_snapshot`]), measuring
/// ([`measured_pins`]), and grading ([`snapshot_pin_failures`]) all iterate it, so a
/// new pinned count is exactly one row here plus its [`RunPins`] field — nothing else
/// to remember, and no way to pin a count nothing measures.
pub(super) const PIN_TABLE: [PinRow; 27] = [
    pin_section(
        "Sweep denominators",
        "in_scope_tests",
        "in-scope tests",
        |p| &mut p.in_scope_tests,
        |r| r.in_scope_tests,
    ),
    pin_row(
        "in_scope_variants",
        "in-scope variants",
        |p| &mut p.in_scope_variants,
        |r| r.in_scope_variants,
    ),
    pin_row(
        "expect_clean",
        "expect-clean graded",
        |p| &mut p.expect_clean,
        |r| r.expect_clean_graded,
    ),
    pin_row(
        "baselined_parsed",
        "baselined parsed",
        |p| &mut p.baselined_parsed,
        |r| r.baselined_parsed,
    ),
    pin_section(
        "Parse-divergence census",
        "parse_rejected",
        "parse-rejected",
        |p| &mut p.parse_rejected,
        |r| r.parse_rejected_total,
    ),
    pin_row(
        "parse_rejected_no_baseline",
        "parse-rejected (no baseline)",
        |p| &mut p.parse_rejected_no_baseline,
        |r| r.parse_rejected_no_baseline,
    ),
    pin_row(
        "parse_rejected_ts1xxx_only",
        "parse-rejected (TS1xxx-only)",
        |p| &mut p.parse_rejected_ts1xxx_only,
        |r| r.parse_rejected_ts1xxx_only,
    ),
    pin_row(
        "parse_rejected_other",
        "parse-rejected (other)",
        |p| &mut p.parse_rejected_other,
        |r| r.parse_rejected_other,
    ),
    pin_row(
        "script_retry",
        "script retries",
        |p| &mut p.script_retry,
        |r| r.script_retry,
    ),
    pin_section(
        "Lib base",
        "lib_files_bound",
        "lib files bound",
        |p| &mut p.lib_files_bound,
        |r| r.lib_files_bound,
    ),
    pin_row(
        "lib_sets",
        "lib sets folded",
        |p| &mut p.lib_sets,
        |r| r.lib_sets_built,
    ),
    pin_section(
        "Family grading",
        "family_graded",
        "family graded",
        |p| &mut p.family_graded,
        |r| r.family_graded_variants,
    ),
    pin_row(
        "family_positive",
        "family positive",
        |p| &mut p.family_positive,
        |r| r.family_positive_variants,
    ),
    pin_row(
        "family_match",
        "family match",
        |p| &mut p.family_match,
        |r| r.family_match,
    ),
    pin_row(
        "family_missing",
        "family missing",
        |p| &mut p.family_missing,
        |r| r.family_missing,
    ),
    pin_row(
        "dup_match",
        "dup match",
        |p| &mut p.dup_match,
        SkeletonReport::dup_match,
    ),
    pin_row(
        "dup_missing",
        "dup missing",
        |p| &mut p.dup_missing,
        SkeletonReport::dup_missing,
    ),
    pin_row(
        "flow_match",
        "flow match",
        |p| &mut p.flow_match,
        SkeletonReport::flow_match,
    ),
    pin_row(
        "flow_missing",
        "flow missing",
        |p| &mut p.flow_missing,
        SkeletonReport::flow_missing,
    ),
    pin_section(
        "Family missing, by deferred cause (`other` is a hard-zero invariant, not pinned here)",
        "missing_merge",
        "missing merge",
        |p| &mut p.missing_merge,
        |r| r.missing(MissingCause::Merge),
    ),
    pin_row(
        "missing_lib",
        "missing lib",
        |p| &mut p.missing_lib,
        |r| r.missing(MissingCause::Lib),
    ),
    pin_row(
        "missing_deferred_late_bound",
        "missing late-bound",
        |p| &mut p.missing_deferred_late_bound,
        |r| r.missing(MissingCause::DeferredLateBound),
    ),
    pin_row(
        "missing_deferred_cfa",
        "missing cfa",
        |p| &mut p.missing_deferred_cfa,
        |r| r.missing(MissingCause::DeferredCfa),
    ),
    pin_section(
        "Related-info channel (matched family primaries)",
        "related_match",
        "related match",
        |p| &mut p.related_match,
        |r| r.related_match,
    ),
    pin_section(
        "Carve-outs",
        "carve_out_rule_a",
        "carve-out rule (a)",
        |p| &mut p.carve_out_rule_a,
        |r| r.carve_out_rule_a,
    ),
    pin_row(
        "carve_out_rule_a_family",
        "carve-out rule (a) family",
        |p| &mut p.carve_out_rule_a_family,
        |r| r.carve_out_rule_a_family,
    ),
    pin_row(
        "module_detection",
        "moduleDetection variants",
        |p| &mut p.module_detection,
        |r| r.module_detection_variants,
    ),
];

/// The exact pins a `run` is held to: the snapshot counts (regenerated by `--update`)
/// plus the zero-valued invariant consts, recorded together in the committed report
/// and the manifest so an artifact states what it was measured against.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
pub(super) struct RunPins {
    in_scope_tests: usize,
    in_scope_variants: usize,
    pub(super) expect_clean: usize,
    baselined_parsed: usize,
    parse_rejected: usize,
    parse_rejected_no_baseline: usize,
    parse_rejected_ts1xxx_only: usize,
    parse_rejected_other: usize,
    family_graded: usize,
    family_positive: usize,
    family_match: usize,
    family_missing: usize,
    dup_match: usize,
    dup_missing: usize,
    flow_match: usize,
    flow_missing: usize,
    missing_merge: usize,
    missing_lib: usize,
    missing_deferred_late_bound: usize,
    missing_deferred_cfa: usize,
    missing_other: usize,
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

impl RunPins {
    /// The invariant (never-snapshot) fields from their consts, every snapshot field
    /// zeroed — the base [`parse_pin_snapshot`] and [`measured_pins`] fill in.
    const fn invariants() -> Self {
        Self {
            in_scope_tests: 0,
            in_scope_variants: 0,
            expect_clean: 0,
            baselined_parsed: 0,
            parse_rejected: 0,
            parse_rejected_no_baseline: 0,
            parse_rejected_ts1xxx_only: 0,
            parse_rejected_other: 0,
            family_graded: 0,
            family_positive: 0,
            family_match: 0,
            family_missing: 0,
            dup_match: 0,
            dup_missing: 0,
            flow_match: 0,
            flow_missing: 0,
            missing_merge: 0,
            missing_lib: 0,
            missing_deferred_late_bound: 0,
            missing_deferred_cfa: 0,
            missing_other: RUN_MISSING_OTHER_PIN,
            family_extra: RUN_FAMILY_EXTRA_PIN,
            family_span_mismatch: RUN_FAMILY_SPAN_MISMATCH_PIN,
            related_match: 0,
            related_missing: RUN_RELATED_MISSING_PIN,
            related_extra: RUN_RELATED_EXTRA_PIN,
            related_span_mismatch: RUN_RELATED_SPAN_MISMATCH_PIN,
            carve_out_rule_a: 0,
            carve_out_rule_a_family: 0,
            module_detection: 0,
            script_retry: 0,
            crash_excluded: CRASH_EXCLUDED_PIN,
            lib_files_bound: 0,
            lib_sets: 0,
        }
    }

    /// Read one pin through a [`PinField`] accessor. `RunPins` is `Copy`, so this
    /// costs a stack copy and mutates nothing.
    pub(super) fn get(&self, field: PinField) -> usize {
        let mut copy = *self;
        *field(&mut copy)
    }
}

/// Parse the pin snapshot: `#` comments and blank lines ignored, every other line
/// `key = value`. Strict on purpose — a malformed line, an unknown or duplicate key,
/// or a missing one is an error naming the offender, since a silently-dropped pin is
/// an unpinned gate.
fn parse_pin_snapshot(text: &str) -> Result<RunPins, String> {
    let mut pins = RunPins::invariants();
    let mut seen = [false; PIN_TABLE.len()];
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let lineno = index + 1;
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "line {lineno}: expected `key = value`, got `{line}`"
            ));
        };
        let (key, value) = (key.trim(), value.trim());
        let Some(row) = PIN_TABLE.iter().position(|r| r.key == key) else {
            return Err(format!("line {lineno}: unknown pin key `{key}`"));
        };
        if seen[row] {
            return Err(format!("line {lineno}: duplicate pin key `{key}`"));
        }
        let parsed: usize = value
            .parse()
            .map_err(|_| format!("line {lineno}: pin `{key}` wants a count, got `{value}`"))?;
        seen[row] = true;
        *(PIN_TABLE[row].field)(&mut pins) = parsed;
    }
    let missing: Vec<&str> = PIN_TABLE
        .iter()
        .zip(seen)
        .filter(|(_, seen)| !seen)
        .map(|(row, _)| row.key)
        .collect();
    if !missing.is_empty() {
        return Err(format!("missing pin key(s): {}", missing.join(", ")));
    }
    Ok(pins)
}

/// Load the committed snapshot.
///
/// A normal run hard-fails on a malformed file — it has no pins to grade against. An
/// `--update` run instead warns and falls back to an all-zero baseline, because
/// regeneration is precisely the fix: a snapshot mangled by a bad merge (conflict
/// markers, a truncated write) must be recoverable by the command the error message
/// recommends, not deadlocked behind it. The old→new lines then honestly read
/// `0 -> N`.
pub(super) fn load_pin_snapshot(update: bool) -> Result<RunPins, CliError> {
    match parse_pin_snapshot(PIN_SNAPSHOT) {
        Ok(pins) => Ok(pins),
        Err(e) if update => {
            eprintln!(
                "Warning: the pin snapshot {PIN_SNAPSHOT_FILE} is malformed — {e}. Regenerating \
                 it from this run; every count reports as `0 -> N`."
            );
            Ok(RunPins::invariants())
        }
        Err(e) => {
            eprintln!(
                "Error: the pin snapshot {PIN_SNAPSHOT_FILE} is malformed — {e}. Restore it, or \
                 regenerate it with `deno task conformance:tsc-check:update` (see \
                 docs/typechecker.md §Pins & re-pinning)."
            );
            Err(CliError::Failed)
        }
    }
}

/// Refuse `--update` unless the checkout is the pinned oracle: the snapshot's counts
/// are denominators *of this corpus*, so writing them from a different typescript-go
/// commit would silently launder a checkout swap into the gate. Moving the oracle is a
/// deliberate two-step — re-pin the oracle-side consts first, then re-pin the counts.
pub(super) fn require_pinned_oracle(checkout: &Path) -> Result<(), CliError> {
    let found = load_baselines(checkout, "run --update")?.len();
    if found == BASELINE_COUNT_PIN {
        return Ok(());
    }
    eprintln!(
        "Error: {} is not at the pinned oracle — found {found} baselines, pin \
         {BASELINE_COUNT_PIN}. --update would write this checkout's denominators into the \
         snapshot; re-pin the oracle-side consts (BASELINE_COUNT_PIN and friends) \
         deliberately first (docs/typechecker.md §Pins & re-pinning).",
        checkout.display()
    );
    Err(CliError::Failed)
}

/// Render the snapshot file: the header, then `key = value` per [`PIN_TABLE`] row in
/// table order, with each section's comment above it.
fn render_pin_snapshot(pins: &RunPins) -> String {
    use std::fmt::Write as _;
    let mut out = String::from(PIN_SNAPSHOT_HEADER);
    for row in &PIN_TABLE {
        if let Some(section) = row.section {
            let _ = write!(out, "\n# {section}\n");
        }
        let _ = writeln!(out, "{} = {}", row.key, pins.get(row.field));
    }
    out
}

/// On-disk path of [`PIN_SNAPSHOT`], for `--update` to rewrite.
/// `CARGO_MANIFEST_DIR` keeps it cwd-independent, matching the audit snapshots.
fn pin_snapshot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/commands/tsc_conformance_pins.txt")
}

/// The pins this run MEASURED — every snapshot field read off the report through its
/// [`PIN_TABLE`] row, the invariant fields left at their consts (a run is only
/// pinnable when those are green anyway).
pub(super) fn measured_pins(report: &SkeletonReport) -> RunPins {
    let mut pins = RunPins::invariants();
    for row in &PIN_TABLE {
        *(row.field)(&mut pins) = (row.measured)(report);
    }
    pins
}

/// Rewrite the snapshot from a green full run: byte-identical → report "no drift" and
/// leave the file untouched; otherwise write it and print one `old -> new` line per
/// changed key.
pub(super) fn update_pin_snapshot(
    committed: &RunPins,
    report: &SkeletonReport,
) -> Result<(), CliError> {
    let measured = measured_pins(report);
    let rendered = render_pin_snapshot(&measured);
    if rendered == PIN_SNAPSHOT {
        println!(
            "\nPins: no drift — all {} count(s) in {PIN_SNAPSHOT_FILE} are current.",
            PIN_TABLE.len()
        );
        return Ok(());
    }
    let path = pin_snapshot_path();
    std::fs::write(&path, &rendered).map_err(|e| {
        eprintln!("Error writing {}: {e}", path.display());
        CliError::Failed
    })?;
    let changes: Vec<String> = PIN_TABLE
        .iter()
        .filter_map(|row| {
            let (old, new) = (committed.get(row.field), measured.get(row.field));
            (old != new).then(|| format!("  {} {old} -> {new}", row.key))
        })
        .collect();
    if changes.is_empty() {
        println!("\nPins: no count moved; rewrote {PIN_SNAPSHOT_FILE} to its canonical shape.");
    } else {
        println!(
            "\nPins: {} moved — rewrote {PIN_SNAPSHOT_FILE}",
            changes.len()
        );
        for change in &changes {
            println!("{change}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_support::sample_report;
    use super::*;
    use crate::tsc_conformance::FAMILIES;

    /// A syntactically valid snapshot text to mutate in the parser tests (the values
    /// are the sample report's, deliberately not the committed ones — a parser test
    /// must not go stale on a re-pin).
    fn valid_snapshot() -> String {
        render_pin_snapshot(&measured_pins(&sample_report()))
    }

    #[test]
    fn committed_snapshot_round_trips() {
        // The one test that keeps the file, the table, and the header honest at once:
        // parse the committed snapshot and re-render it — byte-identical means the key
        // set, the file order, the section shape, and the header all agree with
        // `PIN_TABLE`. It is also exactly what `--update` compares to decide "no drift".
        let pins = parse_pin_snapshot(PIN_SNAPSHOT).expect("committed snapshot parses");
        assert_eq!(render_pin_snapshot(&pins), PIN_SNAPSHOT);
    }

    #[test]
    fn committed_snapshot_carries_exactly_the_table_keys() {
        // Completeness from the file's side: every key the table names is present, and
        // the file names nothing else (an unknown key is a parse error, a missing one
        // too — so a successful parse IS the equality, asserted here explicitly).
        let keys: Vec<&str> = PIN_SNAPSHOT
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim()))
            .collect();
        let table: Vec<&str> = PIN_TABLE.iter().map(|r| r.key).collect();
        assert_eq!(keys, table);
    }

    #[test]
    fn zero_invariants_are_not_snapshot_keys() {
        // The split is the whole design: a semantically-zero contract (and the crash
        // ledger's count) stays a Rust const, so `--update` can never re-pin one.
        for key in [
            "missing_other",
            "family_extra",
            "family_span_mismatch",
            "related_missing",
            "related_extra",
            "related_span_mismatch",
            "crash_excluded",
        ] {
            assert!(
                !PIN_TABLE.iter().any(|r| r.key == key),
                "{key} must not be a snapshot key"
            );
        }
    }

    #[test]
    fn every_pin_row_addresses_its_own_field() {
        // Catches a copy-pasted accessor pointing two keys at one field: write a
        // distinct value through every row, then read them all back. Aliasing collapses
        // the sequence (and would round-trip clean today, since both values start 0).
        let mut pins = RunPins::invariants();
        for (i, row) in PIN_TABLE.iter().enumerate() {
            *(row.field)(&mut pins) = i + 1;
        }
        let read_back: Vec<usize> = PIN_TABLE.iter().map(|row| pins.get(row.field)).collect();
        assert_eq!(read_back, (1..=PIN_TABLE.len()).collect::<Vec<_>>());
    }

    #[test]
    fn every_graded_family_has_partition_pins() {
        // The guard that used to be a fixed-length `[_; FAMILIES.len()]` table: adding a
        // graded family must add its `(match, missing)` snapshot keys too.
        for family in &FAMILIES {
            for suffix in ["match", "missing"] {
                let key = format!("{}_{suffix}", family.key);
                assert!(
                    PIN_TABLE.iter().any(|r| r.key == key),
                    "family `{}` has no `{key}` pin",
                    family.key
                );
            }
        }
    }

    #[test]
    fn every_deferred_missing_cause_is_pinned() {
        // The match is exhaustive, so a new `MissingCause` variant fails to compile here
        // until it declares whether it is a pinned deferred cause or an invariant.
        for cause in [
            MissingCause::Merge,
            MissingCause::Lib,
            MissingCause::DeferredLateBound,
            MissingCause::DeferredCfa,
            MissingCause::Other,
        ] {
            let key = match cause {
                MissingCause::Merge => Some("missing_merge"),
                MissingCause::Lib => Some("missing_lib"),
                MissingCause::DeferredLateBound => Some("missing_deferred_late_bound"),
                MissingCause::DeferredCfa => Some("missing_deferred_cfa"),
                // The hard-zero invariant — deliberately not a snapshot key.
                MissingCause::Other => None,
            };
            match key {
                Some(key) => assert!(
                    PIN_TABLE.iter().any(|r| r.key == key),
                    "{cause:?} has no `{key}` pin"
                ),
                None => assert!(
                    !PIN_TABLE.iter().any(|r| r.key == "missing_other"),
                    "`missing_other` must stay an invariant const"
                ),
            }
        }
    }

    #[test]
    fn measured_pins_keep_the_invariant_consts() {
        // A measured record fills the snapshot fields from the report and the invariant
        // fields from their consts — never from what the run happened to produce.
        let pins = measured_pins(&sample_report());
        assert_eq!(pins.in_scope_tests, 10);
        assert_eq!(pins.missing_other, RUN_MISSING_OTHER_PIN);
        assert_eq!(pins.family_extra, RUN_FAMILY_EXTRA_PIN);
        assert_eq!(pins.crash_excluded, CRASH_EXCLUDED_PIN);
    }

    #[test]
    fn snapshot_parser_ignores_comments_and_blanks() {
        let text = format!("# a comment\n\n{}\n\n# trailing\n", valid_snapshot());
        assert!(parse_pin_snapshot(&text).is_ok());
    }

    #[test]
    fn snapshot_parser_rejects_a_malformed_line() {
        let text = format!("{}nonsense\n", valid_snapshot());
        let err = parse_pin_snapshot(&text).expect_err("malformed line");
        assert!(err.contains("expected `key = value`"), "{err}");
        assert!(err.contains("nonsense"), "{err}");
    }

    #[test]
    fn snapshot_parser_rejects_an_unknown_key() {
        let text = format!("{}bogus_key = 3\n", valid_snapshot());
        let err = parse_pin_snapshot(&text).expect_err("unknown key");
        assert!(err.contains("unknown pin key `bogus_key`"), "{err}");
    }

    #[test]
    fn snapshot_parser_rejects_a_duplicate_key() {
        let text = format!("{}in_scope_tests = 3\n", valid_snapshot());
        let err = parse_pin_snapshot(&text).expect_err("duplicate key");
        assert!(err.contains("duplicate pin key `in_scope_tests`"), "{err}");
    }

    #[test]
    fn snapshot_parser_rejects_a_missing_key() {
        let text: String = valid_snapshot()
            .lines()
            .filter(|l| !l.starts_with("module_detection ="))
            .fold(String::new(), |mut acc, l| {
                acc.push_str(l);
                acc.push('\n');
                acc
            });
        let err = parse_pin_snapshot(&text).expect_err("missing key");
        assert!(
            err.contains("missing pin key(s): module_detection"),
            "{err}"
        );
    }

    #[test]
    fn snapshot_parser_rejects_a_non_numeric_value() {
        let text = valid_snapshot().replace("module_detection = 0", "module_detection = several");
        let err = parse_pin_snapshot(&text).expect_err("non-numeric value");
        assert!(err.contains("wants a count"), "{err}");
    }
}
