//! The walking-skeleton runner: drive `tsv_check` over the in-scope corpus and
//! grade the checker plumbing end-to-end.
//!
//! This is tool #4 of the harness (the runner) in its first form. It reuses the
//! S1 substrate — corpus index, directive parser, variant expansion, the
//! unsupported-option skip classes — and adds the checker leg: for every
//! **in-scope** variant (single-file, non-JSX, non-JS-flavored, not skipped,
//! not an unsupported-option variant) it parses the unit via `tsv_check`'s goal
//! rule, runs `check_program`, and grades the result. The checker emits nothing
//! yet, so the meaningful gate is that every **expect-clean** in-scope variant
//! (one with no on-disk baseline) grades clean (zero diagnostics) with zero
//! panics — proving the harness<->checker plumbing before any family gate.
//!
//! A single-file test's variants all parse identically (the goal rule is
//! directive-independent), so the parse+check runs **once per test** and its
//! outcome is attributed to each in-scope variant — correct while `check` is a
//! no-op and cheap regardless.
//!
//! The **parse-divergence census** (informational, not gated) counts in-scope
//! variants tsv parse-rejects, split by baseline shape (none / TS1xxx-only /
//! other), plus how many parses needed the `Goal::Script` retry — the standing
//! window on tsv's parser vs tsgo's implied parse verdict (a tsv over-rejection
//! shows up as a rejected variant against an absent-or-non-1xxx baseline).
//!
//! Crash containment: the whole sweep runs on a generous-stack worker thread
//! (the corpus has pathological-nesting tests and tsv's parser has no depth
//! guard), and each test's check is wrapped in `catch_unwind` so a panic lands
//! in its own bucket instead of killing the run. A stack-overflow *abort* can't
//! be caught; the [`CRASH_EXCLUSIONS`] list carves out any test that aborts even
//! the big stack (empty on the pinned corpus).
//
// tsgo: internal/compiler/program.go GetDiagnosticsOfAnyProgram (the pipeline)
// tsgo: internal/testrunner/compiler_runner.go (the in-scope selection)

use crate::tsc_conformance::baseline::{parse_baseline, parse_summary_block};
use crate::tsc_conformance::corpus::{discover_corpus, read_corpus_file, CorpusTest};
use crate::tsc_conformance::directives::{extract_settings, split_units, Unit};
use crate::tsc_conformance::discovery::{baselines_dir, discover_baselines, Baseline};
use crate::tsc_conformance::index::{is_js_flavored, is_jsx_scoped};
use crate::tsc_conformance::libs::LibResolver;
use crate::tsc_conformance::options_meta::{
    is_config_file_name, variant_is_unsupported, SKIPPED_TESTS,
};
use crate::tsc_conformance::variants::{config_name, expand, Variant};
use bumpalo::Bump;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Instant;
use tsv_check::{
    bind_program, check_bound, check_program, Diagnostic, ParseReport, SourceUnit,
};
use tsv_lang::{LocationMapper, LocationTracker};

/// The bind-time duplicate/conflict family this slice grades: TS2300 (duplicate
/// identifier), TS2451 (block-scoped redeclare), TS2567 (enum-merge), TS2528
/// (multiple default exports), plus the merge-path codes TS2397/2649/2664/2671
/// (emitted only from the merge phase, out of this slice — they land as *misses*).
const FAMILY_CODES: [u32; 8] = [2300, 2451, 2567, 2528, 2397, 2649, 2664, 2671];

/// The merge-path family codes — a *missing* of one of these is a merge-phase gap
/// (S4), not a same-table cascade bug.
const MERGE_CODES: [u32; 4] = [2397, 2649, 2664, 2671];

/// The TS1xxx codes the binder itself emits (strict-mode + private-identifier
/// checks) — they prove nothing about parse state, so a baseline carrying only
/// these does not trigger the recovery-AST carve-out (predicate v1, rule a).
const BIND_EMITTED_TS1XXX: [u32; 12] =
    [1100, 1101, 1102, 1210, 1212, 1213, 1214, 1215, 1262, 1344, 1359, 18012];

/// The P1-family baselines whose family diagnostics come from a standard-library
/// conflict (S5). These now **match** via the lib base; the classifier is kept as a
/// regression guard — a *missing* in one of these buckets to `missing_lib` (pinned
/// 0) rather than `missing_other`, so a lib-detection regression fails loudly.
const LIB_CONFLICT_BASELINES: [&str; 5] = [
    "intersectionsOfLargeUnions2.ts",
    "jsExportMemberMergedWithModuleAugmentation2.ts",
    "promiseDefinitionTest.ts",
    "recursiveComplicatedClasses.ts",
    "variableDeclarationInStrictMode1.ts",
];

/// Worker-thread stack for the sweep: the corpus has deeply-nested tests and
/// tsv's recursive-descent parser has no depth guard, so the default 8 MiB
/// overflows. 512 MiB is virtual-only reserve on Linux.
const SKELETON_STACK: usize = 512 * 1024 * 1024;

/// How a crash-excluded test fails, and whether its liveness is probeable.
// `GenuineAbort` is the designed flag for a future stack-overflow entry (none on
// the pinned corpus); it is un-probeable, so it is never re-run.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum CrashKind {
    /// A debug-build `debug_assert!` panic `catch_unwind` contains — probeable:
    /// the sweep re-runs it under `catch_unwind` and FAILS if it no longer
    /// panics (a fix landed, so the entry is stale and must be dropped).
    CatchablePanic,
    /// An uncatchable stack-overflow *abort* even on [`SKELETON_STACK`] — not
    /// probeable (probing would abort the whole run), so it is trusted, not tested.
    GenuineAbort,
}

/// Tests that crash the tsv parser — carved out by basename, counted, and
/// reported (never silently). Each entry names its cause + kind; the list is a
/// tracked-defect ledger, not a way to hide bugs. A [`CrashKind::CatchablePanic`]
/// entry is liveness-probed every run (see [`probe_crash_exclusion`]).
const CRASH_EXCLUSIONS: &[(&str, CrashKind)] = &[
    // tsv_ts robustness bug: `export * from <identifier>;` (a non-string module
    // specifier) trips a `debug_assert!(TokenKind::String)` in
    // `parse_string_literal` (parser/mod.rs). Dev-profile only (debug_assert is
    // compiled out in release), so `cargo run` — the gate's profile — panics.
    // A future tsv_ts fix should reject the form gracefully; then drop this entry.
    ("exportDeclarationInInternalModule.ts", CrashKind::CatchablePanic),
];

