//! test262 command - run ECMAScript conformance tests against our parser.

use crate::cli::CliError;
use crate::test262::{Manifest, TestSummary, discover_tests, format_failure, run_test};
use argh::FromArgs;
use std::path::PathBuf;

/// Validate parser against ECMAScript conformance tests.
// argh models each flag as an independent `#[argh(switch)]` bool — orthogonal
// CLI toggles, not a state machine to refactor into an enum.
#[allow(clippy::struct_excessive_bools)]
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "test262")]
pub struct Test262Command {
    /// path to test262 checkout (default: ../test262)
    #[argh(option, default = "PathBuf::from(\"../test262\")")]
    path: PathBuf,

    /// list tests only (do not run)
    #[argh(switch)]
    list: bool,

    /// show all failure details
    #[argh(switch, short = 'v')]
    verbose: bool,

    /// only run negative parse tests
    #[argh(switch)]
    negative_only: bool,

    /// only run positive parse tests
    #[argh(switch)]
    positive_only: bool,

    /// gate mode: fail only on a POSITIVE-parse regression (or a shift in the
    /// pinned positive-pass count); negative failures (the deferred early-error
    /// frontier) are reported, not gated. The release gate — `deno task
    /// conformance:test262` and publish Step 3b.
    #[argh(switch)]
    gate: bool,

    /// emit a JSON manifest of the graded subset — module, onlyStrict, sloppy
    /// and run-both-ways tests alike, one row each (relative path, module flag,
    /// strict flag, expected verdict, tsv verdict) — to this file and exit; the
    /// input to `benches/js/diagnostics/test262_compare.ts` (tsv vs oxc-parser)
    #[argh(option)]
    emit_manifest: Option<PathBuf>,

