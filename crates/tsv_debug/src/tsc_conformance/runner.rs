//! The conformance runner: drive `tsv_check` over the in-scope corpus and
//! grade it against tsgo's committed `.errors.txt` baselines.
//!
//! The runner layers the checker leg on the corpus substrate — corpus index,
//! directive parser, variant expansion, the unsupported-option skip classes:
//! for every **in-scope** variant (single-file, non-JSX, non-JS-flavored, not
//! skipped, not an unsupported-option variant) it parses the unit via
//! `tsv_check`'s goal rule, binds, merges against the variant's lib base, and
//! grades the result on three channels. Every **expect-clean** in-scope
//! variant (one with no on-disk baseline) must grade clean (zero diagnostics);
//! the graded **family** ([`FAMILY_CODES`] — the bind/merge duplicate-conflict
//! sub-family [`DUP_CODES`] plus the flow-construction sub-family [`FLOW_CODES`],
//! TS7027/TS7028) is compared as codes+spans multisets — extra = 0 is the hard
//! gate, missing is classified by deferred cause (merge / lib / late-bound / cfa,
//! else the hard-zero `other`); and **related-info** on matched family primaries
//! is graded as its own channel. Zero panics, always.
//!
//! A single-file test's variants all parse identically (the goal rule is
//! directive-independent), so parse+bind runs **once per test**
//! (`bind_program`) and merge+check runs once per distinct lib set among its
//! variants (`check_bound`), with the outcome attributed to each in-scope
//! variant.
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
//! be caught; the [`CRASH_EXCLUSIONS`] list carves out crashers by kind — the
//! genuine-abort class is empty on the pinned corpus (every current entry is a
//! catchable panic tracking a tsv parser bug, liveness-probed each run).
//
// tsgo: internal/compiler/program.go GetDiagnosticsOfAnyProgram (the pipeline)
// tsgo: internal/testrunner/compiler_runner.go (the in-scope selection)

use crate::tsc_conformance::baseline::{parse_baseline, parse_summary_block};
use crate::tsc_conformance::corpus::{CorpusTest, discover_corpus, read_corpus_file};
use crate::tsc_conformance::directives::{Unit, extract_settings, split_units};
use crate::tsc_conformance::discovery::{Baseline, baselines_dir, discover_baselines};
use crate::tsc_conformance::index::{is_js_flavored, is_jsx_scoped};
use crate::tsc_conformance::libs::LibResolver;
use crate::tsc_conformance::options_meta::{
    SKIPPED_TESTS, is_config_file_name, variant_is_unsupported,
};
use crate::tsc_conformance::variants::{Variant, config_name, expand};
use bumpalo::Bump;
use std::collections::{BTreeMap, HashMap};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::Instant;
use tsv_check::{
    CheckOptions, Diagnostic, FileId, ParseReport, SourceUnit, bind_file, bind_program, build_flow,
    check_bound, check_program, render_flow_dot,
};
use tsv_lang::{LocationMapper, LocationTracker};

/// The full set of codes the gate grades — the bind/merge duplicate-conflict
/// family ([`DUP_CODES`]) plus the flow-construction family ([`FLOW_CODES`]).
/// Duplicate-conflict: TS2300 (duplicate identifier), TS2451 (block-scoped
/// redeclare), TS2567 (enum-merge), TS2528 (multiple default exports), plus the
/// merge-path codes TS2397/2649/2664/2671 (emitted from the globals-merge phase
/// rather than the same-table cascade). Flow: TS7027 (unreachable code), TS7028
/// (unused label), emitted from the post-bind flow-construction shim.
const FAMILY_CODES: [u32; 10] = [2300, 2451, 2567, 2528, 2397, 2649, 2664, 2671, 7027, 7028];

/// The duplicate/conflict sub-family (bind + merge + the check-time TS2300
/// members/type-parameters pass) — the partition used for the `--family dup`
/// filter and the sub-family report lines.
const DUP_CODES: [u32; 8] = [2300, 2451, 2567, 2528, 2397, 2649, 2664, 2671];

/// The flow-construction sub-family (TS7027 unreachable code / TS7028 unused
/// label) — the partition used for the `--family flow` filter and the sub-family
/// report lines.
const FLOW_CODES: [u32; 2] = [7027, 7028];

/// One graded code family: its `--family` filter token (also its report label)
/// and its code set. **Adding a family** = a codes const + a row here + a row in
/// the CLI command's per-family pin table (+ a [`MissingCause`] variant and
/// ledger if it brings a new deferred cause) — the sweep, filter parsing,
/// sub-family accessors, and report lines all read this table.
pub struct GradedFamily {
    /// The `--family` filter token / report label.
    pub key: &'static str,
    /// The family's TS codes.
    pub codes: &'static [u32],
}

/// The graded families, in report order.
pub const FAMILIES: [GradedFamily; 2] = [
    GradedFamily {
        key: "dup",
        codes: &DUP_CODES,
    },
    GradedFamily {
        key: "flow",
        codes: &FLOW_CODES,
    },
];

// `FAMILY_CODES` is maintained by hand as the union of every `FAMILIES` row —
// pin the agreement at compile time (order-preserving concatenation, dup then
// flow), so a family edit that forgets one side cannot build.
const _: () = {
    assert!(FAMILY_CODES.len() == DUP_CODES.len() + FLOW_CODES.len());
    let mut i = 0;
    while i < DUP_CODES.len() {
        assert!(FAMILY_CODES[i] == DUP_CODES[i]);
        i += 1;
    }
    let mut j = 0;
    while j < FLOW_CODES.len() {
        assert!(FAMILY_CODES[DUP_CODES.len() + j] == FLOW_CODES[j]);
        j += 1;
    }
};

/// The merge-path family codes — a *missing* of one of these is classified as a
/// merge-phase gap, not a same-table cascade bug.
const MERGE_CODES: [u32; 4] = [2397, 2649, 2664, 2671];

/// The TS1xxx codes the binder itself emits (strict-mode + private-identifier
/// checks) — they prove nothing about parse state, so a baseline carrying only
/// these does not trigger the recovery-AST carve-out (predicate v1, rule a).
const BIND_EMITTED_TS1XXX: [u32; 12] = [
    1100, 1101, 1102, 1210, 1212, 1213, 1214, 1215, 1262, 1344, 1359, 18012,
];

