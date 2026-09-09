//! Execute test262 tests against tsv's parser.

#![allow(dead_code)] // Some types/variants are useful for future expansion

use super::discovery::TestFile;
use super::frontmatter;
use std::borrow::Cow;
use std::fs;

/// Result of running a single test.
#[derive(Debug)]
pub enum TestResult {
    /// Test passed (result matched expectation)
    Passed,
    /// Test failed (result didn't match expectation)
    Failed(FailureReason),
    /// Test was skipped
    Skipped(SkipReason),
}

/// Reason a test failed.
#[derive(Debug)]
pub enum FailureReason {
    /// Should have parsed successfully but didn't
    UnexpectedParseError(String),
    /// Should have failed to parse but succeeded
    UnexpectedParseSuccess,
    /// A mode-unflagged test's two runs disagreed: one accepted the source and
    /// the other rejected it, so the test's own claim — that the verdict is the
    /// same sloppy and strict — does not hold for tsv. Names the run that
    /// rejected and carries its parse error.
    ModeDisagreement {
        /// Which of the two runs rejected.
        rejected_by: ModeRun,
        /// The rejecting run's parse error.
        error: String,
    },
    /// Couldn't read the test file
    ReadError(String),
}

/// Reason a test was skipped.
#[derive(Debug)]
pub enum SkipReason {
    /// Negative test with runtime phase
    RuntimePhase,
    /// Negative test with resolution phase
    ResolutionPhase,
    /// No frontmatter found
    NoFrontmatter,
    /// Test declares a sloppy-only run (`flags: [noStrict]`) of the Annex B
    /// web-compatibility grammar — a test under `test/annexB/`. Annex B is
    /// "normative but optional if the ECMAScript host is not a web browser"
    /// (ecma262, Annex B preamble) and tsv is not one, so those tests are out of
    /// scope rather than failures. The skip is `noStrict` **and** the subtree,
    /// never the subtree alone: the rest of `test/annexB/` is runtime library
    /// tests (`String.prototype.substr`, `escape`, the RegExp extensions) whose
    /// syntax is ordinary, and those stay graded.
    AnnexB,
    /// Test requires a syntactic proposal tsv does not implement (the named
    /// `features:` entry); see `frontmatter::UNIMPLEMENTED_FEATURES`.
    UnimplementedFeature(&'static str),
    /// The harness's prepended `"use strict"` directive cannot be applied to this
    /// test honestly, for the named reason. No such test exists in test262 — the
    /// bucket is what makes that a checked fact rather than an assumption
    /// [`strict_source`] rests on.
    StrictPrefixConflict(StrictPrefixConflict),
}

/// Why a test's strict run cannot be graded through [`USE_STRICT_PREFIX`] — the
/// three ways the prefix's own preconditions fail, each a shape test262 does not
/// carry today and this bucket keeps checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrictPrefixConflict {
    /// `raw` forbids any modification of the source, `onlyStrict` is graded only
    /// through the prepended directive.
    RawFlag,
    /// `onlyStrict` and `noStrict` name opposite single runs; the harness's own
    /// rule is that they are mutually exclusive.
    NoStrictFlag,
    /// A hashbang or a BOM at byte 0, on a test one of whose runs is prefixed: the
    /// prefix moves it off byte 0, where it is no longer a hashbang or a BOM, so
    /// the strict run would grade a different program (and a positive would fail
    /// the gate with a message blaming tsv's strictness model).
    ByteZero,
}

impl StrictPrefixConflict {
    /// The conflict's name for the run summary.
    pub fn label(self) -> &'static str {
        match self {
            Self::RawFlag => "raw + onlyStrict",
            Self::NoStrictFlag => "onlyStrict + noStrict",
            Self::ByteZero => "hashbang/BOM under a strict prefix",
        }
    }
}

/// Summary of test results.
#[derive(Debug, Default)]
pub struct TestSummary {
    pub positive_passed: usize,
    pub positive_failed: usize,
    pub negative_passed: usize,
    pub negative_failed: usize,
    pub skipped_runtime: usize,
    pub skipped_resolution: usize,
    pub skipped_no_frontmatter: usize,
    pub skipped_annex_b: usize,
    pub skipped_unimplemented_feature: usize,
    pub skipped_strict_prefix_conflict: usize,
    pub skipped_filtered: usize,
    pub failures: Vec<(String, FailureReason)>,
}