/// The [`CrashKind`] of a crash-excluded test, or `None` if not excluded.
fn crash_exclusion_kind(basename: &str) -> Option<CrashKind> {
    CRASH_EXCLUSIONS.iter().find(|(n, _)| *n == basename).map(|(_, k)| *k)
}

/// One expect-clean variant that graded non-clean (should never happen while the
/// checker is a no-op — a non-empty list is a gate failure).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CleanFail {
    /// The `suite/config_name` baseline-space identity.
    pub variant: String,
    /// The number of diagnostics the checker (wrongly) emitted.
    pub diagnostics: usize,
}

/// One test whose check panicked (caught) — a gate failure.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PanicRecord {
    /// The corpus test's relative path.
    pub test: String,
    /// The panic payload's message (downcast to `&str`/`String`), for triage.
    pub payload: String,
}

/// The skeleton sweep report.
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct SkeletonReport {
    /// Tests that passed the test-level in-scope filter and have >=1 in-scope
    /// variant.
    pub in_scope_tests: usize,
    /// In-scope variants graded (parsed or parse-rejected).
    pub in_scope_variants: usize,
    /// In-scope variants that parsed and have no on-disk baseline (expect-clean).
    pub expect_clean_graded: usize,
    /// Expect-clean variants that graded clean (zero diagnostics). Gate: must
    /// equal `expect_clean_graded`.
    pub clean_pass: usize,
    /// Expect-clean variants that graded non-clean (gate failure list).
    pub clean_fail: Vec<CleanFail>,
    /// In-scope variants that parsed and DO have a baseline.
    pub baselined_parsed: usize,

    // --- family grading (the S3 gate) ---
    /// Parsed-with-baseline variants family-graded (not carved by predicate v1).
    pub family_graded_variants: usize,
    /// ...of those, whose baseline carries at least one family code.
    pub family_positive_variants: usize,
    /// Family diagnostics that matched (file, line, col, code).
    pub family_match: usize,
    /// Family baseline diagnostics with no matching diagnostic of ours (classified
    /// below). Expected to be all merge/lib until S4/S5 land.
    pub family_missing: usize,
    /// Family diagnostics we emit that the baseline lacks. **Gate: must be 0.**
    pub family_extra: usize,
    /// Right code + file, wrong position (greedy-paired).
    pub family_span_mismatch: usize,

    // --- related-info grading (its own pinned channel; does NOT gate the
    // per-variant primary verdict) — graded only for matched primaries ---
    /// Related-info entries that matched (code, file, line, col).
    pub related_match: usize,
    /// Baseline related entries with no matching related of ours.
    pub related_missing: usize,
    /// Related entries we emit the baseline lacks.
    pub related_extra: usize,
    /// Right code + file, wrong position (greedy-paired).
    pub related_span_mismatch: usize,
    /// Sample related over-emissions.
    pub related_extra_samples: Vec<String>,
    /// Sample related misses.
    pub related_missing_samples: Vec<String>,

    /// ...missing attributed to the merge phase (TS2397/2649/2664/2671).
    pub missing_merge: usize,
    /// ...missing attributed to absent lib binding (a `LIB_CONFLICT_BASELINES` test).
    pub missing_lib: usize,
    /// ...missing not attributable to merge/lib — investigate (a same-table miss
    /// is a cascade bug).
    pub missing_other: usize,
    /// Variants carved out by predicate v1 rule (a): tsv parses clean but the
    /// baseline carries a non-bind TS1xxx code (recovery-AST incomparability).
    pub carve_out_rule_a: usize,
    /// ...of those, whose baseline also carries a family code.
    pub carve_out_rule_a_family: usize,
    /// In-scope variants that set `moduleDetection` (a watch item — module-ness is
    /// inert for the family cascade, so the parse-once shortcut stays valid).
    pub module_detection_variants: usize,
    /// Sample extra diagnostics (gate failures to fix).
    pub extra_samples: Vec<String>,
    /// Sample unattributed misses (candidate cascade bugs).
    pub missing_other_samples: Vec<String>,
    /// Sample span mismatches.
    pub span_mismatch_samples: Vec<String>,
    /// In-scope variants tsv parse-rejected (census; informational).
    pub parse_rejected_total: usize,
    /// ...of those, with no on-disk baseline (a likely tsv over-rejection).
    pub parse_rejected_no_baseline: usize,
    /// ...with a TS1xxx-only baseline (ambiguous: tsgo parse error or grammar).
    pub parse_rejected_ts1xxx_only: usize,
    /// ...with a baseline carrying non-TS1xxx codes (tsv rejects what tsgo checked).
    pub parse_rejected_other: usize,
    /// In-scope parsed variants that needed the `Goal::Script` retry (census).
    pub script_retry: usize,
    /// Tests whose check panicked (caught) and are NOT crash-excluded. Gate:
    /// must be empty.
    pub panics: Vec<PanicRecord>,
    /// Tests skipped by the crash-exclusion ledger (tracked parser aborts/panics).
    pub excluded_crashes: usize,

    // --- lib base (S5) ---
    /// Distinct lib `.d.ts` files parsed + bound this run (informational).
    pub lib_files_bound: usize,
    /// Distinct resolved lib sets folded into a base this run (informational).
    pub lib_sets_built: usize,
    /// Lib files that failed to parse (`file: error`). **Gate: must be empty.**
    pub lib_parse_errors: Vec<String>,
    /// Referenced lib files not found on disk. **Gate: must be empty.**
    pub lib_missing_files: Vec<String>,
    /// Unrecognized `@lib` / `/// <reference lib>` names. **Gate: must be empty.**
    pub lib_unknown_names: Vec<String>,
    /// Catchable-panic exclusions that no longer panic (a fix landed) — the entry
    /// is stale and must be dropped. **Gate: must be empty.**
    pub stale_exclusions: Vec<String>,
    /// Total bound nodes across in-scope tests (informational).
    pub total_nodes: u64,
    /// Wall-clock of the sweep in milliseconds.
    pub wall_ms: u128,
}

