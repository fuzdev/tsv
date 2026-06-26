//! test262 command - run ECMAScript conformance tests against our parser.

use crate::cli::CliError;
use crate::test262::{
    DiscoveryOptions, Manifest, TestSummary, discover_tests, format_failure, run_test,
};
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

    /// emit a JSON manifest of the graded strict subset (relative path, module
    /// flag, expected verdict, tsv verdict) to this file and exit — the input
    /// to `benches/js/diagnostics/test262_compare.ts` (tsv vs oxc-parser)
    #[argh(option)]
    emit_manifest: Option<PathBuf>,

    /// filter tests by path pattern (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

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
        let options = DiscoveryOptions::default();
        let all_tests = match discover_tests(&self.path, &options) {
            Ok(tests) => tests,
            Err(e) => {
                eprintln!("Error discovering tests: {e}");
                return Err(CliError::Failed);
            }
        };

        let total_count = all_tests.len();
        println!("Found {total_count} test files");

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

        // Manifest mode: grade the strict subset, write JSON, and exit — the
        // input to the tsv-vs-oxc differential consumer. Runs `tsv_ts::parse`
        // on every graded test, so it's about as costly as a normal run.
        if let Some(manifest_path) = self.emit_manifest.as_ref() {
            eprintln!("Grading {} tests for manifest…", filtered_tests.len());
            let manifest =
                Manifest::build(self.path.to_string_lossy().into_owned(), &filtered_tests);

            let file = std::fs::File::create(manifest_path).map_err(|e| {
                eprintln!("Error creating manifest {}: {e}", manifest_path.display());
                CliError::Failed
            })?;
            serde_json::to_writer(std::io::BufWriter::new(file), &manifest).map_err(|e| {
                eprintln!("Error writing manifest: {e}");
                CliError::Failed
            })?;

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
            if let Some(is_neg) = is_negative {
                if self.negative_only && !is_neg {
                    summary.skipped_filtered += 1;
                    continue;
                }
                if self.positive_only && is_neg {
                    summary.skipped_filtered += 1;
                    continue;
                }
            }

            // Record result
            let is_neg = is_negative.unwrap_or(false);
            summary.add(&test.relative_path, is_neg, result);
        }

        // Clear progress line
        eprintln!();
        println!();

        // Print failures if verbose or if there are failures
        if !summary.failures.is_empty() && (self.verbose || summary.total_failed() <= 20) {
            println!("Failures:");
            println!("---------");
            for (path, reason) in &summary.failures {
                println!("{path}");
                for line in format_failure(reason).lines() {
                    println!("  {line}");
                }
                println!();
            }
        } else if !summary.failures.is_empty() {
            println!(
                "Showing first 10 of {} failures (use --verbose to see all):",
                summary.failures.len()
            );
            println!();
            for (path, reason) in summary.failures.iter().take(10) {
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
            println!(
                "  Skipped:        {} (sloppy mode: {}, runtime: {}, resolution: {})",
                summary.skipped(),
                summary.skipped_sloppy_mode,
                summary.skipped_runtime,
                summary.skipped_resolution,
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

        // Exit with appropriate code
        if summary.all_passed() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}