impl TestSummary {
    /// Get total skipped count (excluding user-filtered).
    pub fn skipped(&self) -> usize {
        self.skipped_runtime
            + self.skipped_resolution
            + self.skipped_no_frontmatter
            + self.skipped_annex_b
            + self.skipped_unimplemented_feature
            + self.skipped_strict_prefix_conflict
    }
}

impl TestSummary {
    /// Add a test result to the summary.
    pub fn add(&mut self, test_path: &str, is_negative: bool, result: TestResult) {
        match result {
            TestResult::Passed => {
                if is_negative {
                    self.negative_passed += 1;
                } else {
                    self.positive_passed += 1;
                }
            }
            TestResult::Failed(reason) => {
                if is_negative {
                    self.negative_failed += 1;
                } else {
                    self.positive_failed += 1;
                }
                self.failures.push((test_path.to_string(), reason));
            }
            TestResult::Skipped(reason) => match reason {
                SkipReason::RuntimePhase => self.skipped_runtime += 1,
                SkipReason::ResolutionPhase => self.skipped_resolution += 1,
                SkipReason::NoFrontmatter => self.skipped_no_frontmatter += 1,
                SkipReason::AnnexB => self.skipped_annex_b += 1,
                SkipReason::UnimplementedFeature(_) => self.skipped_unimplemented_feature += 1,
                SkipReason::StrictPrefixConflict(_) => self.skipped_strict_prefix_conflict += 1,
            },
        }
    }

    /// Get total number of tests run (excluding skipped).
    pub fn total_run(&self) -> usize {
        self.positive_passed + self.positive_failed + self.negative_passed + self.negative_failed
    }

    /// Get total number of failures.
    pub fn total_failed(&self) -> usize {
        self.positive_failed + self.negative_failed
    }

    /// Check if all tests passed.
    pub fn all_passed(&self) -> bool {
        self.total_failed() == 0
    }
}

/// How a test's frontmatter classifies it: skip (with a reason) or grade it.
///
/// The single source of truth for "what tsv grades", shared by `run_test` and
/// `grade_for_manifest` so the differential manifest covers exactly the runner's
/// graded set.
enum Classification {
    /// tsv does not grade this test.
    Skip(SkipReason),
    /// tsv grades this test in the parse phase.
    Grade {
        /// Whether a parse-phase failure is expected (negative parse test).
        is_negative_parse: bool,
        /// The run(s) the test's flags declare.
        runs: GradedRuns,
    },
}

/// The parse run(s) a test declares, read off its `flags`
/// (test262/INTERPRETING.md §Strict Mode, §flags). This is the whole of what the
/// runner needs from strictness metadata: it picks the goal, decides whether the
/// harness's `"use strict"` prefix applies, and says how many parses grade the
/// test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GradedRuns {
    /// `flags: [module]` — one run at `Goal::Module`, strict by the goal itself.
    Module,
    /// `flags: [onlyStrict]` — one Script run, made strict the way the harness
    /// makes it strict, by prepending [`USE_STRICT_PREFIX`] to the source.
    StrictScript,
    /// `flags: [noStrict]`, or `flags: [raw]` (verbatim source, non-strict mode
    /// only) — one sloppy Script run over the file's own bytes.
    SloppyScript,
    /// No mode flag — the bulk of the suite. test262 requires the test to run
    /// **twice**, sloppy and strict, and declares the verdict identical in both,
    /// so tsv grades both and holds the test to that claim.
    BothScripts,
}

/// One of the two runs a [`GradedRuns::BothScripts`] test declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeRun {
    /// The file's own bytes, at a sloppy `Goal::Script`.
    Sloppy,
    /// The same bytes behind the harness's [`USE_STRICT_PREFIX`].
    Strict,
}

impl ModeRun {
    /// The run's name for a failure message.
    fn label(self) -> &'static str {
        match self {
            Self::Sloppy => "sloppy",
            Self::Strict => "strict",
        }
    }
}

/// Read a test's declared run(s) off its flags.
///
/// `module` wins over everything (it is a goal, and it negates the two-run
/// default by itself); `onlyStrict` and `noStrict` each declare their single
/// run; `raw` forbids the source modification the strict run needs, so a raw
/// test is the sloppy run alone. Everything left is the two-run default.
fn graded_runs(frontmatter: &frontmatter::Frontmatter) -> GradedRuns {
    if frontmatter.is_module() {
        GradedRuns::Module
    } else if frontmatter.is_only_strict() {
        GradedRuns::StrictScript
    } else if frontmatter.is_no_strict() || frontmatter.is_raw() {
        GradedRuns::SloppyScript
    } else {
        GradedRuns::BothScripts
    }
}

