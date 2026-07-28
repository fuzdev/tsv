//! tsc_conformance command — ad-hoc queries over the TypeScript-Go conformance
//! baselines (`*.errors.txt`). Pure Rust, no typechecker: tool #1 of the
//! typechecker conformance harness (the "ask important questions" tool). Reads
//! only the committed tsgo baselines — the corpus *inputs* live in a git
//! submodule that is often unmaterialized.

use crate::cli::CliError;
use crate::tsc_conformance::index::IndexReport;
use crate::tsc_conformance::runner::SkeletonReport;
use crate::tsc_conformance::{
    CRASH_EXCLUDED_PIN, FAMILIES, FamilyFilter, MissingCause, RunFilter, RunOptions, baselines_dir,
    check_one, corpus_materialized, denominators, discover_baselines, dump_flow_dot, histogram,
    run_index, run_roundtrip, run_skeleton, tests_by_code,
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

/// INVARIANT GATES (semantically zero, never snapshot values). Each of these is a
/// *contract* rather than a measurement — an unclassified family miss, a family
/// diagnostic the baseline lacks, a span that disagrees — so it stays a Rust const
/// that `run --update` cannot move. A red one means the run is broken, not that the
/// corpus shifted.
///
/// [`RUN_MISSING_OTHER_PIN`] is the strictest: it gates unconditionally (a filtered
/// triage run too), since a same-table cascade / flow-construction regression is a
/// bug at any scope. The rest are checked on full runs beside the snapshot pins.
const RUN_MISSING_OTHER_PIN: usize = 0;
const RUN_FAMILY_EXTRA_PIN: usize = 0;
const RUN_FAMILY_SPAN_MISMATCH_PIN: usize = 0;
/// The related-info channel's zero contracts. The lib relateds match the baseline's
/// masked `lib.x.d.ts:--:--` entries by (code, file), loc-agnostic, so a missing /
/// extra / span-mismatched related is a real defect to explain, not drift. (The
/// channel's `match` count IS drift, and lives in the snapshot.)
const RUN_RELATED_MISSING_PIN: usize = 0;
const RUN_RELATED_EXTRA_PIN: usize = 0;
const RUN_RELATED_SPAN_MISMATCH_PIN: usize = 0;

/// The committed pin snapshot, compiled in — the counts a full `run` is graded
/// against. Machine-regenerated by `run --update` (`deno task
/// conformance:tsc-check:update`), never hand-edited: a tsv parser change or a
/// typescript-go pull shifts several of these at once, which is exactly what a
/// multi-const hand edit gets wrong.
const PIN_SNAPSHOT: &str = include_str!("tsc_conformance_pins.txt");

/// Repo-relative path of [`PIN_SNAPSHOT`], for user-facing messages.
const PIN_SNAPSHOT_FILE: &str = "crates/tsv_debug/src/cli/commands/tsc_conformance_pins.txt";

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
type PinField = fn(&mut RunPins) -> &mut usize;

/// What the run measured for a pin — the report side of every comparison.
type PinMeasure = fn(&SkeletonReport) -> usize;

/// One row of the pin table: the snapshot key, the label gate messages use, the
/// [`RunPins`] field it lives in, the report value it grades against, and — on a row
/// that opens a section — the section comment written above it.
struct PinRow {
    key: &'static str,
    label: &'static str,
    field: PinField,
    measured: PinMeasure,
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
const PIN_TABLE: [PinRow; 27] = [
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
struct RunPins {
    in_scope_tests: usize,
    in_scope_variants: usize,
    expect_clean: usize,
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
    fn get(&self, field: PinField) -> usize {
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
fn load_pin_snapshot(update: bool) -> Result<RunPins, CliError> {
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
fn require_pinned_oracle(checkout: &Path) -> Result<(), CliError> {
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
fn measured_pins(report: &SkeletonReport) -> RunPins {
    let mut pins = RunPins::invariants();
    for row in &PIN_TABLE {
        *(row.field)(&mut pins) = (row.measured)(report);
    }
    pins
}

/// Rewrite the snapshot from a green full run: byte-identical → report "no drift" and
/// leave the file untouched; otherwise write it and print one `old -> new` line per
/// changed key.
fn update_pin_snapshot(committed: &RunPins, report: &SkeletonReport) -> Result<(), CliError> {
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

/// Parse a `--family` value into a [`FamilyFilter`] (a `FAMILIES` key or `all`).
fn parse_family_filter(arg: &str) -> Result<FamilyFilter, String> {
    let lower = arg.to_lowercase();
    FamilyFilter::parse(&lower).ok_or_else(|| {
        format!(
            "Error: --family expects {}, got {lower:?}",
            FamilyFilter::tokens()
        )
    })
}

/// The triage flags active on this run, by name — `--update` refuses when any is set.
fn active_filter_flags(filter: &RunFilter) -> Vec<&'static str> {
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
fn refuse_narrowed_update(update: bool, filter: &RunFilter) -> Result<(), CliError> {
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
fn enforce_run_gates(
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
fn refuse_red_update(report: &SkeletonReport) -> Result<(), CliError> {
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

/// The active triage filters echoed into a filtered manifest, so a consumer sees
/// exactly which slice it was run over. Absent on a full (unfiltered) run.
#[derive(serde::Serialize)]
struct ManifestFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
}

/// The `--emit-manifest` wrapper: the per-variant report, the pins snapshot, and a
/// `filtered` marker (plus the echoed filters). A triage-filtered manifest holds only
/// a partial slice of variant rows and its pins were NOT enforced, so the marker keeps
/// a consumer from mistaking it for a full-run one.
#[derive(serde::Serialize)]
struct RunManifest<'a> {
    filtered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters: Option<ManifestFilters>,
    pins: RunPins,
    report: &'a SkeletonReport,
}

/// Assemble the manifest wrapper, marking whether the run was triage-filtered and
/// echoing the active filters (`--variant key=value` re-joined for display).
fn run_manifest<'a>(
    report: &'a SkeletonReport,
    filter: &RunFilter,
    pins: &RunPins,
) -> RunManifest<'a> {
    let filtered = filter.is_active();
    let filters = filtered.then(|| ManifestFilters {
        test: filter.test.clone(),
        code: filter.code,
        variant: filter.variant.as_ref().map(|(k, v)| format!("{k}={v}")),
    });
    RunManifest {
        filtered,
        filters,
        pins: *pins,
        report,
    }
}

/// Write the `--emit-manifest` JSON (per-variant verdicts + buckets + census + pins +
/// the `filtered` marker). Called only after the gates pass, so a bad manifest never
/// lands.
fn write_manifest(
    report: &SkeletonReport,
    filter: &RunFilter,
    pins: &RunPins,
    path: &Path,
) -> Result<(), CliError> {
    let manifest = run_manifest(report, filter, pins);
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
fn build_report_value(report: &SkeletonReport, pins: &RunPins) -> serde_json::Value {
    serde_json::json!({
        "oracle": "tsgo committed .errors.txt baselines (bind + merge + flow family)",
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
            "dup_match": report.dup_match(),
            "flow_match": report.flow_match(),
            "missing": {
                "total": report.family_missing,
                "dup": report.dup_missing(),
                "flow": report.flow_missing(),
                "merge_path": report.missing(MissingCause::Merge),
                "lib_conflict": report.missing(MissingCause::Lib),
                "late_bound": report.missing(MissingCause::DeferredLateBound),
                "cfa": report.missing(MissingCause::DeferredCfa),
                "other": report.missing(MissingCause::Other),
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
        "pins": pins,
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
        "Oracle: tsgo committed `.errors.txt` baselines (bind + merge + flow family). \
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

    s.push_str(
        "## Family (dup 2300 / 2451 / 2567 / 2528 + merge 2397 / 2649 / 2664 / 2671; \
         flow 7027 / 7028)\n\n",
    );
    let _ = writeln!(
        s,
        "- match: {} (dup {}, flow {})",
        report.family_match,
        report.dup_match(),
        report.flow_match()
    );
    let _ = writeln!(
        s,
        "- missing: {} (dup {}, flow {})",
        report.family_missing,
        report.dup_missing(),
        report.flow_missing()
    );
    let _ = writeln!(
        s,
        "  - by cause: merge-path {}, lib-conflict {}, late-bound {}, cfa {}, other {}",
        report.missing(MissingCause::Merge),
        report.missing(MissingCause::Lib),
        report.missing(MissingCause::DeferredLateBound),
        report.missing(MissingCause::DeferredCfa),
        report.missing(MissingCause::Other)
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

/// Whether a run may write the committed `--report` artifact: it is a full-run
/// product (the pins hold only on a full run), so a filtered (triage) run must
/// refuse it. A run that didn't request `--report` trivially may.
fn may_write_report(report_requested: bool, filtered: bool) -> bool {
    !(report_requested && filtered)
}

/// Write the committed compact report to `<base>.json` + `<base>.md` (full runs only;
/// deterministic). Called only after the gates pass.
fn write_report(report: &SkeletonReport, pins: &RunPins, base: &Path) -> Result<(), CliError> {
    let json_path = PathBuf::from(format!("{}.json", base.display()));
    let md_path = PathBuf::from(format!("{}.md", base.display()));
    let value = build_report_value(report, pins);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A report with the deterministic sections populated (per-code maps out of
    /// natural order, a few counters, and a nonzero wall-clock).
    fn sample_report() -> SkeletonReport {
        SkeletonReport {
            in_scope_tests: 10,
            in_scope_variants: 12,
            expect_clean_graded: 4,
            clean_pass: 4,
            baselined_parsed: 8,
            family_graded_variants: 7,
            family_positive_variants: 3,
            family_match: 5,
            family_missing: 2,
            missing_by_cause: std::collections::BTreeMap::from([(MissingCause::Other, 2usize)]),
            family_extra: 0,
            related_match: 1,
            // Inserted out of ascending order — the BTreeMap and the render must sort.
            family_match_by_code: [(2528u32, 1usize), (2300, 2), (2451, 3)]
                .into_iter()
                .collect(),
            family_missing_by_code: [(2664u32, 1usize), (2451, 1)].into_iter().collect(),
            wall_ms: 123_456,
            ..SkeletonReport::default()
        }
    }

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

    #[test]
    fn report_json_is_deterministic() {
        // Two builds from the same report serialize byte-for-byte identically (sorted
        // maps, no timing) — the committed artifact must be diff-clean across re-runs.
        let r = sample_report();
        let pins = measured_pins(&r);
        let a = serde_json::to_string_pretty(&build_report_value(&r, &pins)).unwrap();
        let b = serde_json::to_string_pretty(&build_report_value(&r, &pins)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn report_md_is_deterministic() {
        let r = sample_report();
        assert_eq!(render_report_md(&r), render_report_md(&r));
    }

    #[test]
    fn per_code_table_iterates_sorted() {
        // The per-code table lists codes ascending regardless of map insertion order.
        let md = render_report_md(&sample_report());
        let p2300 = md.find("TS2300").expect("TS2300 row");
        let p2451 = md.find("TS2451").expect("TS2451 row");
        let p2528 = md.find("TS2528").expect("TS2528 row");
        assert!(
            p2300 < p2451 && p2451 < p2528,
            "per-code rows not ascending"
        );
    }

    #[test]
    fn wall_clock_excluded_from_report() {
        // Two reports differing ONLY in wall-clock produce identical JSON and Markdown —
        // machine-varying timing never reaches the committed content.
        let mut fast = sample_report();
        fast.wall_ms = 0;
        let mut slow = sample_report();
        slow.wall_ms = 987_654_321;
        assert_eq!(
            serde_json::to_string_pretty(&build_report_value(&fast, &measured_pins(&fast)))
                .unwrap(),
            serde_json::to_string_pretty(&build_report_value(&slow, &measured_pins(&slow)))
                .unwrap(),
        );
        assert_eq!(render_report_md(&fast), render_report_md(&slow));
    }

    #[test]
    fn sanitize_replaces_path_separators() {
        // Both slash flavors become `_` so a `suite/config` identity is one path segment.
        assert_eq!(sanitize_artifact_name("a/b\\c"), "a_b_c");
    }

    #[test]
    fn sanitize_replaces_whitespace() {
        assert_eq!(sanitize_artifact_name("a b\tc"), "a_b_c");
    }

    #[test]
    fn sanitize_preserves_baseline_identity() {
        // A real baseline name has no path-hostile chars; parens/`=`/`.` pass through.
        let name = "duplicateVar(target=es2015).errors.txt";
        assert_eq!(sanitize_artifact_name(name), name);
    }

    #[test]
    fn sanitize_is_not_injective() {
        // The mapping has no collision handling by design: distinct inputs whose only
        // difference is a slash-vs-space collapse to the same basename. Pins current
        // behavior (do not add collision handling here).
        assert_eq!(sanitize_artifact_name("a/b"), sanitize_artifact_name("a b"));
    }

    #[test]
    fn report_write_requires_full_run() {
        // A committed report is a full-run product: allowed unless requested on a
        // filtered run.
        assert!(may_write_report(false, false));
        assert!(may_write_report(false, true));
        assert!(may_write_report(true, false));
        assert!(!may_write_report(true, true));
    }

    #[test]
    fn manifest_marks_and_echoes_filtered_runs() {
        // A full (unfiltered) run: `filtered` is false and no `filters` object is
        // emitted — nothing distinguishes it from a plain full-run manifest.
        let r = sample_report();
        let pins = measured_pins(&r);
        let full = serde_json::to_value(run_manifest(&r, &RunFilter::default(), &pins)).unwrap();
        assert_eq!(full["filtered"], serde_json::json!(false));
        assert!(full.get("filters").is_none());

        // A triage-filtered run: `filtered` is true and the active filters are echoed
        // (an unset filter — here `--test` — is omitted; `--variant` is re-joined).
        let filter = RunFilter {
            test: None,
            code: Some(2300),
            variant: Some(("target".to_string(), "es2015".to_string())),
            family: None,
        };
        let filtered = serde_json::to_value(run_manifest(&r, &filter, &pins)).unwrap();
        assert_eq!(filtered["filtered"], serde_json::json!(true));
        assert_eq!(filtered["filters"]["code"], serde_json::json!(2300));
        assert_eq!(
            filtered["filters"]["variant"],
            serde_json::json!("target=es2015")
        );
        assert!(filtered["filters"].get("test").is_none());
    }
}