/// The baseline shape used to bucket a parse-rejected variant.
enum BaselineShape {
    None,
    Ts1xxxOnly,
    Other,
}

/// Run the skeleton sweep on a generous-stack worker thread.
///
/// # Errors
///
/// Returns an error string if the worker cannot spawn, the worker panics
/// outside a contained per-test check, or corpus discovery fails.
pub fn run_skeleton(checkout: &Path) -> Result<SkeletonReport, String> {
    let checkout = checkout.to_path_buf();
    let handle = std::thread::Builder::new()
        .stack_size(SKELETON_STACK)
        .name("tsc-skeleton".to_string())
        .spawn(move || run_skeleton_inner(&checkout))
        .map_err(|e| format!("spawn skeleton worker: {e}"))?;
    handle.join().map_err(|_| "skeleton worker panicked".to_string())?
}

fn run_skeleton_inner(checkout: &Path) -> Result<SkeletonReport, String> {
    let start = Instant::now();
    let corpus = discover_corpus(checkout)?;
    let baselines = discover_baselines(&baselines_dir(checkout))?;

    // Baseline lookup keyed by (suite, config-name) — exactly the runner's join.
    let mut ondisk: HashMap<(&str, String), &Baseline> = HashMap::new();
    for baseline in &baselines {
        if let Some((suite, name)) = baseline.relative_path.split_once('/') {
            ondisk.insert((suite, name.to_string()), baseline);
        }
    }

    let mut report = SkeletonReport::default();
    let mut resolver = LibResolver::new(checkout);

    for test in &corpus {
        if SKIPPED_TESTS.contains(&test.basename.as_str()) {
            continue;
        }
        if let Some(kind) = crash_exclusion_kind(&test.basename) {
            report.excluded_crashes += 1;
            // Liveness probe: a catchable-panic entry must still panic; if it no
            // longer does, the ledger entry is stale and the run fails.
            if kind == CrashKind::CatchablePanic && !probe_crash_exclusion(test) {
                report.stale_exclusions.push(test.basename.clone());
            }
            continue;
        }

        let content = read_corpus_file(&test.path)?;
        let settings = extract_settings(&content);
        let units = split_units(&content, &test.basename);

        // Test-level in-scope filter: single-file (one non-config unit), not
        // JSX-scoped, not JS-flavored.
        if units.len() != 1 || is_config_file_name(&units[0].name) {
            continue;
        }
        if is_jsx_scoped(test, &settings) || is_js_flavored(test, &settings) {
            continue;
        }

        let expansion = expand(&settings);
        if expansion.cap_exceeded {
            continue;
        }
        let in_scope: Vec<&Variant> =
            expansion.variants.iter().filter(|v| !variant_is_unsupported(&v.config)).collect();
        if in_scope.is_empty() {
            continue;
        }

        report.in_scope_tests += 1;
        grade_test(test, &units[0], &in_scope, &ondisk, &mut resolver, &mut report);
    }

    // Fold in the resolver's lib-base census (parse-once/fold-once counts + gates).
    report.lib_files_bound = resolver.files_bound();
    report.lib_sets_built = resolver.sets_built();
    report.lib_parse_errors =
        resolver.parse_errors().iter().map(|(f, e)| format!("{f}: {e}")).collect();
    report.lib_missing_files = resolver.missing_files().to_vec();
    report.lib_unknown_names = {
        let mut names: Vec<String> = resolver.unknown_libs().to_vec();
        names.sort_unstable();
        names.dedup();
        names
    };

    report.wall_ms = start.elapsed().as_millis();
    Ok(report)
}

/// Re-run a catchable-panic crash exclusion under `catch_unwind`, returning
/// whether it **still panics**. A `false` (it completed) means the tracked defect
/// is fixed and the ledger entry is stale.
fn probe_crash_exclusion(test: &CorpusTest) -> bool {
    let Ok(content) = read_corpus_file(&test.path) else {
        // Can't read it -> can't disprove the panic; treat as still-live.
        return true;
    };
    let units = split_units(&content, &test.basename);
    let arena = Bump::new();
    let source_units: Vec<SourceUnit<'_>> =
        units.iter().map(|u| SourceUnit::new(&u.name, &u.content)).collect();
    // Silence the default panic hook for the deliberate probe (we expect it to
    // panic; the message would otherwise leak to stderr and read as a failure).
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        let _ = check_program(&source_units, &arena);
    }))
    .is_err();
    std::panic::set_hook(prev);
    panicked
}

/// Extract a caught panic payload's message (the `&str` / `String` cases the
/// standard panic machinery produces).
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "(non-string panic payload)".to_string()
    }
}