/// The directive test262-harness inserts as the initial character sequence of a
/// test's source before running it in strict mode
/// (test262/INTERPRETING.md §Strict Mode). The tests never carry it themselves.
const USE_STRICT_PREFIX: &str = "\"use strict\";\n";

/// The source of a **strict** Script run: the file's own bytes behind
/// [`USE_STRICT_PREFIX`], exactly as test262-harness produces them.
///
/// The prefix shifts every byte offset by its length and every line number by
/// one. Nothing grades a position: `run_test` only formats the parse error into
/// a human-facing failure message — which says so — and `grade_for_manifest`
/// reduces it to a [`Verdict`], so the shift is display-only.
fn strict_source(content: &str) -> String {
    format!("{USE_STRICT_PREFIX}{content}")
}

/// The source of a **single**-run test: the strict transform for the one shape
/// the harness applies it to, the file's own bytes otherwise.
///
/// A [`GradedRuns::BothScripts`] test has two sources, not one, and reaches
/// [`strict_source`] directly for its strict half — so the transform is spelled
/// in exactly one place either way.
fn graded_source(content: &str, runs: GradedRuns) -> Cow<'_, str> {
    if takes_strict_prefix(runs) {
        Cow::Owned(strict_source(content))
    } else {
        Cow::Borrowed(content)
    }
}

/// Whether a single-run test's graded source carries [`USE_STRICT_PREFIX`].
/// Spelled once, so `graded_source` and the failure message that reports the
/// line shift agree.
fn takes_strict_prefix(runs: GradedRuns) -> bool {
    matches!(runs, GradedRuns::StrictScript)
}

/// The test262 subtree holding the **Annex B** web-compatibility tests
/// (`/`-separated, as `TestFile::relative_path` spells it on Unix).
const ANNEX_B_PREFIX: &str = "test/annexB/";

/// Whether a test262-root-relative path names a test under [`ANNEX_B_PREFIX`].
fn is_annex_b_path(relative_path: &str) -> bool {
    relative_path.replace('\\', "/").starts_with(ANNEX_B_PREFIX)
}

/// Read a test's frontmatter and decide skip-vs-grade.
///
/// tsv grades both modes, so a test is skipped only for something tsv does not
/// implement or cannot parse-grade: the Annex B grammar, runtime/resolution
/// negatives (we only test parsing), an unimplemented syntactic proposal,
/// contradictory `raw` + `onlyStrict` metadata, and files with no frontmatter.
/// `relative_path` is the test262-root-relative path, needed only for the Annex B
/// skip.
fn classify(relative_path: &str, content: &str) -> Classification {
    let Some(frontmatter) = frontmatter::parse(content) else {
        return Classification::Skip(SkipReason::NoFrontmatter);
    };
    if frontmatter.is_negative_runtime() {
        return Classification::Skip(SkipReason::RuntimePhase);
    }
    if frontmatter.is_negative_resolution() {
        return Classification::Skip(SkipReason::ResolutionPhase);
    }
    // Drop tests whose syntax tsv hasn't implemented from the graded set: scoring
    // them as parse failures measures scope, not a conformance gap. Both polarities
    // go — we shouldn't claim credit for rejecting a negative whose feature we
    // reject wholesale either. `UNIMPLEMENTED_FEATURES` is currently empty, so
    // nothing matches here
    // until a new unimplemented proposal is added.
    if let Some(feature) = frontmatter.requires_unimplemented_feature() {
        return Classification::Skip(SkipReason::UnimplementedFeature(feature));
    }
    // A sloppy-only test OF THE ANNEX B GRAMMAR is out of scope: Annex B is
    // optional for a non-browser host and tsv is one, so its constructs are a
    // declared non-goal rather than a conformance gap. Keyed on `noStrict` AND
    // the subtree — the rest of `test/annexB/` is mostly runtime library tests
    // plus semantics tests over core syntax, and a directory-keyed skip would
    // drop them for nothing.
    if frontmatter.is_no_strict() && is_annex_b_path(relative_path) {
        return Classification::Skip(SkipReason::AnnexB);
    }
    // The strict prefix's preconditions, refused rather than assumed: `raw` +
    // `onlyStrict` (the first forbids modifying the source, the second is graded
    // only through the prepended directive), `onlyStrict` + `noStrict` (opposite
    // single runs — `graded_runs` would silently pick one), and a byte-0 hashbang
    // or BOM on a test that takes the prefix (the prefix moves it off byte 0, where
    // it is a different program). test262 carries none of these; refusing them here
    // is what lets `strict_source` prepend unconditionally.
    if frontmatter.is_only_strict() && frontmatter.is_raw() {
        return Classification::Skip(SkipReason::StrictPrefixConflict(
            StrictPrefixConflict::RawFlag,
        ));
    }
    if frontmatter.is_only_strict() && frontmatter.is_no_strict() {
        return Classification::Skip(SkipReason::StrictPrefixConflict(
            StrictPrefixConflict::NoStrictFlag,
        ));
    }
    let runs = graded_runs(&frontmatter);
    if has_strict_run(runs) && starts_at_byte_zero(content) {
        return Classification::Skip(SkipReason::StrictPrefixConflict(
            StrictPrefixConflict::ByteZero,
        ));
    }
    Classification::Grade {
        is_negative_parse: frontmatter.is_negative_parse(),
        runs,
    }
}

