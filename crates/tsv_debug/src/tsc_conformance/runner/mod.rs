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

mod filter;
mod grade;
mod report;
#[cfg(test)]
mod tests;
mod watchdog;

pub use filter::{FamilyFilter, RunFilter, RunOptions};
pub use report::{CheckTestReport, SkeletonReport};

use grade::{check_options_for, run_skeleton_inner};
use report::DiagLine;
use watchdog::TestWatchdog;

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

/// Worker-thread stack for the sweep: the corpus has deeply-nested tests and
/// tsv's recursive-descent parser has no depth guard, so the default 8 MiB
/// overflows. 512 MiB is virtual-only reserve on Linux.
const SKELETON_STACK: usize = 512 * 1024 * 1024;

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