/// Parse+bind one single-file test once, then — per in-scope variant — merge it
/// against that variant's resolved lib base and grade the result. Parse+bind is
/// variant-independent; only the merge (and thus the lib-conflict family) varies by
/// the resolved lib set, so a variant with a Promise/Symbol/… global conflicts at
/// one target and is clean at another.
fn grade_test(
    test: &CorpusTest,
    unit: &Unit,
    in_scope: &[&Variant],
    ondisk: &HashMap<(&str, String), &Baseline>,
    resolver: &mut LibResolver,
    report: &mut SkeletonReport,
) {
    // Parse + bind on a fresh arena, contained against panics (the tsv parser is the
    // panic source; the merge over owned data that follows is deterministic).
    let arena = Bump::new();
    let bound = match catch_unwind(AssertUnwindSafe(|| {
        bind_program(&[SourceUnit::new(&unit.name, &unit.content)], &arena)
    })) {
        Ok(bound) => bound,
        Err(payload) => {
            report.panics.push(PanicRecord {
                test: test.relative_path.clone(),
                payload: panic_payload_message(&*payload),
            });
            return;
        }
    };

    // The single unit's parse outcome (parse_reports is never empty for one input).
    let reports = bound.parse_reports();
    let Some(&(_, parse)) = reports.first() else { return };
    let parsed = matches!(parse, ParseReport::Parsed(_));

    // The unit's line map — reused across the test's variants for the parsed case.
    let line_map = parsed.then(|| LocationTracker::new_ecmascript_with_map(&unit.content));

    for variant in in_scope {
        report.in_scope_variants += 1;
        if variant.config.contains_key("moduledetection") {
            report.module_detection_variants += 1;
        }
        let name = config_name(&test.basename, &variant.description);
        let baseline = ondisk.get(&(test.suite, name.clone())).copied();

        match parse {
            ParseReport::Rejected { .. } => {
                report.parse_rejected_total += 1;
                match baseline_shape(baseline) {
                    BaselineShape::None => report.parse_rejected_no_baseline += 1,
                    BaselineShape::Ts1xxxOnly => report.parse_rejected_ts1xxx_only += 1,
                    BaselineShape::Other => report.parse_rejected_other += 1,
                }
            }
            ParseReport::Parsed(facts) => {
                if facts.used_script_retry {
                    report.script_retry += 1;
                }
                // Resolve this variant's lib set (cached) and merge the bound program
                // against it — the merge diagnostics are the lib-conflict family.
                let base = resolver.base_for(&variant.config);
                let result = check_bound(&bound, base.as_deref());
                let lib_files = base.as_ref().map_or(&[][..], |b| b.lib_files.as_slice());

                match baseline {
                    None => {
                        report.expect_clean_graded += 1;
                        if result.diagnostics.is_empty() {
                            report.clean_pass += 1;
                        } else {
                            report.clean_fail.push(CleanFail {
                                variant: format!("{}/{name}", test.suite),
                                diagnostics: result.diagnostics.len(),
                            });
                        }
                    }
                    Some(b) => {
                        report.baselined_parsed += 1;
                        // `parsed` => `line_map` is `Some`; the `None` arm is dead.
                        let ours_family = match line_map.as_ref() {
                            Some((tracker, map)) => {
                                let mapper = LocationMapper { tracker, map };
                                build_ours_family(&result.diagnostics, &unit.name, &mapper, lib_files)
                            }
                            None => Vec::new(),
                        };
                        grade_family(test, &name, b, &ours_family, report);
                    }
                }
            }
        }
    }

    // Node total: counted once per test (all variants share the parse+bind).
    report.total_nodes += bound.total_node_count();
}

/// The number of program units in the single-file sweep (a lib FileId is
/// `>= UNITS_LEN`, translating to `lib_files[FileId - UNITS_LEN]`).
const UNITS_LEN: u32 = 1;

/// Build our family multiset for one variant's diagnostics, resolving each FileId to
/// a display name. A **lib-file primary** (FileId beyond the program units) is
/// dropped — the baseline masks it (`lib.x.d.ts(--,--)`) — and a lib-sourced related
/// carries the lib file name with a masked location so it matches the baseline's
/// `lib.x.d.ts:--:--` related by `(code, file)`.
fn build_ours_family(
    diagnostics: &[Diagnostic],
    unit_name: &str,
    mapper: &LocationMapper<'_>,
    lib_files: &[String],
) -> Vec<FamilyEntry> {
    diagnostics
        .iter()
        .filter(|d| FAMILY_CODES.contains(&d.code))
        .filter_map(|d| {
            let file = d.file?;
            // A lib-file primary is masked in the baseline — exclude it.
            if file.index() >= UNITS_LEN as usize {
                return None;
            }
            let (_, pos) = mapper.pos_and_position(d.span.start);
            let key = FamilyDiag {
                file: unit_name.to_string(),
                line: pos.line as u32,
                col: pos.column as u32 + 1,
                code: d.code,
            };
            let related = d
                .related
                .iter()
                .map(|r| resolve_related(r, unit_name, mapper, lib_files))
                .collect();
            Some(FamilyEntry { key, related })
        })
        .collect()
}

/// Resolve one related-info entry's FileId to a [`RelatedKey`]: an in-unit related
/// carries its computed location; a lib-sourced related carries the lib file name
/// and a masked (`None`) location.
fn resolve_related(
    r: &Diagnostic,
    unit_name: &str,
    mapper: &LocationMapper<'_>,
    lib_files: &[String],
) -> RelatedKey {
    match r.file {
        Some(f) if f.index() < UNITS_LEN as usize => {
            let (_, pos) = mapper.pos_and_position(r.span.start);
            RelatedKey {
                code: r.code,
                file: unit_name.to_string(),
                loc: Some((pos.line as u32, pos.column as u32 + 1)),
            }
        }
        Some(f) => {
            let idx = f.index() - UNITS_LEN as usize;
            RelatedKey {
                code: r.code,
                file: lib_files.get(idx).cloned().unwrap_or_default(),
                loc: None,
            }
        }
        None => RelatedKey { code: r.code, file: unit_name.to_string(), loc: None },
    }
}

/// One family diagnostic in baseline coordinates: `(file, 1-based line, 1-based
/// UTF-16 col, code)`.
#[derive(Clone, PartialEq, Eq, Hash)]
struct FamilyDiag {
    file: String,
    line: u32,
    col: u32,
    code: u32,
}

/// One related-info entry in baseline coordinates: `(code, file, location)`. A
/// `--,--` (default-library) location is [`None`] and compares by code+file only.
#[derive(Clone, PartialEq, Eq, Hash)]
struct RelatedKey {
    code: u32,
    file: String,
    loc: Option<(u32, u32)>,
}

/// A family primary plus its related-info entries — the unit the related-info
/// channel grades (a matched primary's related sets are compared as multisets).
struct FamilyEntry {
    key: FamilyDiag,
    related: Vec<RelatedKey>,
}

