//! Execute test262 tests against tsv's parser.

#![allow(dead_code)] // Some types/variants are useful for future expansion

use super::discovery::TestFile;
use super::frontmatter::{self, Frontmatter};
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

/// Run a single test and return the result.
pub fn run_test(test: &TestFile) -> (TestResult, Option<bool>) {
    // Read the test file
    let content = match fs::read_to_string(&test.path) {
        Ok(c) => c,
        Err(e) => {
            return (
                TestResult::Failed(FailureReason::ReadError(e.to_string())),
                None,
            );
        }
    };

    // Parse frontmatter
    let Some(frontmatter) = frontmatter::parse(&content) else {
        return (TestResult::Skipped(SkipReason::NoFrontmatter), None);
    };

    // Check if we should skip this test
    if frontmatter.is_negative_runtime() {
        return (TestResult::Skipped(SkipReason::RuntimePhase), None);
    }
    if frontmatter.is_negative_resolution() {
        return (TestResult::Skipped(SkipReason::ResolutionPhase), None);
    }
    // Skip tests that require sloppy (non-strict) mode
    // tsv parses as strict mode only (TypeScript/ES modules are always strict)
    if frontmatter.requires_sloppy_mode() {
        return (TestResult::Skipped(SkipReason::SloppyModeRequired), None);
    }

    let is_negative_parse = frontmatter.is_negative_parse();

    // Run the test
    let result = run_parse_test(&content, &frontmatter);

    (result, Some(is_negative_parse))
}

/// Run a parse test and return the result.
fn run_parse_test(content: &str, frontmatter: &Frontmatter) -> TestResult {
    let is_negative_parse = frontmatter.is_negative_parse();

    // Try to parse the content as TypeScript/JS
    // Note: test262 tests are pure ECMAScript, so we parse as TypeScript
    // (which is a superset of JS)
    let parse_result = tsv_ts::parse(content);

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
