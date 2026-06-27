//! Execute test262 tests against tsv's parser.

#![allow(dead_code)] // Some types/variants are useful for future expansion

use super::discovery::TestFile;
use super::frontmatter;
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
    /// Test requires sloppy (non-strict) mode
    SloppyModeRequired,
    /// Test requires a syntactic proposal tsv does not implement (the named
    /// `features:` entry); see `frontmatter::UNIMPLEMENTED_FEATURES`.
    UnimplementedFeature(&'static str),
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
    pub skipped_sloppy_mode: usize,
    pub skipped_unimplemented_feature: usize,
    pub skipped_filtered: usize,
    pub failures: Vec<(String, FailureReason)>,
}

impl TestSummary {
    /// Get total skipped count (excluding user-filtered).
    pub fn skipped(&self) -> usize {
        self.skipped_runtime
            + self.skipped_resolution
            + self.skipped_no_frontmatter
            + self.skipped_sloppy_mode
            + self.skipped_unimplemented_feature
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
                SkipReason::SloppyModeRequired => self.skipped_sloppy_mode += 1,
                SkipReason::UnimplementedFeature(_) => self.skipped_unimplemented_feature += 1,
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
        /// Whether the test carries `flags: [module]`.
        module: bool,
    },
}

/// `raw` tests (`flags: [raw]`) whose parse verdict genuinely depends on sloppy
/// semantics, keyed by test262-root-relative path (`/`-separated). Per
/// test262/INTERPRETING.md a raw test runs once **in non-strict mode only**;
/// nearly all exercise mode-INDEPENDENT syntax (hashbang comments, HTML-close
/// comments, `"use strict"` directive prologues) whose accept/reject is identical
/// strict or sloppy, so a strict-only parser grades them correctly and they stay
/// graded. The entries here are the exceptions: their source uses a construct tsv
/// rejects only because it parses strict-mode-only (`with`, legacy octal), so
/// grading at strict produces a *spurious* failure. They're out of tsv's scope
/// exactly as `noStrict` is, and skipped for the same reason.
///
/// Currently one: `hashbang/use-strict.js`, where the leading `#!` makes the
/// following `"use strict"` a hashbang comment rather than a directive, leaving
/// the program sloppy so its `with ({}) {}` is legal — which tsv, being
/// strict-only, correctly rejects.
const SLOPPY_ONLY_RAW_TESTS: &[&str] = &["test/language/comments/hashbang/use-strict.js"];

/// Whether a graded path is a `raw` test that needs sloppy-mode semantics tsv
/// can't grade (see `SLOPPY_ONLY_RAW_TESTS`). Gated on the `raw` flag so a future
/// non-raw test reusing one of these paths wouldn't be silently skipped.
fn is_sloppy_only_raw(frontmatter: &frontmatter::Frontmatter, relative_path: &str) -> bool {
    frontmatter.is_raw() && {
        let normalized = relative_path.replace('\\', "/");
        SLOPPY_ONLY_RAW_TESTS.contains(&normalized.as_str())
    }
}

/// Read a test's frontmatter and decide skip-vs-grade.
///
/// tsv is strict-mode only, so sloppy (`noStrict`) tests are skipped, as are
/// runtime/resolution negatives (we only test parsing), tests requiring an
/// unimplemented syntactic proposal, and files with no frontmatter. `relative_path`
/// is the test262-root-relative path, needed only for the sloppy-by-content `raw`
/// skip (`is_sloppy_only_raw`).
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
    // Drop tests whose syntax tsv hasn't implemented (Stage-3 import proposals)
    // from the graded set: scoring them as parse failures measures scope, not a
    // conformance gap. Both polarities go — we shouldn't claim credit for
    // rejecting a negative whose feature we reject wholesale either.
    if let Some(feature) = frontmatter.requires_unimplemented_feature() {
        return Classification::Skip(SkipReason::UnimplementedFeature(feature));
    }
    // Both kinds of sloppy-mode-required test are out of tsv's strict-only scope:
    // an explicit `noStrict` declaration, and a `raw` test (non-strict mode only
    // per test262/INTERPRETING.md) that is sloppy *by content* — it uses a
    // construct tsv rejects only because it's strict-only (`with`, legacy octal),
    // so grading it at strict would be a spurious failure. The remaining raw tests
    // exercise mode-independent syntax and stay graded at their goal.
    if frontmatter.requires_sloppy_mode() || is_sloppy_only_raw(&frontmatter, relative_path) {
        return Classification::Skip(SkipReason::SloppyModeRequired);
    }
    Classification::Grade {
        is_negative_parse: frontmatter.is_negative_parse(),
        module: frontmatter.is_module(),
    }
}