/// The family baselines whose family diagnostics come from a standard-library
/// conflict. These **match** via the lib base; the classifier is kept as a
/// regression guard — a *missing* in one of these is bucketed to
/// [`MissingCause::Lib`] (pinned 0) rather than the hard-zero
/// [`MissingCause::Other`], so a lib-detection regression fails loudly.
const LIB_CONFLICT_BASELINES: [&str; 5] = [
    "intersectionsOfLargeUnions2.ts",
    "jsExportMemberMergedWithModuleAugmentation2.ts",
    "promiseDefinitionTest.ts",
    "recursiveComplicatedClasses.ts",
    "variableDeclarationInStrictMode1.ts",
];

/// The family baselines whose remaining missing family diagnostics are genuinely
/// deferred: they need the type engine (a literal-type or `unique symbol` computed
/// member name, resolved via tsgo's `lateBindMember`) that this bind+check
/// implementation has no counterpart for. A *missing* in one of these is bucketed to
/// [`MissingCause::DeferredLateBound`] (exact-pinned) rather than the hard-zero
/// [`MissingCause::Other`], so the honest residual stays visible without gating a
/// release on a type-engine gap. Basenames, mirroring `LIB_CONFLICT_BASELINES`.
const LATE_BOUND_BASELINES: [&str; 4] = [
    "dynamicNamesErrors.ts",
    "symbolDeclarationEmit12.ts",
    "symbolProperty37.ts",
    "symbolProperty44.ts",
];

/// The flow-family baselines whose remaining missing TS7027 diagnostics are
/// genuinely deferred to the CFA type engine: the construction-only fast-path
/// shim (a strict subset of what tsgo reports) can't resolve them because they
/// come from tsgo's `isReachableFlowNode` fallback — never-returning call
/// signatures, `asserts` type predicates, switch exhaustiveness, and the
/// structural reachability fallback. A *missing* in one of these is bucketed to
/// [`MissingCause::DeferredCfa`] (exact-pinned) rather than the hard-zero
/// [`MissingCause::Other`], so the honest CFA residual stays visible without gating
/// a release on a type-engine gap. Basenames, mirroring `LATE_BOUND_BASELINES`.
///
/// `assertionTypePredicates1.ts` never reaches the cfa bucket — its entry is a
/// defensive no-op kept for completeness. tsv currently **parse-rejects** it (an
/// over-rejection of the setter assertion-predicate form `set p(x: this is T)`,
/// counted in the parse-divergence census), so it is graded not at all; and were
/// the parser fixed, its baseline's non-bind TS1xxx (TS1228) would carve it out by
/// predicate v1 rule (a) instead — either way its 5 flow instances never land in
/// the cfa bucket. The live cfa residual is **26**: neverReturningFunctions1
/// 22, exhaustiveSwitchStatements1 1, unreachableSwitchTypeofAny 1,
/// unreachableSwitchTypeofUnknown 1 (**25** across the four non-carved named
/// baselines), plus reachabilityChecks8 1 (the structural `isReachableFlowNode`
/// fallback tsv's faithful for-construction correctly doesn't emit).
const DEFERRED_CFA_BASELINES: [&str; 6] = [
    "neverReturningFunctions1.ts",
    "assertionTypePredicates1.ts",
    "exhaustiveSwitchStatements1.ts",
    "unreachableSwitchTypeofAny.ts",
    "unreachableSwitchTypeofUnknown.ts",
    "reachabilityChecks8.ts",
];

/// Why a graded family baseline diagnostic is missing — the classifier's
/// verdict, tallied keyed in `SkeletonReport::missing_by_cause`. Every non-
/// [`MissingCause::Other`] cause is exact-pinned in the CLI command's cause-pin
/// table; `Other` is the HARD-zero invariant (enforced even on filtered runs).
/// **Adding a deferred cause** (a new family's type-engine residual) = a variant
/// here + a ledger const + an arm in [`classify_missing`] + a pin-table row.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingCause {
    /// The merge phase owns the code ([`MERGE_CODES`]).
    Merge,
    /// A [`LIB_CONFLICT_BASELINES`] test missing a dup-family code (absent lib
    /// binding — a lib-detection regression guard, pinned 0).
    Lib,
    /// A [`LATE_BOUND_BASELINES`] test missing a dup-family code — the
    /// type-engine `lateBindMember` residual (exact-pinned).
    DeferredLateBound,
    /// A [`DEFERRED_CFA_BASELINES`] test missing a flow-family code — the
    /// `isReachableFlowNode` residual (exact-pinned).
    DeferredCfa,
    /// Unclassified — a same-table cascade / flow-construction bug. **HARD
    /// gate: zero**, an invariant even on filtered triage runs.
    Other,
}

/// Classify one missing family diagnostic by its baseline + code. Each
/// name-keyed cause also requires the code to be in its family (dup for
/// lib/late-bound, flow for cfa), mirroring the merge branch — a wrong-family
/// missing inside a named baseline falls through to the hard-zero `Other`
/// instead of being silently absorbed. This keeps the classification honest on
/// filtered/triage runs, not only on a full run where the code-keyed pins
/// catch it.
fn classify_missing(basename: &str, code: u32) -> MissingCause {
    if MERGE_CODES.contains(&code) {
        MissingCause::Merge
    } else if LIB_CONFLICT_BASELINES.contains(&basename) && DUP_CODES.contains(&code) {
        MissingCause::Lib
    } else if LATE_BOUND_BASELINES.contains(&basename) && DUP_CODES.contains(&code) {
        MissingCause::DeferredLateBound
    } else if DEFERRED_CFA_BASELINES.contains(&basename) && FLOW_CODES.contains(&code) {
        MissingCause::DeferredCfa
    } else {
        MissingCause::Other
    }
}

/// Worker-thread stack for the sweep: the corpus has deeply-nested tests and
/// tsv's recursive-descent parser has no depth guard, so the default 8 MiB
/// overflows. 512 MiB is virtual-only reserve on Linux.
const SKELETON_STACK: usize = 512 * 1024 * 1024;

/// Per-test wall-clock budget for the [`TestWatchdog`]. The full ~12k-test
/// sweep runs in seconds, so a single test at 60 s is pathological with huge
/// margin (~10⁴× the mean) — the limit exists to convert a *hang* into a loud
/// named failure, not to police slow tests.
const WATCHDOG_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// The sweep's hang watchdog — the wall-clock half of the "watchdog
/// independent of ported budgets" requirement. `catch_unwind` converts panics
/// into per-test buckets, but a **hang** (a mis-ported budget at P3, a parser
/// loop) would otherwise freeze the gate silently. The worker heartbeats each
/// test's name + start; a monitor thread checks ~1 Hz and, past
/// [`WATCHDOG_LIMIT`], prints the offending test and exits the process (a hung
/// thread cannot be killed safely — a loud named exit is the correct failure).
/// The instruction-count half (budget-arithmetic cross-check) rides P3 with
/// the budgets themselves.
struct TestWatchdog {
    /// `(current test relative_path, its start)`; `None` after `finish`.
    current: std::sync::Arc<std::sync::Mutex<Option<(String, Instant)>>>,
}