    /// filter tests by path pattern (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

/// REGRESSION PIN (exact): discovered test files on an unfiltered run — the
/// checkout is updated deliberately, so any move (a discovery bug shrinking
/// the suite, or a test262 pull growing it) must be re-pinned, never absorbed.
/// Measured 2026-07-06, ../test262 at 7153986f; same ritual as
/// `benches/js/lib/gate_counts.ts`.
const DISCOVERED_PIN: usize = 49_136;

/// REGRESSION PIN (exact): graded-subset size for an unfiltered
/// `--emit-manifest` run — a frontmatter/feature-filter change moving the
/// graded set silently shifts the differential and the bench corpus.
/// Measured 2026-09-08, ../test262 at 7153986f.
const GRADED_MANIFEST_PIN: usize = 48_274;

/// REGRESSION PIN (exact): positive-parse pass count on an unfiltered `--gate`
/// run — the drop-in property the release gate enforces (tsv rejects no valid
/// syntax; every graded positive parses). Exact like `DISCOVERED_PIN`: a drop is
/// a regression or a positive silently reclassified as skipped, a rise is a
/// test262 pull or a grading change — both re-pinned deliberately. Measured
/// 2026-09-08, ../test262 at 7153986f. Mirrors `TEST262_POSITIVES_PIN` in
/// benches/js/lib/gate_counts.ts (the harvest's positive-file count).
const POSITIVE_PASSED_PIN: usize = 43_739;

impl Test262Command {
    pub(crate) fn run(self) -> Result<(), CliError> {
        println!("test262 validation");
        println!("==================");
        println!("Path: {}", self.path.display());
        println!();

        // Check if path exists
        if !self.path.exists() {
            eprintln!(
                "Error: test262 directory not found: {}",
                self.path.display()
            );
            eprintln!();
            eprintln!("To clone the test262 repository:");
            eprintln!("  cd .. && git clone https://github.com/tc39/test262.git");
            eprintln!();
            eprintln!("Or specify a custom path:");
            eprintln!("  cargo run -p tsv_debug test262 --path /path/to/test262");
            return Err(CliError::Failed);
        }

        // Discover tests
        let all_tests = match discover_tests(&self.path) {
            Ok(tests) => tests,
            Err(e) => {
                eprintln!("Error discovering tests: {e}");
                return Err(CliError::Failed);
            }
        };

        let total_count = all_tests.len();
        println!("Found {total_count} test files");

        if self.filters.is_empty() && total_count != DISCOVERED_PIN {
            eprintln!(
                "Error: pinned count mismatch — discovered {total_count} test files ≠ pinned {DISCOVERED_PIN}. \
                 If deliberate (test262 pull, discovery change), re-pin DISCOVERED_PIN."
            );
            return Err(CliError::Failed);
        }

        // Apply filters
        let filtered_tests: Vec<_> = all_tests
            .into_iter()
            .filter(|t| t.matches_filters(&self.filters))
            .collect();

        if filtered_tests.is_empty() {
            if self.filters.is_empty() {
                eprintln!("No tests found");
            } else {
                eprintln!("No tests found matching: {}", self.filters.join(" "));
            }
            return Err(CliError::Failed);
        }

        if !self.filters.is_empty() {
            println!(
                "Filtered to {} tests matching: {}",
                filtered_tests.len(),
                self.filters.join(" ")
            );
        }
        println!();

        // Manifest mode: grade the same subset `run_test` grades, write JSON,
        // and exit — the input to the tsv-vs-oxc differential consumer. Runs
        // `tsv_ts::parse` on every graded test, so it's about as costly as a
        // normal run.
        if let Some(manifest_path) = self.emit_manifest.as_ref() {
            eprintln!("Grading {} tests for manifest…", filtered_tests.len());
            let manifest =
                Manifest::build(self.path.to_string_lossy().into_owned(), &filtered_tests);

            // Pin BEFORE writing, so a wrong manifest never replaces a good one.
            if self.filters.is_empty() && manifest.count != GRADED_MANIFEST_PIN {
                eprintln!(
                    "Error: pinned count mismatch — graded {} tests ≠ pinned {GRADED_MANIFEST_PIN}; \
                     manifest not written. If deliberate, re-pin GRADED_MANIFEST_PIN.",
                    manifest.count
                );
                return Err(CliError::Failed);
            }

            super::write_manifest_json(manifest_path, &manifest)?;

            println!(
                "Wrote {} graded tests to {}",
                manifest.count,
                manifest_path.display()
            );
            return Ok(());
        }

        // List only mode
        if self.list {
            println!("Test files:");
            for test in &filtered_tests {
                println!("  {}", test.relative_path);
            }
            println!("\nTotal: {}", filtered_tests.len());
            return Ok(());
        }

        // Run tests
        let mut summary = TestSummary::default();
        let test_count = filtered_tests.len();
        let mut processed = 0;

        for test in &filtered_tests {
            processed += 1;

            // Progress indicator (every 1000 tests or at end)
            if processed % 1000 == 0 || processed == test_count {
                eprint!("\rProcessing: {processed}/{test_count}");
            }

            let (result, is_negative) = run_test(test);

            // Apply negative/positive filters
            if let Some(is_neg) = is_negative
                && ((self.negative_only && !is_neg) || (self.positive_only && is_neg))
            {
                continue;
            }

            // Record result
            let is_neg = is_negative.unwrap_or(false);
            summary.add(&test.relative_path, is_neg, result);
        }

        // Clear progress line
        eprintln!();
        println!();

        // Print failures if verbose or if there are failures
        if !summary.failures.is_empty() {
            let show_all = self.verbose || summary.total_failed() <= 20;
            let shown = if show_all {
                println!("Failures:");
                println!("---------");
                summary.failures.len()
            } else {
                println!(
                    "Showing first 10 of {} failures (use --verbose to see all):",
                    summary.failures.len()
                );
                println!();
                10
            };
            for (path, reason) in summary.failures.iter().take(shown) {
                println!("{path}");
                for line in format_failure(reason).lines() {
                    println!("  {line}");
                }
                println!();
            }
        }

        // Print summary
        println!("Results:");
        println!(
            "  Positive tests: {} passed, {} failed",
            summary.positive_passed, summary.positive_failed
        );
        println!(
            "  Negative tests: {} passed, {} failed",
            summary.negative_passed, summary.negative_failed
        );
        if summary.skipped() > 0 {
            // Every reason `TestSummary::skipped` sums, so the parts always add up
            // to the total; a reason that never fired is dropped rather than
            // printed as a zero, so the line names only what actually happened.
            let reasons: Vec<String> = [
                ("Annex B", summary.skipped_annex_b),
                (
                    "unimplemented feature",
                    summary.skipped_unimplemented_feature,
                ),
                ("runtime", summary.skipped_runtime),
                ("resolution", summary.skipped_resolution),
                ("no frontmatter", summary.skipped_no_frontmatter),
                (
                    "strict-prefix conflict",
                    summary.skipped_strict_prefix_conflict,
                ),
            ]
            .into_iter()
            .filter(|&(_, count)| count > 0)
            .map(|(label, count)| format!("{label}: {count}"))
            .collect();
            println!(
                "  Skipped:        {} ({})",
                summary.skipped(),
                reasons.join(", ")
            );
        }
        // A strict-prefix conflict (`raw` + `onlyStrict`, `onlyStrict` + `noStrict`, a
        // byte-0 hashbang/BOM under a prefixed run): no test262 test carries one, so
        // this is silent unless the suite changes shape under us. The count also
        // rides the `Skipped:` parenthetical above so that line sums; this one is the
        // loud signal, deliberately repeated.
        if summary.skipped_strict_prefix_conflict > 0 {
            println!(
                "  ⚠ {} test(s) skipped because the harness's `\"use strict\"` prefix cannot \
                 be applied honestly (`raw` + `onlyStrict`, `onlyStrict` + `noStrict`, or a \
                 byte-0 hashbang/BOM under a prefixed run)",
                summary.skipped_strict_prefix_conflict
            );
        }
        println!();

        let total = summary.total_run();
        let passed = summary.positive_passed + summary.negative_passed;
        #[allow(clippy::cast_precision_loss)] // Test counts won't exceed f64 precision
        let pass_rate = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        println!("Pass rate: {passed}/{total} ({pass_rate:.1}%)");

        // Gate mode (--gate): scope the pass/fail verdict to the POSITIVE-parse
        // drop-in property — tsv must reject no valid syntax. The ~2.4k negative
        // failures are the deliberately-deferred early-error frontier (see
        // docs/conformance_test262.md), so a full run's non-zero exit is a
        // diagnostic, not a release regression; gate mode ignores negative_failed
        // and fails only on a positive regression or a shift in the pinned
        // positive-pass count. This is what `deno task conformance:test262` and
        // publish Step 3b run.
        if self.gate {
            let mut gate_failed = false;
            if summary.positive_failed > 0 {
                eprintln!(
                    "\nGATE FAIL: {} positive test(s) regressed — tsv rejects valid syntax \
                     (a drop-in parser regression). See the Failures above.",
                    summary.positive_failed
                );
                gate_failed = true;
            }
            // Exact pin on the positive-pass count (unfiltered, positives graded),
            // so a positive silently reclassified as SKIPPED — which
            // positive_failed==0 can't see — still trips. Same ritual as
            // DISCOVERED_PIN; re-pin deliberately on a test262 pull.
            if self.filters.is_empty()
                && !self.negative_only
                && summary.positive_passed != POSITIVE_PASSED_PIN
            {
                eprintln!(
                    "\nGATE FAIL: positive pass count {} ≠ pinned {POSITIVE_PASSED_PIN}. \
                     If deliberate (test262 pull, grading change), re-pin POSITIVE_PASSED_PIN.",
                    summary.positive_passed
                );
                gate_failed = true;
            }
            if gate_failed {
                return Err(CliError::Failed);
            }
            println!(
                "\nGATE PASS: {} positive tests, 0 failed (negatives reported, not gated).",
                summary.positive_passed
            );
            return Ok(());
        }

        // Exit with appropriate code
        if summary.all_passed() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}