/// The parse goal for a graded test. A `module`-flagged test is parsed as a
/// `Module`; everything else tsv grades (the run-both-ways default and
/// `onlyStrict`) is a strict `Script` — `await` is an ordinary identifier there,
/// and `import`/`export`/`import.meta` are syntax errors. tsv is strict under both
/// goals (sloppy `noStrict` tests, and the sloppy-by-content `raw` test, are
/// skipped above; the remaining `raw` tests are graded).
fn goal_for(module: bool) -> tsv_ts::Goal {
    if module {
        tsv_ts::Goal::Module
    } else {
        tsv_ts::Goal::Script
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
            module,
        } => (
            run_parse_test(&content, is_negative_parse, goal_for(module)),
            Some(is_negative_parse),
        ),
    }
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
#[derive(Debug, serde::Serialize)]
pub struct ManifestEntry {
    /// Path relative to the test262 root — the join key, and (joined onto
    /// `Manifest::test262_root`) where the consumer reads the source.
    pub relative_path: String,
    /// Whether the test carries `flags: [module]`. Load-bearing: it selects the
    /// parse goal on both sides of the differential. tsv grades this file at
    /// `goal_for(module)` (`module` → `Goal::Module`, else strict `Goal::Script`),
    /// and the consumer mirrors that goal in the alternative parser — so an
    /// `await`-as-identifier script test lands in `both-accept`, not `both-reject`.
    pub module: bool,
    /// Always `true` for the graded subset — tsv is strict under both goals and
    /// skips sloppy tests (`noStrict`, and the sloppy-by-content `raw` test).
    /// Emitted for transparency / future flexibility.
    pub strict: bool,
    /// What test262 expects: `accept` for positives, `reject` for parse negatives.
    pub expected: Verdict,
    /// What `tsv_ts::parse_with_goal` did on this file at `goal_for(module)`.
    pub tsv: Verdict,
}

/// Top-level differential manifest: tsv's graded strict subset plus metadata.
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
/// graded strict subset (unreadable files are also skipped).
pub fn grade_for_manifest(test: &TestFile) -> Option<ManifestEntry> {
    let content = fs::read_to_string(&test.path).ok()?;
    let Classification::Grade {
        is_negative_parse,
        module,
    } = classify(&test.relative_path, &content)
    else {
        return None;
    };

    let expected = if is_negative_parse {
        Verdict::Reject
    } else {
        Verdict::Accept
    };
    let arena = bumpalo::Bump::new();
    let tsv = match tsv_ts::parse_with_goal(&content, goal_for(module), &arena) {
        Ok(_) => Verdict::Accept,
        Err(_) => Verdict::Reject,
    };

    Some(ManifestEntry {
        relative_path: test.relative_path.clone(),
        module,
        strict: true,
        expected,
        tsv,
    })
}