/// Grade one parsed-with-baseline variant's family diagnostics against its
/// baseline, folding the buckets into `report`. Applies predicate v1 rule (a)
/// (recovery-AST carve-out) first, then the primary-code channel and — for the
/// matched primaries — the independent related-info channel.
fn grade_family(
    test: &CorpusTest,
    name: &str,
    baseline: &Baseline,
    ours: &[FamilyEntry],
    report: &mut SkeletonReport,
) {
    let Ok(content) = std::fs::read_to_string(&baseline.path) else { return };
    let base_all = parse_base_diags(&content);

    // Predicate v1 rule (a): tsv parses clean (it did — this variant parsed) and
    // the baseline carries a non-bind TS1xxx code -> recovery-AST incomparable.
    let has_nonbind_ts1xxx = base_all.iter().any(|d| {
        (1000..2000).contains(&d.code)
            && u32::try_from(d.code).is_ok_and(|c| !BIND_EMITTED_TS1XXX.contains(&c))
    });
    let base_family: Vec<FamilyEntry> = base_all
        .iter()
        .filter_map(|d| {
            let code = u32::try_from(d.code).ok()?;
            if !FAMILY_CODES.contains(&code) {
                return None;
            }
            Some(FamilyEntry {
                key: FamilyDiag { file: d.file.clone()?, line: d.line?, col: d.col?, code },
                related: d.related.clone(),
            })
        })
        .collect();
    let has_family = !base_family.is_empty();

    if has_nonbind_ts1xxx {
        report.carve_out_rule_a += 1;
        if has_family {
            report.carve_out_rule_a_family += 1;
        }
        return;
    }

    report.family_graded_variants += 1;
    if has_family {
        report.family_positive_variants += 1;
    }

    let ours_keys: Vec<FamilyDiag> = ours.iter().map(|e| e.key.clone()).collect();
    let base_keys: Vec<FamilyDiag> = base_family.iter().map(|e| e.key.clone()).collect();
    let buckets = family_buckets(&ours_keys, &base_keys);
    report.family_match += buckets.matched;
    report.family_span_mismatch += buckets.span_mismatch;
    report.family_extra += buckets.extra;
    if buckets.extra > 0 && report.extra_samples.len() < 20 {
        report.extra_samples.push(format!("{}/{name} (+{})", test.suite, buckets.extra));
    }
    if buckets.span_mismatch > 0 && report.span_mismatch_samples.len() < 20 {
        report
            .span_mismatch_samples
            .push(format!("{}/{name} (~{})", test.suite, buckets.span_mismatch));
    }
    let is_lib = LIB_CONFLICT_BASELINES.contains(&test.basename.as_str());
    for (code, count) in buckets.missing_by_code {
        report.family_missing += count;
        if MERGE_CODES.contains(&code) {
            report.missing_merge += count;
        } else if is_lib {
            report.missing_lib += count;
        } else {
            report.missing_other += count;
            if report.missing_other_samples.len() < 20 {
                report
                    .missing_other_samples
                    .push(format!("{}/{name} TS{code} x{count}", test.suite));
            }
        }
    }

    // The related-info channel (independent of the primary verdict): grade related
    // multisets only for the primaries that matched.
    let rel = grade_related(ours, &base_family);
    report.related_match += rel.matched;
    report.related_span_mismatch += rel.span_mismatch;
    report.related_extra += rel.extra;
    report.related_missing += rel.missing;
    if rel.extra > 0 && report.related_extra_samples.len() < 20 {
        report.related_extra_samples.push(format!("{}/{name} (+{})", test.suite, rel.extra));
    }
    if rel.missing > 0 && report.related_missing_samples.len() < 20 {
        report.related_missing_samples.push(format!("{}/{name} (-{})", test.suite, rel.missing));
    }
}

/// A baseline summary diagnostic with its parsed related-info entries.
struct BaseDiag {
    file: Option<String>,
    line: Option<u32>,
    col: Option<u32>,
    /// The `TS<code>` (i32 — the harness's `TS-1` and non-family codes appear here).
    code: i32,
    related: Vec<RelatedKey>,
}

/// Parse a baseline into summary diagnostics with related info, via the full
/// [`parse_baseline`] model (100% of the pinned baselines round-trip through it).
/// Falls back to the related-free summary parse on the rare structural surprise,
/// so the primary channel never shifts.
fn parse_base_diags(content: &str) -> Vec<BaseDiag> {
    use crate::tsc_conformance::baseline::Loc;
    match parse_baseline(content) {
        Ok(parsed) => parsed
            .diags
            .iter()
            .map(|d| {
                let (line, col) = match d.loc {
                    Some(Loc::Numbered { line, col }) => (Some(line), Some(col)),
                    _ => (None, None),
                };
                let related = d.related.iter().filter_map(|s| parse_related_line(s)).collect();
                BaseDiag { file: d.file.clone(), line, col, code: d.code, related }
            })
            .collect(),
        Err(_) => parse_summary_block(content)
            .into_iter()
            .map(|d| BaseDiag {
                file: d.file,
                line: d.line,
                col: d.col,
                code: d.code as i32,
                related: Vec::new(),
            })
            .collect(),
    }
}

/// Parse one `!!! related TS<code> <file>:<line>:<col>: <msg>` line into a
/// [`RelatedKey`], or `None` for a chain-continuation line (no `!!! related`
/// prefix). A `--:--` location parses to [`None`] (a masked default-lib position).
fn parse_related_line(line: &str) -> Option<RelatedKey> {
    let rest = line.strip_prefix("!!! related TS")?;
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    let code: u32 = rest.get(..end)?.parse().ok()?;
    let after = rest.get(end..)?.strip_prefix(' ')?; // `<file>:<line>:<col>: <msg>`
    // The first `": "` separates the location from the message (a filename holds
    // no space, and line/col are digits-or-`--`).
    let boundary = after.find(": ")?;
    let locpart = after.get(..boundary)?; // `<file>:<line>:<col>`
    let (rest2, col) = locpart.rsplit_once(':')?;
    let (file, line_s) = rest2.rsplit_once(':')?;
    let loc = if line_s == "--" && col == "--" {
        None
    } else {
        Some((line_s.parse().ok()?, col.parse().ok()?))
    };
    Some(RelatedKey { code, file: file.to_string(), loc })
}

/// The related-info buckets across a variant's matched primaries.
#[derive(Default)]
struct RelatedBuckets {
    matched: usize,
    extra: usize,
    span_mismatch: usize,
    missing: usize,
}

/// Grade related-info multisets for the primaries that match by
/// `(file,line,col,code)`. Ours and the baseline are grouped by primary key;
/// matched primaries are paired positionally and their related sets diffed
/// (exact `(code,file,loc)` match, masked `--,--` by `(code,file)`, then
/// `(code,file)` span-mismatch pairing of the leftovers).
fn grade_related(ours: &[FamilyEntry], base: &[FamilyEntry]) -> RelatedBuckets {
    let mut ours_by: HashMap<&FamilyDiag, Vec<&[RelatedKey]>> = HashMap::new();
    for e in ours {
        ours_by.entry(&e.key).or_default().push(&e.related);
    }
    let mut base_by: HashMap<&FamilyDiag, Vec<&[RelatedKey]>> = HashMap::new();
    for e in base {
        base_by.entry(&e.key).or_default().push(&e.related);
    }

    let mut out = RelatedBuckets::default();
    for (key, ours_sets) in &ours_by {
        let Some(base_sets) = base_by.get(key) else { continue };
        let paired = ours_sets.len().min(base_sets.len());
        for i in 0..paired {
            related_diff(ours_sets[i], base_sets[i], &mut out);
        }
    }
    out
}