impl TestWatchdog {
    fn spawn() -> TestWatchdog {
        let current: std::sync::Arc<std::sync::Mutex<Option<(String, Instant)>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let monitor = std::sync::Arc::clone(&current);
        // Detached monitor: exits within a tick of the sweep clearing the slot
        // (or dies with the process — it holds nothing that needs cleanup).
        drop(
            std::thread::Builder::new()
                .name("tsc-watchdog".to_string())
                .spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        let guard = monitor.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                        let Some((test, start)) = guard.as_ref() else {
                            return; // sweep finished
                        };
                        if start.elapsed() > WATCHDOG_LIMIT {
                            eprintln!(
                                "tsc_conformance watchdog: test {test:?} exceeded {}s — a hang \
                                 (mis-ported budget / parser loop); aborting the run",
                                WATCHDOG_LIMIT.as_secs()
                            );
                            std::process::exit(3);
                        }
                    }
                }),
        );
        TestWatchdog { current }
    }

    /// Heartbeat: the sweep is entering `test` now.
    fn enter(&self, test: &str) {
        *self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((test.to_string(), Instant::now()));
    }

    /// The sweep is done — clear the slot so the monitor thread exits.
    fn finish(&self) {
        *self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

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
    (
        "exportDeclarationInInternalModule.ts",
        CrashKind::CatchablePanic,
    ),
];

/// The [`CrashKind`] of a crash-excluded test, or `None` if not excluded.
fn crash_exclusion_kind(basename: &str) -> Option<CrashKind> {
    CRASH_EXCLUSIONS
        .iter()
        .find(|(n, _)| *n == basename)
        .map(|(_, k)| *k)
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

/// Which graded sub-family the `--family` filter isolates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FamilyFilter {
    /// One [`FAMILIES`] row, by index (`dup` = 0, `flow` = 1).
    One(usize),
    /// The whole graded family ([`FAMILY_CODES`]) — isolates the family-graded slice.
    All,
}

impl FamilyFilter {
    /// Parse a `--family` token: a [`FAMILIES`] key, or `all`.
    #[must_use]
    pub fn parse(arg: &str) -> Option<FamilyFilter> {
        if arg == "all" {
            return Some(FamilyFilter::All);
        }
        FAMILIES
            .iter()
            .position(|f| f.key == arg)
            .map(FamilyFilter::One)
    }

    /// The valid `--family` tokens, for error messages (`dup / flow / all`).
    #[must_use]
    pub fn tokens() -> String {
        let mut tokens: Vec<&str> = FAMILIES.iter().map(|f| f.key).collect();
        tokens.push("all");
        tokens.join(" / ")
    }
}

/// The code set a [`FamilyFilter`] keeps a variant for (its baseline must carry at
/// least one).
fn family_filter_codes(f: FamilyFilter) -> &'static [u32] {
    match f {
        FamilyFilter::One(i) => FAMILIES[i].codes,
        FamilyFilter::All => &FAMILY_CODES,
    }
}

/// Filters for a scoped `run` sweep. Any active filter SKIPS the exact pins (the
/// `roundtrip`/`query` convention), so a filtered run is a triage view — the
/// invariant gates (clean grading, no panics, `family_extra == 0`) still hold.
#[derive(Default, Clone)]
pub struct RunFilter {
    /// Keep only tests whose relative path contains this substring.
    pub test: Option<String>,
    /// Keep only variants whose joined baseline carries this TS code.
    pub code: Option<u32>,
    /// Keep only variants whose config has this `key=value` (key lowercased).
    pub variant: Option<(String, String)>,
    /// Keep only variants whose baseline carries a code in this sub-family
    /// (`dup` / `flow` / `all`).
    pub family: Option<FamilyFilter>,
}

impl RunFilter {
    /// Whether any filter is active (drives pin skipping).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.test.is_some()
            || self.code.is_some()
            || self.variant.is_some()
            || self.family.is_some()
    }

    /// Whether a test passes the `--test` substring filter (absent filter ⇒ keep).
    fn keeps_test(&self, relative_path: &str) -> bool {
        self.test
            .as_deref()
            .is_none_or(|sub| relative_path.contains(sub))
    }

    /// Whether a variant passes the `--variant key=value` filter (absent ⇒ keep).
    /// The key is already lowercased (the config maps store lowercased keys).
    fn keeps_variant(&self, config: &BTreeMap<String, String>) -> bool {
        self.variant
            .as_ref()
            .is_none_or(|(k, v)| config.get(k).map(String::as_str) == Some(v.as_str()))
    }

    /// Whether a variant passes the `--code` filter. `baseline_carries` reports
    /// whether the variant's baseline carries a given code; it is consulted only
    /// when the filter is active, so a run without `--code` never reads a baseline
    /// on its behalf. Absent filter ⇒ keep.
    fn keeps_code(&self, baseline_carries: impl FnOnce(u32) -> bool) -> bool {
        self.code.is_none_or(baseline_carries)
    }

    /// Whether a variant passes the `--family` filter: its baseline must carry at
    /// least one code in the selected sub-family (an expect-clean variant carries
    /// none, so it is filtered out). `baseline_carries` is consulted only when the
    /// filter is active. Absent filter ⇒ keep.
    fn keeps_family(&self, baseline_carries: impl Fn(u32) -> bool) -> bool {
        self.family.is_none_or(|f| {
            family_filter_codes(f)
                .iter()
                .any(|&code| baseline_carries(code))
        })
    }
}

/// Options for the skeleton sweep.
#[derive(Default, Clone)]
pub struct RunOptions {
    /// The triage filter (empty = full pinned run).
    pub filter: RunFilter,
    /// Collect the per-variant verdict rows (for `--emit-manifest`).
    pub collect_manifest: bool,
}

/// One graded variant's verdict for the `--emit-manifest` JSON (the per-variant
/// row — the test262-manifest analog). Collected only when a manifest is requested.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManifestEntry {
    /// The suite (`compiler` / `conformance`).
    pub suite: String,
    /// The corpus test's relative path.
    pub test: String,
    /// The joined baseline name (the variant identity).
    pub config: String,
    /// Whether the variant has an on-disk baseline.
    pub baselined: bool,
    /// Whether tsv parsed the unit (`false` = parse-rejected).
    pub parsed: bool,
    /// The per-variant verdict (see [`grade_test`] / [`grade_family`]).
    pub verdict: &'static str,
}

