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

use crate::tsc_conformance::baseline::parse_summary_block;
use crate::tsc_conformance::corpus::{discover_corpus, read_corpus_file, CorpusTest};
use crate::tsc_conformance::directives::{extract_settings, split_units, Unit};
use crate::tsc_conformance::discovery::{baselines_dir, discover_baselines, Baseline};
use crate::tsc_conformance::index::{is_js_flavored, is_jsx_scoped};
use crate::tsc_conformance::options_meta::{
    is_config_file_name, variant_is_unsupported, SKIPPED_TESTS,
};
use crate::tsc_conformance::variants::{config_name, expand, Variant};
use bumpalo::Bump;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Instant;
use tsv_check::{check_program, ParseReport, SourceUnit};

/// Worker-thread stack for the sweep: the corpus has deeply-nested tests and
/// tsv's recursive-descent parser has no depth guard, so the default 8 MiB
/// overflows. 512 MiB is virtual-only reserve on Linux.
const SKELETON_STACK: usize = 512 * 1024 * 1024;

/// Tests that crash the tsv parser — carved out by basename, counted, and
/// reported (never silently). Two crash classes land here: uncatchable
/// stack-overflow aborts (even on [`SKELETON_STACK`]), and debug-build
/// `debug_assert!` panics in `tsv_ts` that `catch_unwind` *does* contain but
/// which are tracked parser bugs to fix rather than absorb. Each entry names its
/// cause; the list is a tracked-defect ledger, not a way to hide bugs.
const CRASH_EXCLUSIONS: &[&str] = &[
    // tsv_ts robustness bug: `export * from <identifier>;` (a non-string module
    // specifier) trips a `debug_assert!(TokenKind::String)` in
    // `parse_string_literal` (parser/mod.rs). Dev-profile only (debug_assert is
    // compiled out in release), so `cargo run` — the gate's profile — panics.
    // A future tsv_ts fix should reject the form gracefully; then drop this entry.
    "exportDeclarationInInternalModule.ts",
];

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
    /// In-scope variants that parsed and DO have a baseline (informational — the
    /// checker emits nothing, so these would all be "missing"; not gated yet).
    pub baselined_parsed: usize,
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

    for test in &corpus {
        if SKIPPED_TESTS.contains(&test.basename.as_str()) {
            continue;
        }
        if CRASH_EXCLUSIONS.contains(&test.basename.as_str()) {
            report.excluded_crashes += 1;
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
        grade_test(test, &units[0], &in_scope, &ondisk, &mut report);
    }

    report.wall_ms = start.elapsed().as_millis();
    Ok(report)
}

/// Parse+check one single-file test once and attribute the outcome to each of
/// its in-scope variants.
fn grade_test(
    test: &CorpusTest,
    unit: &Unit,
    in_scope: &[&Variant],
    ondisk: &HashMap<(&str, String), &Baseline>,
    report: &mut SkeletonReport,
) {
    // Parse + check on a fresh arena, contained against panics.
    let arena = Bump::new();
    let checked = catch_unwind(AssertUnwindSafe(|| {
        check_program(&[SourceUnit::new(&unit.name, &unit.content)], &arena)
    }));
    let Ok(result) = checked else {
        report.panics.push(PanicRecord { test: test.relative_path.clone() });
        return;
    };

    // The single unit's parse outcome (files is never empty for one input).
    let Some(file) = result.files.first() else { return };
    for variant in in_scope {
        report.in_scope_variants += 1;
        let name = config_name(&test.basename, &variant.description);
        let baseline = ondisk.get(&(test.suite, name.clone())).copied();

        match &file.parse {
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
                if baseline.is_none() {
                    report.expect_clean_graded += 1;
                    if result.diagnostics.is_empty() {
                        report.clean_pass += 1;
                    } else {
                        report.clean_fail.push(CleanFail {
                            variant: format!("{}/{name}", test.suite),
                            diagnostics: result.diagnostics.len(),
                        });
                    }
                } else {
                    report.baselined_parsed += 1;
                }
            }
        }
    }

    // Node total: counted once per test (all variants share the parse).
    if let ParseReport::Parsed(facts) = &file.parse {
        report.total_nodes += u64::from(facts.node_count);
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
        println!("  parsed, baselined:       {} (informational)", self.baselined_parsed);
        println!("  parse-rejected:          {}", self.parse_rejected_total);
        println!("    no baseline:           {}", self.parse_rejected_no_baseline);
        println!("    TS1xxx-only baseline:  {}", self.parse_rejected_ts1xxx_only);
        println!("    other baseline:        {}", self.parse_rejected_other);
        println!("Script-goal retries:       {}", self.script_retry);
        println!("Bound nodes (total):       {}", self.total_nodes);
        println!();
        println!("Panics (caught):           {}", self.panics.len());
        println!("Crash-excluded (tracked):  {}", self.excluded_crashes);
        println!("Wall-clock:                {} ms", self.wall_ms);
        if !self.clean_fail.is_empty() {
            println!();
            for f in &self.clean_fail {
                println!("  CLEAN-FAIL {} ({} diagnostics)", f.variant, f.diagnostics);
            }
        }
        for p in &self.panics {
            println!("  PANIC {}", p.test);
        }
    }
}

// ===========================================================================
// check-test: the inner dev loop over one test.
// ===========================================================================

/// One diagnostic line (ours or the baseline's) for the check-test diff.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagLine {
    /// The file the diagnostic points at (or `null` for a global one).
    pub file: Option<String>,
    /// 1-based line (`null` for a global diagnostic).
    pub line: Option<u32>,
    /// 1-based column (`null` for a global diagnostic).
    pub col: Option<u32>,
    /// The `TS<code>` number.
    pub code: u32,
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

    // Parse + check every unit (single- or multi-file).
    let arena = Bump::new();
    let source_units: Vec<SourceUnit<'_>> =
        units.iter().map(|u| SourceUnit::new(&u.name, &u.content)).collect();
    let result = check_program(&source_units, &arena);

    let ours: Vec<DiagLine> = result
        .diagnostics
        .iter()
        .map(|d| DiagLine {
            file: d.file.and_then(|f| source_units.get(f.index()).map(|u| u.name.to_string())),
            line: None,
            col: None,
            code: d.code,
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
                    .map(|d| DiagLine { file: d.file, line: d.line, col: d.col, code: d.code })
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
        _ => format!("error TS{} (global)", d.code),
    }
}