/// Diff one matched primary's related multisets, folding into `out`.
fn related_diff(ours: &[RelatedKey], base: &[RelatedKey], out: &mut RelatedBuckets) {
    // Exact `(code,file,loc)` matches first.
    let mut ours_counts: HashMap<&RelatedKey, usize> = HashMap::new();
    for r in ours {
        *ours_counts.entry(r).or_default() += 1;
    }
    let mut base_counts: HashMap<&RelatedKey, usize> = HashMap::new();
    for r in base {
        *base_counts.entry(r).or_default() += 1;
    }
    // Leftovers grouped by `(code, file)` for masked-match and span-mismatch pairing.
    let mut left_ours: HashMap<(u32, &str), usize> = HashMap::new();
    let mut left_base_located: HashMap<(u32, &str), usize> = HashMap::new();
    let mut left_base_masked: HashMap<(u32, &str), usize> = HashMap::new();

    for (r, &oc) in &ours_counts {
        let bc = base_counts.get(*r).copied().unwrap_or(0);
        let m = oc.min(bc);
        out.matched += m;
        if oc > m {
            *left_ours.entry((r.code, r.file.as_str())).or_default() += oc - m;
        }
    }
    for (r, &bc) in &base_counts {
        let oc = ours_counts.get(*r).copied().unwrap_or(0);
        let m = oc.min(bc);
        if bc > m {
            let bucket = if r.loc.is_none() { &mut left_base_masked } else { &mut left_base_located };
            *bucket.entry((r.code, r.file.as_str())).or_default() += bc - m;
        }
    }

    // Masked baseline related (default-lib `--,--`) matches ours by `(code,file)`.
    for (key, bcount) in &mut left_base_masked {
        if let Some(ocount) = left_ours.get_mut(key) {
            let m = (*ocount).min(*bcount);
            out.matched += m;
            *ocount -= m;
            *bcount -= m;
        }
    }

    // Remaining located leftovers: `(code,file)` pairing = span mismatch; the rest
    // is extra (ours) / missing (baseline).
    let keys: std::collections::HashSet<(u32, &str)> = left_ours
        .keys()
        .chain(left_base_located.keys())
        .chain(left_base_masked.keys())
        .copied()
        .collect();
    for key in keys {
        let oc = left_ours.get(&key).copied().unwrap_or(0);
        let bc = left_base_located.get(&key).copied().unwrap_or(0)
            + left_base_masked.get(&key).copied().unwrap_or(0);
        let sm = oc.min(bc);
        out.span_mismatch += sm;
        out.extra += oc - sm;
        out.missing += bc - sm;
    }
}

/// The four family buckets for one variant.
struct FamilyBuckets {
    matched: usize,
    extra: usize,
    span_mismatch: usize,
    /// The unattributed misses, per code (for cause classification).
    missing_by_code: HashMap<u32, usize>,
}

/// Compare our family multiset against the baseline's: exact `(file,line,col,code)`
/// matches, then greedy `(file,code)` span-mismatch pairing of the leftovers, with
/// the residue split into extra (ours) and missing (baseline).
fn family_buckets(ours: &[FamilyDiag], base: &[FamilyDiag]) -> FamilyBuckets {
    let mut ours_counts: HashMap<&FamilyDiag, usize> = HashMap::new();
    for d in ours {
        *ours_counts.entry(d).or_default() += 1;
    }
    let mut base_counts: HashMap<&FamilyDiag, usize> = HashMap::new();
    for d in base {
        *base_counts.entry(d).or_default() += 1;
    }

    let mut matched = 0usize;
    // Leftover counts grouped by (file, code) for span-mismatch pairing.
    let mut left_ours: HashMap<(&str, u32), usize> = HashMap::new();
    let mut left_base: HashMap<(&str, u32), usize> = HashMap::new();

    for (d, &oc) in &ours_counts {
        let bc = base_counts.get(d).copied().unwrap_or(0);
        let m = oc.min(bc);
        matched += m;
        if oc > m {
            *left_ours.entry((d.file.as_str(), d.code)).or_default() += oc - m;
        }
    }
    for (d, &bc) in &base_counts {
        let oc = ours_counts.get(d).copied().unwrap_or(0);
        let m = oc.min(bc);
        if bc > m {
            *left_base.entry((d.file.as_str(), d.code)).or_default() += bc - m;
        }
    }

    // Pair leftovers within each (file, code) group: min = span mismatch, the
    // ours residue = extra, the baseline residue = missing.
    let mut span_mismatch = 0usize;
    let mut extra = 0usize;
    let mut missing_by_code: HashMap<u32, usize> = HashMap::new();
    let keys: std::collections::HashSet<(&str, u32)> =
        left_ours.keys().chain(left_base.keys()).copied().collect();
    for &(file, code) in &keys {
        let oc = left_ours.get(&(file, code)).copied().unwrap_or(0);
        let bc = left_base.get(&(file, code)).copied().unwrap_or(0);
        let sm = oc.min(bc);
        span_mismatch += sm;
        extra += oc - sm;
        if bc - sm > 0 {
            *missing_by_code.entry(code).or_default() += bc - sm;
        }
    }

    FamilyBuckets { matched, extra, span_mismatch, missing_by_code }
}

/// Classify a parse-rejected variant's baseline shape for the census.
fn baseline_shape(baseline: Option<&Baseline>) -> BaselineShape {
    let Some(baseline) = baseline else {
        return BaselineShape::None;
    };
    let Ok(content) = std::fs::read_to_string(&baseline.path) else {
        return BaselineShape::Other;
    };
    let diags = parse_summary_block(&content);
    if !diags.is_empty() && diags.iter().all(|d| (1000..2000).contains(&d.code)) {
        BaselineShape::Ts1xxxOnly
    } else {
        BaselineShape::Other
    }
}