/// One failing variant with a pre-rendered ours-vs-baseline diff — written to a
/// `.diff` artifact when a run's gates fail (a regression aid; empty when green).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FailingVariant {
    /// The suite (`compiler` / `conformance`).
    pub suite: String,
    /// The joined baseline name (the artifact basename).
    pub config: String,
    /// Why it failed — the same vocabulary as the per-variant verdict
    /// (`family_extra` / `family_span_mismatch` / `clean_fail` / `panic`).
    pub reason: &'static str,
    /// The rendered ours-vs-baseline text (file-artifact only — not in `--json`).
    #[serde(skip)]
    pub diff: String,
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

    // --- family grading ---
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

    /// ...missing, tallied by classified cause (see [`MissingCause`]; keyed so
    /// new causes are a variant + a pin row, not a new field). Read via
    /// [`SkeletonReport::missing`]. [`MissingCause::Other`] **gates at 0**.
    pub missing_by_cause: BTreeMap<MissingCause, usize>,
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

    // --- lib base ---
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
    /// Lib files that bound as an external module with no `declare global {}` block —
    /// their globals would silently fold to nothing. **Gate: must be empty.**
    pub lib_external_no_globals: Vec<String>,
    /// Catchable-panic exclusions that no longer panic (a fix landed) — the entry
    /// is stale and must be dropped. **Gate: must be empty.**
    pub stale_exclusions: Vec<String>,
    /// Total bound nodes across in-scope tests (informational).
    pub total_nodes: u64,
    /// Wall-clock of the sweep in milliseconds (EXCLUDED from the committed report —
    /// machine-varying).
    pub wall_ms: u128,

    // --- deterministic per-code breakdown (the committed report's per-code table) ---
    /// Family diagnostics that matched, keyed by TS code (sorted for determinism).
    pub family_match_by_code: BTreeMap<u32, usize>,
    /// Family baseline diagnostics with no match, keyed by TS code (sorted).
    pub family_missing_by_code: BTreeMap<u32, usize>,

    // --- optional artifacts (empty on a normal green run) ---
    /// Per-variant verdict rows for `--emit-manifest` (empty unless requested).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifest_entries: Vec<ManifestEntry>,
    /// Failing variants with a pre-rendered diff — written to `.diff` artifacts when
    /// the gates fail (empty when green).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failing_variants: Vec<FailingVariant>,
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
pub fn run_skeleton(checkout: &Path, options: &RunOptions) -> Result<SkeletonReport, String> {
    let checkout = checkout.to_path_buf();
    let options = options.clone();
    let handle = std::thread::Builder::new()
        .stack_size(SKELETON_STACK)
        .name("tsc-skeleton".to_string())
        .spawn(move || run_skeleton_inner(&checkout, &options))
        .map_err(|e| format!("spawn skeleton worker: {e}"))?;
    handle
        .join()
        .map_err(|_| "skeleton worker panicked".to_string())?
}