/// Whether any of a test's runs is graded behind [`USE_STRICT_PREFIX`].
fn has_strict_run(runs: GradedRuns) -> bool {
    matches!(runs, GradedRuns::StrictScript | GradedRuns::BothScripts)
}

/// Whether the source opens with something that is only itself at byte 0 — a
/// hashbang comment or a byte-order mark — and so cannot take a prefix.
fn starts_at_byte_zero(content: &str) -> bool {
    content.starts_with("#!") || content.starts_with('\u{feff}')
}

/// The parse goal for a graded test. A `module`-flagged test is parsed as a
/// `Module`; every other run is a `Script` — `await` is an ordinary identifier
/// there, and `import`/`export`/`import.meta` are syntax errors. The goal carries
/// no strictness of its own past `Module`: a Script run is strict exactly when its
/// source carries the harness's `"use strict"` prefix.
fn goal_for(runs: GradedRuns) -> tsv_ts::Goal {
    match runs {
        GradedRuns::Module => tsv_ts::Goal::Module,
        GradedRuns::StrictScript | GradedRuns::SloppyScript | GradedRuns::BothScripts => {
            tsv_ts::Goal::Script
        }
    }
}

/// Run a single test and return the result.
pub fn run_test(test: &TestFile) -> (TestResult, Option<bool>) {
    let content = match fs::read_to_string(&test.path) {
        Ok(c) => c,
        Err(e) => {
            return (
                TestResult::Failed(FailureReason::ReadError(e.to_string())),
                None,
            );
        }
    };

    match classify(&test.relative_path, &content) {
        Classification::Skip(reason) => (TestResult::Skipped(reason), None),
        Classification::Grade {
            is_negative_parse,
            runs,
        } => (
            grade_runs(&content, is_negative_parse, runs),
            Some(is_negative_parse),
        ),
    }
}

/// Grade every run a test declares, into one result.
///
/// A single-run test is its run's verdict. A mode-unflagged test declares
/// **two** runs — sloppy and strict — and asserts the same verdict in both
/// (test262/INTERPRETING.md §Strict Mode), so tsv parses both and holds it to
/// that claim: a positive passes only if both accept, a parse negative only if
/// both reject. When the two disagree the test's claim is the one thing that
/// cannot be true, so the disagreement is its own failure rather than a plain
/// accept/reject miss — it says tsv's strictness model, not the source, is what
/// differs between the runs.
fn grade_runs(content: &str, is_negative_parse: bool, runs: GradedRuns) -> TestResult {
    if runs != GradedRuns::BothScripts {
        return run_parse_test(content, is_negative_parse, runs);
    }

    let sloppy = parse_run(content, tsv_ts::Goal::Script);
    let strict = parse_run(&strict_source(content), tsv_ts::Goal::Script);
    match (sloppy, strict) {
        (None, None) => {
            if is_negative_parse {
                TestResult::Failed(FailureReason::UnexpectedParseSuccess)
            } else {
                TestResult::Passed
            }
        }
        (Some(error), Some(_)) => {
            if is_negative_parse {
                TestResult::Passed
            } else {
                TestResult::Failed(FailureReason::UnexpectedParseError(error))
            }
        }
        (None, Some(error)) => TestResult::Failed(FailureReason::ModeDisagreement {
            rejected_by: ModeRun::Strict,
            error: format!("{error}{}", strict_prefix_note()),
        }),
        (Some(error), None) => TestResult::Failed(FailureReason::ModeDisagreement {
            rejected_by: ModeRun::Sloppy,
            error,
        }),
    }
}