impl SkeletonReport {
    /// Print the human summary.
    pub fn print(&self) {
        println!("tsc_conformance — walking skeleton");
        println!("==================================");
        println!("In-scope tests:            {}", self.in_scope_tests);
        println!("In-scope variants:         {}", self.in_scope_variants);
        println!("  parsed, expect-clean:    {}", self.expect_clean_graded);
        println!("    graded clean:          {}", self.clean_pass);
        println!("    graded NON-clean:      {}", self.clean_fail.len());
        println!("  parsed, baselined:       {}", self.baselined_parsed);
        println!("  parse-rejected:          {}", self.parse_rejected_total);
        println!("    no baseline:           {}", self.parse_rejected_no_baseline);
        println!("    TS1xxx-only baseline:  {}", self.parse_rejected_ts1xxx_only);
        println!("    other baseline:        {}", self.parse_rejected_other);
        println!("Script-goal retries:       {}", self.script_retry);
        println!("Bound nodes (total):       {}", self.total_nodes);
        println!();
        println!("Family grading (2300/2451/2567/2528 + merge 2397/2649/2664/2671)");
        println!("---------------------------------------------------------------");
        println!("Graded variants:           {}", self.family_graded_variants);
        println!("  ...family-positive:      {}", self.family_positive_variants);
        println!("  match:                   {}", self.family_match);
        println!("  missing:                 {}", self.family_missing);
        println!("    merge-path (S4):       {}", self.missing_merge);
        println!("    lib-conflict (S5):     {}", self.missing_lib);
        println!("    check-time (S3+):      {} (checker-emitted TS2300/2451: duplicate members, type params, computed/private names)", self.missing_other);
        println!("  extra (GATE=0):          {}", self.family_extra);
        println!("  span_mismatch:           {}", self.family_span_mismatch);
        println!("Related-info (matched primaries; own channel, non-gating)");
        println!("  related match:           {}", self.related_match);
        println!("  related missing:         {}", self.related_missing);
        println!("  related extra:           {}", self.related_extra);
        println!("  related span_mismatch:   {}", self.related_span_mismatch);
        for s in &self.related_missing_samples {
            println!("  REL-MISSING {s}");
        }
        for s in &self.related_extra_samples {
            println!("  REL-EXTRA {s}");
        }
        println!("Carve-out rule (a):        {}", self.carve_out_rule_a);
        println!("  ...family-positive:      {}", self.carve_out_rule_a_family);
        println!("moduleDetection variants:  {} (watch; inert for family)", self.module_detection_variants);
        for s in &self.extra_samples {
            println!("  EXTRA {s}");
        }
        for s in &self.missing_other_samples {
            println!("  MISSING-OTHER {s}");
        }
        for s in &self.span_mismatch_samples {
            println!("  SPAN {s}");
        }
        println!();
        println!("Lib base (S5)");
        println!("  lib files bound:         {}", self.lib_files_bound);
        println!("  lib sets folded:         {}", self.lib_sets_built);
        println!("  lib parse errors:        {} (GATE=0)", self.lib_parse_errors.len());
        println!("  lib missing files:       {} (GATE=0)", self.lib_missing_files.len());
        println!("  lib unknown names:       {} (GATE=0)", self.lib_unknown_names.len());
        for e in &self.lib_parse_errors {
            println!("  LIB-PARSE-ERR {e}");
        }
        for f in &self.lib_missing_files {
            println!("  LIB-MISSING {f}");
        }
        for n in &self.lib_unknown_names {
            println!("  LIB-UNKNOWN {n}");
        }
        println!();
        println!("Panics (caught):           {}", self.panics.len());
        println!("Crash-excluded (tracked):  {}", self.excluded_crashes);
        if !self.stale_exclusions.is_empty() {
            println!("Stale crash-exclusions:    {} (drop them)", self.stale_exclusions.len());
        }
        println!("Wall-clock:                {} ms", self.wall_ms);
        if !self.clean_fail.is_empty() {
            println!();
            for f in &self.clean_fail {
                println!("  CLEAN-FAIL {} ({} diagnostics)", f.variant, f.diagnostics);
            }
        }
        for p in &self.panics {
            println!("  PANIC {} — {}", p.test, p.payload);
        }
    }
}

// ===========================================================================
// check-test: the inner dev loop over one test.
// ===========================================================================

/// One diagnostic line (ours or the baseline's) for the check-test diff.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DiagLine {
    /// The file the diagnostic points at (or `null` for a global one). A lib file
    /// (`lib.es5.d.ts`) with `null` line/col is a masked lib-sourced entry.
    pub file: Option<String>,
    /// 1-based line (`null` for a global or masked-lib diagnostic).
    pub line: Option<u32>,
    /// 1-based column (`null` for a global or masked-lib diagnostic).
    pub col: Option<u32>,
    /// The `TS<code>` number.
    pub code: u32,
    /// The diagnostic's related-info entries (empty for a baseline summary line).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<DiagLine>,
}

/// The `check-test` report for one test/variant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckTestReport {
    /// The corpus test's relative path.
    pub test: String,
    /// The suite (`compiler` / `conformance`).
    pub suite: String,
    /// The variant description, or `(default)`.
    pub variant: String,
    /// The joined baseline name, or `None` when the variant is expect-clean.
    pub baseline: Option<String>,
    /// Whether the variant is expect-clean (no on-disk baseline).
    pub expect_clean: bool,
    /// Whether tsv parse-rejected the program.
    pub parse_rejected: bool,
    /// The parse error message, when rejected.
    pub parse_error: Option<String>,
    /// Our diagnostics (empty while the checker is a no-op).
    pub ours: Vec<DiagLine>,
    /// The baseline's summary-block diagnostics (the expected set).
    pub baseline_summary: Vec<DiagLine>,
}