/// Run a parse test and return the result.
fn run_parse_test(content: &str, is_negative_parse: bool, goal: tsv_ts::Goal) -> TestResult {
    // Try to parse the content as TypeScript/JS at the test's goal.
    // Note: test262 tests are pure ECMAScript, so we parse as TypeScript
    // (which is a superset of JS)
    let arena = bumpalo::Bump::new();
    let parse_result = tsv_ts::parse_with_goal(content, goal, &arena);

    match (parse_result, is_negative_parse) {
        // Positive test passed: parsed successfully as expected
        (Ok(_), false) => TestResult::Passed,

        // Positive test failed: should have parsed but didn't
        (Err(error), false) => {
            TestResult::Failed(FailureReason::UnexpectedParseError(format!("{error:?}")))
        }

        // Negative test passed: failed to parse as expected
        (Err(_), true) => TestResult::Passed,

        // Negative test failed: should have failed but parsed successfully
        (Ok(_), true) => TestResult::Failed(FailureReason::UnexpectedParseSuccess),
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
        FailureReason::ReadError(e) => format!("Could not read file: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test requiring an unimplemented proposal is skipped, not graded — the
    /// wiring from `frontmatter::requires_unimplemented_feature` through
    /// `classify` into the skip bucket (the actual behavior change, beyond the
    /// frontmatter predicate's own unit test).
    #[test]
    fn classify_skips_unimplemented_feature() {
        // The real shape: proposal name alongside `dynamic-import`.
        let proposal =
            "/*---\nfeatures: [source-phase-imports, dynamic-import]\n---*/\nimport.source('x');\n";
        assert!(matches!(
            classify("test/x.js", proposal),
            Classification::Skip(SkipReason::UnimplementedFeature("source-phase-imports"))
        ));

        // Plain dynamic import is implemented — graded, not skipped.
        let plain = "/*---\nfeatures: [dynamic-import]\n---*/\nimport('x');\n";
        assert!(matches!(
            classify("test/x.js", plain),
            Classification::Grade { .. }
        ));
    }

    /// The feature check runs before the sloppy-mode check, so a test that is
    /// both noStrict and a proposal attributes to the feature bucket — keeps the
    /// `unimplemented feature:` count the true scope of the unimplemented set.
    #[test]
    fn classify_feature_precedes_sloppy() {
        let both = "/*---\nfeatures: [import-defer]\nflags: [noStrict]\n---*/\n";
        assert!(matches!(
            classify("test/x.js", both),
            Classification::Skip(SkipReason::UnimplementedFeature("import-defer"))
        ));
    }

    /// A `raw` test runs in non-strict mode only, but most exercise
    /// mode-independent syntax tsv grades correctly, so a raw test is GRADED at
    /// its goal — UNLESS it is sloppy by content (in `SLOPPY_ONLY_RAW_TESTS`), in
    /// which case it's skipped like `noStrict`, out of tsv's strict-only scope.
    #[test]
    fn classify_grades_raw_but_skips_sloppy() {
        // Mode-independent raw test (a hashbang not in the sloppy-only list) — graded.
        let raw = "/*---\nflags: [raw]\n---*/\n#!/usr/bin/env node\n";
        assert!(matches!(
            classify("test/language/comments/hashbang/preceding-whitespace.js", raw),
            Classification::Grade { .. }
        ));

        // The sloppy-by-content raw test (`with` needs sloppy mode) — skipped.
        let sloppy_raw = "/*---\nflags: [raw]\n---*/\n#!\"use strict\"\nwith ({}) {}\n";
        assert!(matches!(
            classify("test/language/comments/hashbang/use-strict.js", sloppy_raw),
            Classification::Skip(SkipReason::SloppyModeRequired)
        ));
        // Same content at a non-listed path stays graded — the skip is path-keyed
        // and gated on the `raw` flag, not a content sniff.
        assert!(matches!(
            classify("test/language/comments/hashbang/preceding-hashbang.js", sloppy_raw),
            Classification::Grade { .. }
        ));

        let no_strict = "/*---\nflags: [noStrict]\n---*/\n";
        assert!(matches!(
            classify("test/x.js", no_strict),
            Classification::Skip(SkipReason::SloppyModeRequired)
        ));
    }
}