fn run_skeleton_inner(checkout: &Path, options: &RunOptions) -> Result<SkeletonReport, String> {
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
    let watchdog = TestWatchdog::spawn();

    for test in &corpus {
        // Test-level triage filter (`--test <substr>`): match the roundtrip identity.
        if !options.filter.keeps_test(&test.relative_path) {
            continue;
        }
        watchdog.enter(&test.relative_path);
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
        let in_scope: Vec<&Variant> = expansion
            .variants
            .iter()
            .filter(|v| !variant_is_unsupported(&v.config))
            .collect();
        if in_scope.is_empty() {
            continue;
        }

        report.in_scope_tests += 1;
        grade_test(
            test,
            &units[0],
            &in_scope,
            &ondisk,
            &mut resolver,
            options,
            &mut report,
        );
    }
    watchdog.finish();

    // Fold in the resolver's lib-base census (parse-once/fold-once counts + gates).
    report.lib_files_bound = resolver.files_bound();
    report.lib_sets_built = resolver.sets_built();
    report.lib_parse_errors = {
        let mut errors: Vec<String> = resolver
            .parse_errors()
            .iter()
            .map(|(f, e)| format!("{f}: {e}"))
            .collect();
        errors.sort_unstable();
        errors
    };
    report.lib_missing_files = {
        let mut files: Vec<String> = resolver.missing_files().to_vec();
        files.sort_unstable();
        files
    };
    report.lib_unknown_names = {
        let mut names: Vec<String> = resolver.unknown_libs().to_vec();
        names.sort_unstable();
        names.dedup();
        names
    };
    report.lib_external_no_globals = resolver.external_no_globals();

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
    let source_units: Vec<SourceUnit<'_>> = units
        .iter()
        .map(|u| SourceUnit::new(&u.name, &u.content))
        .collect();
    // Silence the default panic hook for the deliberate probe (we expect it to
    // panic; the message would otherwise leak to stderr and read as a failure).
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        let _ = check_program(&source_units, &arena, &CheckOptions::default());
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
/// Build `tsv_check`'s options from a variant's resolved directive config, mapping
/// the harness tri-state to the checker's. `preserveConstEnums` feeds
/// `ShouldPreserveConstEnums` (the `isolatedModules` contribution is not modeled —
/// a rare-in-dead-code residual that can only under-report).
fn check_options_for(config: &BTreeMap<String, String>) -> CheckOptions {
    use crate::tsc_conformance::options_meta::{Tristate as OptTri, resolve_bool};
    let map = |t: OptTri| match t {
        OptTri::Unset => tsv_check::Tristate::Unknown,
        OptTri::False => tsv_check::Tristate::False,
        OptTri::True => tsv_check::Tristate::True,
    };
    CheckOptions {
        allow_unreachable_code: map(resolve_bool(config, "allowunreachablecode")),
        allow_unused_labels: map(resolve_bool(config, "allowunusedlabels")),
        preserve_const_enums: resolve_bool(config, "preserveconstenums") == OptTri::True,
    }
}

fn grade_test(
    test: &CorpusTest,
    unit: &Unit,
    in_scope: &[&Variant],
    ondisk: &HashMap<(&str, String), &Baseline>,
    resolver: &mut LibResolver,
    options: &RunOptions,
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
    let Some(&(_, parse)) = reports.first() else {
        return;
    };
    let parsed = matches!(parse, ParseReport::Parsed(_));

    // The unit's line map — reused across the test's variants for the parsed case.
    let line_map = parsed.then(|| LocationTracker::new_ecmascript_with_map(&unit.content));

    for variant in in_scope {
        let name = config_name(&test.basename, &variant.description);
        let baseline = ondisk.get(&(test.suite, name.clone())).copied();

        // Variant-level triage filters, applied BEFORE counting so a filtered sweep's
        // denominators reflect only the graded slice (any active filter skips the pins).
        if !options.filter.keeps_variant(&variant.config) {
            continue;
        }
        if !options
            .filter
            .keeps_code(|code| baseline.is_some_and(|b| baseline_carries_code(b, code)))
        {
            continue;
        }
        if !options
            .filter
            .keeps_family(|code| baseline.is_some_and(|b| baseline_carries_code(b, code)))
        {
            continue;
        }

        report.in_scope_variants += 1;
        if variant.config.contains_key("moduledetection") {
            report.module_detection_variants += 1;
        }

        let verdict: &'static str = match parse {
            ParseReport::Rejected { .. } => {
                report.parse_rejected_total += 1;
                match baseline_shape(baseline) {
                    BaselineShape::None => report.parse_rejected_no_baseline += 1,
                    BaselineShape::Ts1xxxOnly => report.parse_rejected_ts1xxx_only += 1,
                    BaselineShape::Other => report.parse_rejected_other += 1,
                }
                "parse_rejected"
            }
            ParseReport::Parsed(facts) => {
                if facts.used_script_retry {
                    report.script_retry += 1;
                }
                // Resolve this variant's lib set (cached) and merge the bound program
                // against it — the merge diagnostics are the lib-conflict family. The
                // lib resolution (parse+bind of each `.d.ts`) is the only remaining
                // panic source past the initial bind, so contain it per variant: a
                // future lib parse panic is recorded, not sweep-fatal.
                let check_opts = check_options_for(&variant.config);
                let checked = catch_unwind(AssertUnwindSafe(|| {
                    let base = resolver.base_for(&variant.config);
                    let result = check_bound(&bound, base.as_deref(), &check_opts);
                    (base, result)
                }));
                let (base, result) = match checked {
                    Ok(pair) => pair,
                    Err(payload) => {
                        report.panics.push(PanicRecord {
                            test: test.relative_path.clone(),
                            payload: panic_payload_message(&*payload),
                        });
                        report.failing_variants.push(FailingVariant {
                            suite: test.suite.to_string(),
                            config: name.clone(),
                            reason: "panic",
                            diff: format!("# {}/{name}  (lib-resolution panic)\n", test.suite),
                        });
                        record_manifest(
                            report,
                            options,
                            test,
                            &name,
                            baseline.is_some(),
                            true,
                            "panic",
                        );
                        continue;
                    }
                };
                let lib_files = base.as_ref().map_or(&[][..], |b| b.lib_files.as_slice());

                match baseline {
                    None => {
                        report.expect_clean_graded += 1;
                        if result.diagnostics.is_empty() {
                            report.clean_pass += 1;
                            "clean_pass"
                        } else {
                            report.clean_fail.push(CleanFail {
                                variant: format!("{}/{name}", test.suite),
                                diagnostics: result.diagnostics.len(),
                            });
                            report.failing_variants.push(FailingVariant {
                                suite: test.suite.to_string(),
                                config: name.clone(),
                                reason: "clean_fail",
                                diff: render_clean_fail_diff(test, &name, &result.diagnostics),
                            });
                            "clean_fail"
                        }
                    }
                    Some(b) => {
                        report.baselined_parsed += 1;
                        // `parsed` => `line_map` is `Some`; the `None` arm is dead.
                        let ours_family = match line_map.as_ref() {
                            Some((tracker, map)) => {
                                let mapper = LocationMapper { tracker, map };
                                build_ours_family(
                                    &result.diagnostics,
                                    &unit.name,
                                    &mapper,
                                    lib_files,
                                )
                            }
                            None => Vec::new(),
                        };
                        grade_family(test, &name, b, &ours_family, report)
                    }
                }
            }
        };

        record_manifest(
            report,
            options,
            test,
            &name,
            baseline.is_some(),
            parsed,
            verdict,
        );
    }

    // Node total: counted once per test (all variants share the parse+bind).
    report.total_nodes += bound.total_node_count();
}

/// Record one per-variant verdict row for `--emit-manifest` (a no-op unless a
/// manifest is being collected).
fn record_manifest(
    report: &mut SkeletonReport,
    options: &RunOptions,
    test: &CorpusTest,
    config: &str,
    baselined: bool,
    parsed: bool,
    verdict: &'static str,
) {
    if options.collect_manifest {
        report.manifest_entries.push(ManifestEntry {
            suite: test.suite.to_string(),
            test: test.relative_path.clone(),
            config: config.to_string(),
            baselined,
            parsed,
            verdict,
        });
    }
}

/// Whether a baseline carries a given TS code (the `--code` / `--family` filters).
/// Uses the category-generic [`parse_base_diags`] (not the error-only
/// `parse_summary_block`), so a `--code 7027` / `--family flow` triage matches
/// suggestion- and message-category flow lines too, not just error-category ones.
fn baseline_carries_code(baseline: &Baseline, code: u32) -> bool {
    let Ok(content) = std::fs::read_to_string(&baseline.path) else {
        return false;
    };
    let want = i32::try_from(code).unwrap_or(-1);
    parse_base_diags(&content).iter().any(|d| d.code == want)
}

/// Render one failing family variant's ours-vs-baseline diff for a `.diff` artifact.
fn render_family_diff(
    test: &CorpusTest,
    name: &str,
    reason: &str,
    ours: &[FamilyEntry],
    base: &[FamilyEntry],
) -> String {
    use std::fmt::Write as _;
    let mut s = format!("# {}/{name}  ({reason})\n", test.suite);
    let _ = writeln!(s, "## ours family ({})", ours.len());
    for e in ours {
        let _ = writeln!(
            s,
            "  {}({},{}): TS{}",
            e.key.file, e.key.line, e.key.col, e.key.code
        );
    }
    let _ = writeln!(s, "## baseline family ({})", base.len());
    for e in base {
        let _ = writeln!(
            s,
            "  {}({},{}): TS{}",
            e.key.file, e.key.line, e.key.col, e.key.code
        );
    }
    s
}