/// Parse one run and render its error, or `None` when the source parsed. The
/// unit both `grade_runs` and `run_parse_test` grade.
fn parse_run(source: &str, goal: tsv_ts::Goal) -> Option<String> {
    // test262 tests are pure ECMAScript, so we parse as TypeScript (a superset).
    let arena = bumpalo::Bump::new();
    match tsv_ts::parse_with_goal(source, goal, &arena) {
        Ok(_) => None,
        Err(error) => Some(format!("{error:?}")),
    }
}

/// The note appended to a strict run's parse error: every position it carries
/// counts from the prefixed source rather than the file.
fn strict_prefix_note() -> String {
    // The prefix is one line and `USE_STRICT_PREFIX.len()` bytes, so every
    // position in the render is off the file's by exactly that.
    format!(
        " (source graded with the `\"use strict\";` prefix: `position` is +{} \
         bytes and `line_number` +1 against the file)",
        USE_STRICT_PREFIX.len()
    )
}

/// Accept-or-reject verdict for a single parse — the unit of the differential
/// manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// The parser produced an AST.
    Accept,
    /// The parser reported a syntax error.
    Reject,
}

/// One graded test's row in the differential manifest.
///
/// `expected` is what test262 wants (accept for positives, reject for
/// parse-phase negatives); `tsv` is what `tsv_ts::parse` actually did. A
/// downstream consumer (`benches/js/diagnostics/test262_compare.ts`) runs the
/// alternative parser over the same file and joins on `relative_path`.
///
/// **One row per test, not per run.** A mode-unflagged test is graded twice by
/// `run_test` (sloppy and strict); the manifest carries its **sloppy** run,
/// which `strict: false` names — so a consumer that reproduces the row's parse
/// reproduces one real run of the test, and both sides compare like for like.
/// The strict half of such a test is graded by the runner alone.
#[derive(Debug, serde::Serialize)]
pub struct ManifestEntry {
    /// Path relative to the test262 root — the join key, and (joined onto
    /// `Manifest::test262_root`) where the consumer reads the source.
    pub relative_path: String,
    /// Whether the test carries `flags: [module]`. Load-bearing: it selects the
    /// parse goal on both sides of the differential (`module` → `Goal::Module`,
    /// else `Goal::Script`), and the consumer mirrors that goal in the
    /// alternative parser — so an `await`-as-identifier script test lands in
    /// `both-accept`, not `both-reject`.
    pub module: bool,
    /// Whether this row's parse is a strict one: `module` (strict by the goal)
    /// or `onlyStrict` (strict by the harness's prepended `"use strict"` — see
    /// `graded_source`). A consumer reproducing tsv's parse applies the same
    /// directive when this is `true` and `module` is `false`. A mode-unflagged
    /// test's row is its sloppy run, so this is `false` there — the goal alone
    /// then carries the mode, which is what makes the row reproducible from
    /// `module` + `strict` and nothing else.
    pub strict: bool,
    /// What test262 expects: `accept` for positives, `reject` for parse negatives.
    pub expected: Verdict,
    /// What `tsv_ts::parse_with_goal` did on this row's source and goal.
    pub tsv: Verdict,
}

/// Top-level differential manifest: tsv's graded subset plus metadata.
#[derive(Debug, serde::Serialize)]
pub struct Manifest {
    /// The test262 root the `relative_path`s are relative to, exactly as passed
    /// on the CLI (e.g. `../test262`). The consumer joins it with each
    /// `relative_path` to read the source.
    pub test262_root: String,
    /// Number of graded tests (`== tests.len()`).
    pub count: usize,
    /// One row per graded test (positive and negative).
    pub tests: Vec<ManifestEntry>,
}

impl Manifest {
    /// Grade every test, keeping only the rows tsv actually grades.
    pub fn build(test262_root: String, tests: &[TestFile]) -> Self {
        let entries: Vec<ManifestEntry> = tests.iter().filter_map(grade_for_manifest).collect();
        Self {
            test262_root,
            count: entries.len(),
            tests: entries,
        }
    }
}

/// Grade one test for the differential manifest, or `None` if tsv skips it.
///
/// Shares `classify` with `run_test`, so the manifest covers precisely tsv's
/// graded subset (unreadable files are also skipped).
pub fn grade_for_manifest(test: &TestFile) -> Option<ManifestEntry> {
    let content = fs::read_to_string(&test.path).ok()?;
    let Classification::Grade {
        is_negative_parse,
        runs,
    } = classify(&test.relative_path, &content)
    else {
        return None;
    };

    let expected = if is_negative_parse {
        Verdict::Reject
    } else {
        Verdict::Accept
    };
    // One row per test, so a two-run test contributes its SLOPPY run — the row's
    // `strict: false` says which, and the consumer reproduces exactly that parse.
    let source = graded_source(&content, runs);
    let tsv = match parse_run(&source, goal_for(runs)) {
        None => Verdict::Accept,
        Some(_) => Verdict::Reject,
    };

    let (module, strict) = manifest_mode(runs);
    Some(ManifestEntry {
        relative_path: test.relative_path.clone(),
        module,
        strict,
        expected,
        tsv,
    })
}