/// Run one corpus test (optionally one variant) and build its check-test report.
///
/// `name` matches a corpus test by exact relative path or exact basename.
///
/// # Errors
///
/// Returns an error string when the test is not found, the match is ambiguous,
/// the requested variant does not exist, or corpus discovery fails.
pub fn check_one(
    checkout: &Path,
    name: &str,
    variant_filter: Option<(String, String)>,
) -> Result<CheckTestReport, String> {
    let corpus = discover_corpus(checkout)?;
    let baselines = discover_baselines(&baselines_dir(checkout))?;

    let matches: Vec<&CorpusTest> = corpus
        .iter()
        .filter(|t| t.relative_path == name || t.basename == name)
        .collect();
    let test = match matches.as_slice() {
        [] => return Err(format!("no corpus test matches {name:?}")),
        [one] => *one,
        many => {
            let paths: Vec<String> =
                many.iter().map(|t| format!("{}/{}", t.suite, t.relative_path)).collect();
            return Err(format!("{name:?} is ambiguous: {}", paths.join(", ")));
        }
    };

    let content = read_corpus_file(&test.path)?;
    let settings = extract_settings(&content);
    let units = split_units(&content, &test.basename);

    // Pick the variant.
    let expansion = expand(&settings);
    let variant = select_variant(&expansion.variants, variant_filter.as_ref())?;
    let baseline_name = config_name(&test.basename, &variant.description);

    // Join the baseline.
    let mut ondisk: HashMap<(&str, String), &Baseline> = HashMap::new();
    for baseline in &baselines {
        if let Some((suite, n)) = baseline.relative_path.split_once('/') {
            ondisk.insert((suite, n.to_string()), baseline);
        }
    }
    let baseline = ondisk.get(&(test.suite, baseline_name.clone())).copied();

    // Parse + bind every unit, then merge against the selected variant's lib base.
    let arena = Bump::new();
    let source_units: Vec<SourceUnit<'_>> =
        units.iter().map(|u| SourceUnit::new(&u.name, &u.content)).collect();
    let bound = bind_program(&source_units, &arena);
    let mut resolver = LibResolver::new(checkout);
    let base = resolver.base_for(&variant.config);
    let lib_files = base.as_ref().map_or(&[][..], |b| b.lib_files.as_slice());
    let result = check_bound(&bound, base.as_deref());

    // Resolve each diagnostic's FileId to a display line: a program unit carries its
    // (line, col); a lib file carries the lib name with a masked location.
    let resolve_line = |d: &Diagnostic| -> DiagLine {
        let units_len = units.len();
        match d.file {
            Some(f) if f.index() < units_len => {
                let (line, col) = units.get(f.index()).map_or((None, None), |u| {
                    let (t, m) = LocationTracker::new_ecmascript_with_map(&u.content);
                    let (_, pos) =
                        LocationMapper { tracker: &t, map: &m }.pos_and_position(d.span.start);
                    (Some(pos.line as u32), Some(pos.column as u32 + 1))
                });
                DiagLine {
                    file: units.get(f.index()).map(|u| u.name.clone()),
                    line,
                    col,
                    code: d.code,
                    related: Vec::new(),
                }
            }
            Some(f) => DiagLine {
                file: lib_files.get(f.index() - units_len).cloned(),
                line: None,
                col: None,
                code: d.code,
                related: Vec::new(),
            },
            None => DiagLine { file: None, line: None, col: None, code: d.code, related: Vec::new() },
        }
    };
    let ours: Vec<DiagLine> = result
        .diagnostics
        .iter()
        .map(|d| {
            let mut line = resolve_line(d);
            line.related = d.related.iter().map(&resolve_line).collect();
            line
        })
        .collect();
    let parse_error = result.files.iter().find_map(|f| match &f.parse {
        ParseReport::Rejected { message } => Some(message.clone()),
        ParseReport::Parsed(_) => None,
    });

    let baseline_summary = match baseline {
        Some(b) => std::fs::read_to_string(&b.path)
            .map(|c| {
                parse_summary_block(&c)
                    .into_iter()
                    .map(|d| DiagLine {
                        file: d.file,
                        line: d.line,
                        col: d.col,
                        code: d.code,
                        related: Vec::new(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    };

    Ok(CheckTestReport {
        test: test.relative_path.clone(),
        suite: test.suite.to_string(),
        variant: if variant.description.is_empty() {
            "(default)".to_string()
        } else {
            variant.description.clone()
        },
        baseline: baseline.map(|_| baseline_name),
        expect_clean: baseline.is_none(),
        parse_rejected: result.parse_rejected,
        parse_error,
        ours,
        baseline_summary,
    })
}

/// Select a variant by an optional `k=v` filter (config match, lowercased key);
/// with no filter the first (usually the unvaried) variant.
fn select_variant<'a>(
    variants: &'a [Variant],
    filter: Option<&(String, String)>,
) -> Result<&'a Variant, String> {
    match filter {
        None => variants.first().ok_or_else(|| "test has no variants".to_string()),
        Some((key, value)) => {
            let key = key.to_lowercase();
            variants
                .iter()
                .find(|v| v.config.get(&key).map(String::as_str) == Some(value.as_str()))
                .ok_or_else(|| {
                    let available: Vec<&str> = variants
                        .iter()
                        .map(|v| {
                            if v.description.is_empty() {
                                "(default)"
                            } else {
                                &v.description
                            }
                        })
                        .collect();
                    format!("no variant with {key}={value}; available: {}", available.join(", "))
                })
        }
    }
}

impl CheckTestReport {
    /// Print the human diff (ours vs the baseline summary).
    pub fn print(&self) {
        println!("check-test: {}/{}  variant={}", self.suite, self.test, self.variant);
        if self.parse_rejected {
            println!(
                "  tsv PARSE-REJECTED: {}",
                self.parse_error.as_deref().unwrap_or("(no message)")
            );
        }
        if self.expect_clean {
            println!("  baseline: (none — expect-clean)");
        } else {
            println!("  baseline: {}", self.baseline.as_deref().unwrap_or("?"));
        }
        println!();
        println!("  ours ({}):", self.ours.len());
        for d in &self.ours {
            println!("    {}", fmt_diag(d));
            for r in &d.related {
                println!("      related {}", fmt_diag(r));
            }
        }
        if self.ours.is_empty() {
            println!("    (none)");
        }
        println!("  baseline ({}):", self.baseline_summary.len());
        for d in &self.baseline_summary {
            println!("    {}", fmt_diag(d));
        }
        if self.baseline_summary.is_empty() {
            println!("    (none)");
        }
    }
}

/// Format one diagnostic line for the human diff.
fn fmt_diag(d: &DiagLine) -> String {
    match (&d.file, d.line, d.col) {
        (Some(file), Some(line), Some(col)) => format!("{file}({line},{col}): TS{}", d.code),
        // A masked lib entry (file, no location) or a global one.
        (Some(file), _, _) => format!("{file}(--,--): TS{}", d.code),
        _ => format!("error TS{} (global)", d.code),
    }
}