/// Render an expect-clean variant's spurious diagnostics for a `.diff` artifact.
fn render_clean_fail_diff(test: &CorpusTest, name: &str, diags: &[Diagnostic]) -> String {
    use std::fmt::Write as _;
    let mut s = format!(
        "# {}/{name}  (clean_fail — expect-clean but {} diagnostic(s))\n## ours ({})\n",
        test.suite,
        diags.len(),
        diags.len(),
    );
    for d in diags {
        let _ = writeln!(s, "  TS{} @ [{}..{}]", d.code, d.span.start, d.span.end);
    }
    s
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
        None => RelatedKey {
            code: r.code,
            file: unit_name.to_string(),
            loc: None,
        },
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
/// baseline, folding the buckets into `report` and returning the per-variant
/// verdict (for the `--emit-manifest` row). Applies predicate v1 rule (a)
/// (recovery-AST carve-out) first, then the primary-code channel and — for the
/// matched primaries — the independent related-info channel.
fn grade_family(
    test: &CorpusTest,
    name: &str,
    baseline: &Baseline,
    ours: &[FamilyEntry],
    report: &mut SkeletonReport,
) -> &'static str {
    let Ok(content) = std::fs::read_to_string(&baseline.path) else {
        return "baseline_unreadable";
    };
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
                key: FamilyDiag {
                    file: d.file.clone()?,
                    line: d.line?,
                    col: d.col?,
                    code,
                },
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
        return "carve_out";
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
    for (code, count) in &buckets.matched_by_code {
        *report.family_match_by_code.entry(*code).or_default() += *count;
    }
    if buckets.extra > 0 && report.extra_samples.len() < 20 {
        report
            .extra_samples
            .push(format!("{}/{name} (+{})", test.suite, buckets.extra));
    }
    if buckets.span_mismatch > 0 && report.span_mismatch_samples.len() < 20 {
        report.span_mismatch_samples.push(format!(
            "{}/{name} (~{})",
            test.suite, buckets.span_mismatch
        ));
    }
    // An unexplained hard-fail bucket (extra / span mismatch) gets a rendered
    // ours-vs-baseline diff artifact; the pinned deferred-late-bound `missing` is
    // expected, so it does not.
    if buckets.extra > 0 || buckets.span_mismatch > 0 {
        let reason = if buckets.extra > 0 {
            "family_extra"
        } else {
            "family_span_mismatch"
        };
        report.failing_variants.push(FailingVariant {
            suite: test.suite.to_string(),
            config: name.to_string(),
            reason,
            diff: render_family_diff(test, name, reason, ours, &base_family),
        });
    }
    for (code, count) in &buckets.missing_by_code {
        report.family_missing += *count;
        *report.family_missing_by_code.entry(*code).or_default() += *count;
        let cause = classify_missing(&test.basename, *code);
        *report.missing_by_cause.entry(cause).or_default() += *count;
        if cause == MissingCause::Other && report.missing_other_samples.len() < 20 {
            report
                .missing_other_samples
                .push(format!("{}/{name} TS{code} x{count}", test.suite));
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
        report
            .related_extra_samples
            .push(format!("{}/{name} (+{})", test.suite, rel.extra));
    }
    if rel.missing > 0 && report.related_missing_samples.len() < 20 {
        report
            .related_missing_samples
            .push(format!("{}/{name} (-{})", test.suite, rel.missing));
    }

    // The per-variant verdict (extra dominates — it is the hard gate).
    if buckets.extra > 0 {
        "family_extra"
    } else if buckets.span_mismatch > 0 {
        "family_span_mismatch"
    } else if !buckets.missing_by_code.is_empty() {
        "family_missing"
    } else if has_family {
        "family_match"
    } else {
        "baselined_clean"
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
                let related = d
                    .related
                    .iter()
                    .filter_map(|s| parse_related_line(s))
                    .collect();
                BaseDiag {
                    file: d.file.clone(),
                    line,
                    col,
                    code: d.code,
                    related,
                }
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
    Some(RelatedKey {
        code,
        file: file.to_string(),
        loc,
    })
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
        let Some(base_sets) = base_by.get(key) else {
            continue;
        };
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
            let bucket = if r.loc.is_none() {
                &mut left_base_masked
            } else {
                &mut left_base_located
            };
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
    /// The exact matches, per code (for the committed report's per-code table).
    matched_by_code: HashMap<u32, usize>,
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
    let mut matched_by_code: HashMap<u32, usize> = HashMap::new();
    // Leftover counts grouped by (file, code) for span-mismatch pairing.
    let mut left_ours: HashMap<(&str, u32), usize> = HashMap::new();
    let mut left_base: HashMap<(&str, u32), usize> = HashMap::new();

    for (d, &oc) in &ours_counts {
        let bc = base_counts.get(d).copied().unwrap_or(0);
        let m = oc.min(bc);
        matched += m;
        if m > 0 {
            *matched_by_code.entry(d.code).or_default() += m;
        }
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

    FamilyBuckets {
        matched,
        extra,
        span_mismatch,
        matched_by_code,
        missing_by_code,
    }
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

/// Sum a per-code map's entries whose code is in `codes` (the sub-family partition
/// behind `dup_*` / `flow_*`).
fn sub_family_sum(by_code: &BTreeMap<u32, usize>, codes: &[u32]) -> usize {
    by_code
        .iter()
        .filter(|(code, _)| codes.contains(code))
        .map(|(_, count)| *count)
        .sum()
}

impl SkeletonReport {
    /// A family's matches (partition of `family_match_by_code` by its code set).
    #[must_use]
    pub fn family_match_for(&self, family: &GradedFamily) -> usize {
        sub_family_sum(&self.family_match_by_code, family.codes)
    }

    /// A family's misses (partition of `family_missing_by_code` by its code set).
    #[must_use]
    pub fn family_missing_for(&self, family: &GradedFamily) -> usize {
        sub_family_sum(&self.family_missing_by_code, family.codes)
    }

    /// The missing count attributed to `cause` (0 when the cause never fired).
    #[must_use]
    pub fn missing(&self, cause: MissingCause) -> usize {
        self.missing_by_cause.get(&cause).copied().unwrap_or(0)
    }

    /// The duplicate/conflict sub-family matches (partition of `family_match_by_code`).
    #[must_use]
    pub fn dup_match(&self) -> usize {
        self.family_match_for(&FAMILIES[0])
    }

    /// The flow sub-family matches (partition of `family_match_by_code`).
    #[must_use]
    pub fn flow_match(&self) -> usize {
        self.family_match_for(&FAMILIES[1])
    }

    /// The duplicate/conflict sub-family misses (partition of `family_missing_by_code`).
    #[must_use]
    pub fn dup_missing(&self) -> usize {
        self.family_missing_for(&FAMILIES[0])
    }

    /// The flow sub-family misses (partition of `family_missing_by_code`).
    #[must_use]
    pub fn flow_missing(&self) -> usize {
        self.family_missing_for(&FAMILIES[1])
    }

    /// Print the human summary.
    pub fn print(&self) {
        println!("tsc_conformance run");
        println!("===================");
        println!("In-scope tests:            {}", self.in_scope_tests);
        println!("In-scope variants:         {}", self.in_scope_variants);
        println!("  parsed, expect-clean:    {}", self.expect_clean_graded);
        println!("    graded clean:          {}", self.clean_pass);
        println!("    graded NON-clean:      {}", self.clean_fail.len());
        println!("  parsed, baselined:       {}", self.baselined_parsed);
        println!("  parse-rejected:          {}", self.parse_rejected_total);
        println!(
            "    no baseline:           {}",
            self.parse_rejected_no_baseline
        );
        println!(
            "    TS1xxx-only baseline:  {}",
            self.parse_rejected_ts1xxx_only
        );
        println!("    other baseline:        {}", self.parse_rejected_other);
        println!("Script-goal retries:       {}", self.script_retry);
        println!("Bound nodes (total):       {}", self.total_nodes);
        println!();
        println!(
            "Family grading (dup 2300/2451/2567/2528 + merge 2397/2649/2664/2671; flow 7027/7028)"
        );
        println!("---------------------------------------------------------------");
        println!("Graded variants:           {}", self.family_graded_variants);
        println!(
            "  ...family-positive:      {}",
            self.family_positive_variants
        );
        let per_family = |get: &dyn Fn(&GradedFamily) -> usize| -> String {
            FAMILIES
                .iter()
                .map(|f| format!("{} {}", f.key, get(f)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "  match:                   {} ({})",
            self.family_match,
            per_family(&|f| self.family_match_for(f))
        );
        println!(
            "  missing:                 {} ({})",
            self.family_missing,
            per_family(&|f| self.family_missing_for(f))
        );
        // One line per classified cause (label-aligned; `Other` is the gate).
        const CAUSE_LINES: [(MissingCause, &str, &str); 5] = [
            (MissingCause::Merge, "    merge-path:            ", ""),
            (MissingCause::Lib, "    lib-conflict:          ", ""),
            (
                MissingCause::DeferredLateBound,
                "    late-bound (deferred): ",
                " (needs the type engine — literal-type / unique-symbol computed member names)",
            ),
            (
                MissingCause::DeferredCfa,
                "    cfa (deferred):        ",
                " (needs the CFA type engine — never-returning sigs / assertion predicates / switch exhaustiveness / structural reachability)",
            ),
            (
                MissingCause::Other,
                "    other (GATE=0):        ",
                " (unclassified family miss — a same-table cascade bug)",
            ),
        ];
        for (cause, label, note) in CAUSE_LINES {
            println!("{label}{}{note}", self.missing(cause));
        }
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
        println!(
            "  ...family-positive:      {}",
            self.carve_out_rule_a_family
        );
        println!(
            "moduleDetection variants:  {} (watch; inert for family)",
            self.module_detection_variants
        );
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
        println!("Lib base");
        println!("  lib files bound:         {}", self.lib_files_bound);
        println!("  lib sets folded:         {}", self.lib_sets_built);
        println!(
            "  lib parse errors:        {} (GATE=0)",
            self.lib_parse_errors.len()
        );
        println!(
            "  lib missing files:       {} (GATE=0)",
            self.lib_missing_files.len()
        );
        println!(
            "  lib unknown names:       {} (GATE=0)",
            self.lib_unknown_names.len()
        );
        println!(
            "  lib external no-globals: {} (GATE=0)",
            self.lib_external_no_globals.len()
        );
        for e in &self.lib_parse_errors {
            println!("  LIB-PARSE-ERR {e}");
        }
        for f in &self.lib_missing_files {
            println!("  LIB-MISSING {f}");
        }
        for n in &self.lib_unknown_names {
            println!("  LIB-UNKNOWN {n}");
        }
        for f in &self.lib_external_no_globals {
            println!("  LIB-EXT-NO-GLOBALS {f}");
        }
        println!();
        println!("Panics (caught):           {}", self.panics.len());
        println!("Crash-excluded (tracked):  {}", self.excluded_crashes);
        if !self.stale_exclusions.is_empty() {
            println!(
                "Stale crash-exclusions:    {} (drop them)",
                self.stale_exclusions.len()
            );
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
            let paths: Vec<String> = many
                .iter()
                .map(|t| format!("{}/{}", t.suite, t.relative_path))
                .collect();
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
    let source_units: Vec<SourceUnit<'_>> = units
        .iter()
        .map(|u| SourceUnit::new(&u.name, &u.content))
        .collect();
    let bound = bind_program(&source_units, &arena);
    let mut resolver = LibResolver::new(checkout);
    let base = resolver.base_for(&variant.config);
    let lib_files = base.as_ref().map_or(&[][..], |b| b.lib_files.as_slice());
    let result = check_bound(&bound, base.as_deref(), &check_options_for(&variant.config));

    // Resolve each diagnostic's FileId to a display line: a program unit carries its
    // (line, col); a lib file carries the lib name with a masked location.
    let resolve_line = |d: &Diagnostic| -> DiagLine {
        let units_len = units.len();
        match d.file {
            Some(f) if f.index() < units_len => {
                let (line, col) = units.get(f.index()).map_or((None, None), |u| {
                    let (t, m) = LocationTracker::new_ecmascript_with_map(&u.content);
                    let (_, pos) = LocationMapper {
                        tracker: &t,
                        map: &m,
                    }
                    .pos_and_position(d.span.start);
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
            None => DiagLine {
                file: None,
                line: None,
                col: None,
                code: d.code,
                related: Vec::new(),
            },
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

/// Build the flow graph of a corpus test's **first** unit and render it to DOT
/// (the `check-test --dump-flow` product). Parses under the goal rule (Module,
/// then a Script retry), binds (F0), builds the flow product (F1), and renders
/// through `tsv_check`'s source-aware DOT renderer. Keeps the `BoundFile` alive
/// so the renderer can slice subject-node source text from its span column.
pub fn dump_flow_dot(checkout: &Path, name: &str) -> Result<String, String> {
    let corpus = discover_corpus(checkout)?;
    let matches: Vec<&CorpusTest> = corpus
        .iter()
        .filter(|t| t.relative_path == name || t.basename == name)
        .collect();
    let test = match matches.as_slice() {
        [] => return Err(format!("no corpus test matches {name:?}")),
        [one] => *one,
        many => {
            let paths: Vec<String> = many
                .iter()
                .map(|t| format!("{}/{}", t.suite, t.relative_path))
                .collect();
            return Err(format!("{name:?} is ambiguous: {}", paths.join(", ")));
        }
    };

    let content = read_corpus_file(&test.path)?;
    let units = split_units(&content, &test.basename);
    let unit = units
        .first()
        .ok_or_else(|| "test has no units".to_string())?;

    let arena = Bump::new();
    // The goal rule (Module first, Script retry) — the same rule bind_program
    // uses, inlined here because --dump-flow keeps the BoundFile for rendering.
    let program = match tsv_ts::parse_with_goal(&unit.content, tsv_ts::Goal::Module, &arena) {
        Ok(p) => p,
        Err(module_err) => tsv_ts::parse_with_goal(&unit.content, tsv_ts::Goal::Script, &arena)
            .map_err(|_| format!("parse error: {module_err}"))?,
    };
    let bound = bind_file(&program, &unit.content, FileId::ROOT);
    let flow = build_flow(&program, &unit.content, &bound);
    Ok(render_flow_dot(&flow, &bound.spans, &unit.content))
}

/// Select a variant by an optional `k=v` filter (config match, lowercased key);
/// with no filter the first (usually the unvaried) variant.
fn select_variant<'a>(
    variants: &'a [Variant],
    filter: Option<&(String, String)>,
) -> Result<&'a Variant, String> {
    match filter {
        None => variants
            .first()
            .ok_or_else(|| "test has no variants".to_string()),
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
                    format!(
                        "no variant with {key}={value}; available: {}",
                        available.join(", ")
                    )
                })
        }
    }
}

impl CheckTestReport {
    /// Print the human diff (ours vs the baseline summary).
    pub fn print(&self) {
        println!(
            "check-test: {}/{}  variant={}",
            self.suite, self.test, self.variant
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A variant config from `key=value` pairs (the maps store lowercased keys).
    fn config(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn keeps_test_substring() {
        // No `--test` filter keeps every path; an active one keeps only substrings.
        let none = RunFilter::default();
        assert!(none.keeps_test("compiler/anything.ts"));

        let f = RunFilter {
            test: Some("duplicate".to_string()),
            ..RunFilter::default()
        };
        assert!(f.keeps_test("compiler/duplicateVar.ts"));
        assert!(!f.keeps_test("compiler/asyncAwait.ts"));
    }

    #[test]
    fn keeps_variant_key_value() {
        // No `--variant` filter keeps everything.
        let none = RunFilter::default();
        assert!(none.keeps_variant(&config(&[("target", "es5")])));

        let f = RunFilter {
            variant: Some(("target".to_string(), "es2015".to_string())),
            ..RunFilter::default()
        };
        // Exact key=value match keeps.
        assert!(f.keeps_variant(&config(&[("target", "es2015")])));
        // Wrong value excludes.
        assert!(!f.keeps_variant(&config(&[("target", "es5")])));
        // Absent key excludes (the variant doesn't set it).
        assert!(!f.keeps_variant(&config(&[("strict", "true")])));
    }

    #[test]
    fn keeps_code_consults_baseline_only_when_active() {
        // No `--code` filter keeps without ever consulting the baseline resolver
        // (the closure must not run — it would panic if it did).
        let none = RunFilter::default();
        assert!(none.keeps_code(|_| panic!("resolver consulted with no --code filter")));

        let f = RunFilter {
            code: Some(2300),
            ..RunFilter::default()
        };
        // Active filter keeps iff the baseline carries the code.
        let carried = [2300u32, 2451];
        assert!(f.keeps_code(|code| carried.contains(&code)));
        let other = [2451u32];
        assert!(!f.keeps_code(|code| other.contains(&code)));
        // A variant with no baseline (resolver reports false) is excluded.
        assert!(!f.keeps_code(|_| false));
    }

    #[test]
    fn keeps_family_selects_sub_family() {
        // No `--family` filter keeps without consulting the baseline resolver.
        let none = RunFilter::default();
        assert!(none.keeps_family(|_| panic!("resolver consulted with no --family filter")));

        // `flow` keeps iff the baseline carries a FLOW_CODES member; a dup-only
        // baseline is excluded, a flow baseline is kept. (Parsed through the
        // `FAMILIES`-table tokens — the same path the CLI takes.)
        let flow = RunFilter {
            family: FamilyFilter::parse("flow"),
            ..RunFilter::default()
        };
        assert!(flow.keeps_family(|c| c == 7027));
        assert!(!flow.keeps_family(|c| c == 2300));

        // `dup` is the complementary partition.
        let dup = RunFilter {
            family: FamilyFilter::parse("dup"),
            ..RunFilter::default()
        };
        assert!(dup.keeps_family(|c| c == 2300));
        assert!(!dup.keeps_family(|c| c == 7027));

        // `all` keeps any family code (either partition); an unknown token
        // refuses to parse.
        assert!(FamilyFilter::parse("nope").is_none());
        let all = RunFilter {
            family: FamilyFilter::parse("all"),
            ..RunFilter::default()
        };
        assert!(all.keeps_family(|c| c == 7028));
        assert!(all.keeps_family(|c| c == 2451));
        // A non-family code (or no baseline) is excluded.
        assert!(!all.keeps_family(|c| c == 9999));
    }

    #[test]
    fn filters_compose_as_and() {
        // The call site ANDs the three predicates; all must keep for a variant to be
        // graded, and any one failing excludes it.
        let f = RunFilter {
            test: Some("dup".to_string()),
            code: Some(2300),
            variant: Some(("target".to_string(), "es5".to_string())),
            family: None,
        };
        let cfg = config(&[("target", "es5")]);
        let carried = [2300u32];
        let keeps = |path: &str, cfg: &BTreeMap<String, String>, codes: &[u32]| {
            f.keeps_test(path) && f.keeps_variant(cfg) && f.keeps_code(|c| codes.contains(&c))
        };
        // All three match.
        assert!(keeps("compiler/dupVar.ts", &cfg, &carried));
        // Test substring misses.
        assert!(!keeps("compiler/other.ts", &cfg, &carried));
        // Variant value misses.
        assert!(!keeps(
            "compiler/dupVar.ts",
            &config(&[("target", "es2015")]),
            &carried
        ));
        // Code missing from the baseline.
        assert!(!keeps("compiler/dupVar.ts", &cfg, &[2451]));
    }
}