/// A manifest row's `(module, strict)` pair for a test's runs — the contract the
/// differential consumer (`benches/js/diagnostics/test262_compare.ts`) keys its
/// own strict prefix on: it prepends exactly when `strict && !module`, so
/// `strict` must name the graded source's mode, not the test's flag.
fn manifest_mode(runs: GradedRuns) -> (bool, bool) {
    (
        runs == GradedRuns::Module,
        matches!(runs, GradedRuns::Module | GradedRuns::StrictScript),
    )
}

/// Run a single-run parse test over the file's own `content` and return the
/// result.
///
/// The source graded and the goal it is graded at both follow from `runs`
/// (`graded_source`, `goal_for`), and so does whether the rendered parse error
/// counts from a prefixed source: when it does, every position it carries — the
/// byte `position` and the `line_number` in its context — is shifted from the
/// file's, so the failure message names both shifts.
fn run_parse_test(content: &str, is_negative_parse: bool, runs: GradedRuns) -> TestResult {
    debug_assert_ne!(
        runs,
        GradedRuns::BothScripts,
        "a two-run test is graded by grade_runs"
    );
    let source = graded_source(content, runs);
    match (parse_run(&source, goal_for(runs)), is_negative_parse) {
        // Positive test passed: parsed successfully as expected
        (None, false) => TestResult::Passed,

        // Positive test failed: should have parsed but didn't
        (Some(error), false) => {
            let note = if takes_strict_prefix(runs) {
                Cow::Owned(strict_prefix_note())
            } else {
                Cow::Borrowed("")
            };
            TestResult::Failed(FailureReason::UnexpectedParseError(format!(
                "{error}{note}"
            )))
        }

        // Negative test passed: failed to parse as expected
        (Some(_), true) => TestResult::Passed,

        // Negative test failed: should have failed but parsed successfully
        (None, true) => TestResult::Failed(FailureReason::UnexpectedParseSuccess),
    }
}

/// Format a failure reason for display.
pub fn format_failure(reason: &FailureReason) -> String {
    match reason {
        FailureReason::UnexpectedParseError(e) => {
            format!("Expected: Parse success\nGot: Parse error\n{e}")
        }
        FailureReason::UnexpectedParseSuccess => {
            "Expected: Parse error (phase: parse)\nGot: Parse success".to_string()
        }
        FailureReason::ModeDisagreement { rejected_by, error } => format!(
            "Expected: the same verdict sloppy and strict (the test declares both runs)\n\
             Got: the {} run rejected and the other accepted\n{error}",
            rejected_by.label()
        ),
        FailureReason::ReadError(e) => format!("Could not read file: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The import-phase proposals are parsed, so a test requiring one is
    /// graded, not skipped — `UNIMPLEMENTED_FEATURES` is empty. (Were a future
    /// proposal added to that list, `classify` would route it to the skip bucket;
    /// the `frontmatter` predicate's own unit test covers the matching itself.)
    #[test]
    fn classify_grades_implemented_proposals() {
        let source_phase =
            "/*---\nfeatures: [source-phase-imports, dynamic-import]\n---*/\nimport.source('x');\n";
        assert!(matches!(
            classify("test/x.js", source_phase),
            Classification::Grade { .. }
        ));

        let import_defer = "/*---\nfeatures: [import-defer]\n---*/\nimport.defer('x');\n";
        assert!(matches!(
            classify("test/x.js", import_defer),
            Classification::Grade { .. }
        ));

        // Plain dynamic import is implemented — graded, not skipped.
        let plain = "/*---\nfeatures: [dynamic-import]\n---*/\nimport('x');\n";
        assert!(matches!(
            classify("test/x.js", plain),
            Classification::Grade { .. }
        ));
    }

    /// A `noStrict` test declares the sloppy run, which tsv grades — the only
    /// ones skipped are the Annex B ones, keyed on the subtree as well as the
    /// flag.
    #[test]
    fn classify_grades_nostrict_outside_annex_b() {
        let no_strict = "/*---\nflags: [noStrict]\n---*/\nwith ({}) {}\n";
        assert!(matches!(
            classify("test/language/statements/with/x.js", no_strict),
            Classification::Grade {
                runs: GradedRuns::SloppyScript,
                ..
            }
        ));

        // Same flag under `test/annexB/` — out of scope for a non-browser host.
        assert!(matches!(
            classify("test/annexB/language/statements/with/x.js", no_strict),
            Classification::Skip(SkipReason::AnnexB)
        ));

        // An Annex B test WITHOUT `noStrict` is an ordinary runtime library test
        // whose syntax is fine: graded, not skipped.
        let annex_b_library = "/*---\nesid: sec-string.prototype.substr\n---*/\n'a'.substr(0);\n";
        assert!(matches!(
            classify(
                "test/annexB/built-ins/String/prototype/substr/x.js",
                annex_b_library
            ),
            Classification::Grade { .. }
        ));
    }

    /// A `raw` test's source is used verbatim and runs in non-strict mode only,
    /// so it is graded as the sloppy run alone — including the one whose `#!`
    /// turns its `"use strict"` into a comment and leaves a sloppy `with`.
    #[test]
    fn classify_grades_raw_as_the_sloppy_run() {
        let raw = "/*---\nflags: [raw]\n---*/\n#!/usr/bin/env node\n";
        assert!(matches!(
            classify(
                "test/language/comments/hashbang/preceding-whitespace.js",
                raw
            ),
            Classification::Grade {
                runs: GradedRuns::SloppyScript,
                ..
            }
        ));

        let sloppy_raw = "/*---\nflags: [raw]\n---*/\n#!\"use strict\"\nwith ({}) {}\n";
        assert!(matches!(
            classify("test/language/comments/hashbang/use-strict.js", sloppy_raw),
            Classification::Grade {
                runs: GradedRuns::SloppyScript,
                ..
            }
        ));
    }

    /// `module` outranks `raw`: a `[module, raw]` test is one Module parse of
    /// the file's own bytes, so the verbatim-source demand is honored by
    /// construction rather than by the flag's own arm. The suite carries such
    /// tests (`test/language/comments/hashbang/module.js`).
    #[test]
    fn classify_grades_module_raw_as_one_module_run() {
        let module_raw = "#!/usr/bin/env node\n/*---\nflags: [module, raw]\n---*/\n";
        assert!(matches!(
            classify("test/language/comments/hashbang/module.js", module_raw),
            Classification::Grade {
                runs: GradedRuns::Module,
                ..
            }
        ));
        assert_eq!(graded_source(module_raw, GradedRuns::Module), module_raw);
    }

    /// A test with no mode flag declares BOTH runs, and the classification says
    /// so — that is what makes `grade_runs` parse it twice.
    #[test]
    fn classify_reports_both_runs_when_unflagged() {
        let plain = "/*---\nesid: sec-example\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", plain),
            Classification::Grade {
                runs: GradedRuns::BothScripts,
                ..
            }
        ));

        // `module` wins over the two-run default: it negates it by itself.
        let module = "/*---\nflags: [module]\n---*/\nexport default 1;\n";
        assert!(matches!(
            classify("test/x.js", module),
            Classification::Grade {
                runs: GradedRuns::Module,
                ..
            }
        ));
    }

    /// The two runs of an unflagged test are graded together: a positive passes
    /// only if both accept, a negative only if both reject, and a construct whose
    /// verdict is strictness-keyed makes the two disagree.
    #[test]
    fn grade_runs_holds_both_scripts_to_one_verdict() {
        // Mode-independent and valid: both runs accept.
        assert!(matches!(
            grade_runs("var x = 1;\n", false, GradedRuns::BothScripts),
            TestResult::Passed
        ));
        // Mode-independent and invalid: both runs reject, so the negative passes.
        assert!(matches!(
            grade_runs("var 1 = x;\n", true, GradedRuns::BothScripts),
            TestResult::Passed
        ));
        // `with` is legal sloppy and a syntax error strict: the runs disagree, so
        // neither polarity can pass, and the failure names the strict run.
        assert!(matches!(
            grade_runs("with ({}) {}\n", false, GradedRuns::BothScripts),
            TestResult::Failed(FailureReason::ModeDisagreement {
                rejected_by: ModeRun::Strict,
                ..
            })
        ));
        assert!(matches!(
            grade_runs("with ({}) {}\n", true, GradedRuns::BothScripts),
            TestResult::Failed(FailureReason::ModeDisagreement {
                rejected_by: ModeRun::Strict,
                ..
            })
        ));
        // A `noStrict` test of the same source declares the sloppy run only, and
        // that run accepts.
        assert!(matches!(
            grade_runs("with ({}) {}\n", false, GradedRuns::SloppyScript),
            TestResult::Passed
        ));
    }

    /// `onlyStrict` reaches the graded classification, so the harness's
    /// strict-mode transform can be applied to it.
    #[test]
    fn classify_reports_only_strict() {
        let only_strict = "/*---\nflags: [onlyStrict]\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", only_strict),
            Classification::Grade {
                runs: GradedRuns::StrictScript,
                ..
            }
        ));
    }

    /// `raw` and `onlyStrict` contradict each other, so such a test is refused
    /// rather than graded against a source neither flag sanctions.
    #[test]
    fn classify_skips_raw_with_only_strict() {
        let conflict = "/*---\nflags: [raw, onlyStrict]\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", conflict),
            Classification::Skip(SkipReason::StrictPrefixConflict(
                StrictPrefixConflict::RawFlag
            ))
        ));

        // Either flag alone still grades.
        let raw_only = "/*---\nflags: [raw]\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", raw_only),
            Classification::Grade { .. }
        ));
    }

    /// `onlyStrict` and `noStrict` name opposite single runs; refused rather than
    /// silently resolved to one of them.
    #[test]
    fn classify_skips_only_strict_with_no_strict() {
        let conflict = "/*---\nflags: [onlyStrict, noStrict]\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", conflict),
            Classification::Skip(SkipReason::StrictPrefixConflict(
                StrictPrefixConflict::NoStrictFlag
            ))
        ));
    }

    /// A byte-0 hashbang or BOM is only itself at byte 0, so a test that takes the
    /// strict prefix on any run is refused; one graded on its own bytes alone is not.
    #[test]
    fn classify_skips_byte_zero_content_under_a_strict_prefix() {
        // Unflagged = both runs, one of them prefixed.
        let hashbang = "#!/usr/bin/env node\n/*---\ndescription: x\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", hashbang),
            Classification::Skip(SkipReason::StrictPrefixConflict(
                StrictPrefixConflict::ByteZero
            ))
        ));
        let bom = "\u{feff}/*---\nflags: [onlyStrict]\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", bom),
            Classification::Skip(SkipReason::StrictPrefixConflict(
                StrictPrefixConflict::ByteZero
            ))
        ));

        // `raw` (own bytes, sloppy) and `module` take no prefix, so both grade —
        // which is how test262's eight hashbang tests are actually flagged.
        let raw = "#!/usr/bin/env node\n/*---\nflags: [raw]\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", raw),
            Classification::Grade {
                runs: GradedRuns::SloppyScript,
                ..
            }
        ));
        let module = "#!/usr/bin/env node\n/*---\nflags: [module, raw]\n---*/\nvar x = 1;\n";
        assert!(matches!(
            classify("test/x.js", module),
            Classification::Grade {
                runs: GradedRuns::Module,
                ..
            }
        ));
    }

    /// A single-run test takes the directive only when it is the `onlyStrict`
    /// one — a `module` test is strict by its goal, and the sloppy runs are
    /// graded verbatim. (A two-run test's strict half reaches `strict_source`
    /// directly, which is the same transform.)
    #[test]
    fn graded_source_prefixes_only_strict() {
        let content = "var x = 1;\n";
        assert_eq!(
            graded_source(content, GradedRuns::StrictScript),
            strict_source(content)
        );
        assert_eq!(
            strict_source(content),
            format!("\"use strict\";\n{content}")
        );
        assert_eq!(graded_source(content, GradedRuns::SloppyScript), content);
        assert_eq!(graded_source(content, GradedRuns::BothScripts), content);
        assert_eq!(graded_source(content, GradedRuns::Module), content);
    }

    /// The manifest's `(module, strict)` pair is what `test262_compare.ts` keys its
    /// prefix on (`strict && !module`), so the mapping is pinned per run shape: a
    /// two-run test contributes its SLOPPY run, and only the prefixed single run
    /// reads `strict` without `module`.
    #[test]
    fn manifest_mode_names_the_graded_source() {
        assert_eq!(manifest_mode(GradedRuns::Module), (true, true));
        assert_eq!(manifest_mode(GradedRuns::StrictScript), (false, true));
        assert_eq!(manifest_mode(GradedRuns::SloppyScript), (false, false));
        assert_eq!(manifest_mode(GradedRuns::BothScripts), (false, false));
    }
}
