//! Aggregation across fixtures and result printing, including
//! cross-fixture duplicate detection.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use super::FixtureValidation;
use super::errors::ValidationError;

/// Context for cross-fixture duplicate detection (internal use only)
#[derive(Debug, Default)]
struct DuplicateDetector {
    /// Map from content hash to list of fixture paths with that content
    input_hashes: HashMap<u64, Vec<String>>,
}

impl DuplicateDetector {
    fn record(&mut self, fixture_path: &str, content: &str, input_file: &str) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Include input file name so different file types (input.ts, input.css, input.svelte)
        // with identical content don't collide
        input_file.hash(&mut hasher);
        content.hash(&mut hasher);
        let hash = hasher.finish();

        self.input_hashes
            .entry(hash)
            .or_default()
            .push(fixture_path.to_string());
    }

    fn find_duplicates(&self) -> Vec<Vec<String>> {
        self.input_hashes
            .values()
            .filter(|paths| paths.len() > 1)
            .cloned()
            .collect()
    }
}

/// Aggregate results from validating multiple fixtures
#[derive(Debug, Default)]
pub struct ValidationSummary {
    pub total_fixtures: usize,
    pub passed_fixtures: usize,
    pub failed_fixtures: usize,
    pub total_unformatted: usize,
    pub total_unformatted_ours: usize,
    pub total_unformatted_prettier: usize,
    pub total_prettier_variant: usize,
    pub total_variant: usize,
    pub total_divergent_variant: usize,
    pub total_prettier_intermediate: usize,
    pub total_prettier_intermediate_to_variant: usize,
    pub total_invalid_syntax: usize,
    pub results: Vec<FixtureValidation>,
    pub cross_fixture_duplicates: Vec<Vec<String>>,
    pub total_undocumented_prettier: usize,
}

impl ValidationSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, result: FixtureValidation) {
        self.total_fixtures += 1;
        self.total_unformatted += result.unformatted_count;
        self.total_unformatted_ours += result.unformatted_ours_count;
        self.total_unformatted_prettier += result.unformatted_prettier_count;
        self.total_prettier_variant += result.prettier_variant_count;
        self.total_variant += result.variant_count;
        self.total_divergent_variant += result.divergent_variant_count;
        self.total_prettier_intermediate += result.prettier_intermediate_count;
        self.total_prettier_intermediate_to_variant +=
            result.prettier_intermediate_to_variant_count;
        self.total_invalid_syntax += result.invalid_syntax_count;
        self.total_undocumented_prettier += result.undocumented_prettier_outputs.len();

        if result.is_valid() {
            self.passed_fixtures += 1;
        } else {
            self.failed_fixtures += 1;
        }

        self.results.push(result);
    }

    /// Build duplicate detection from collected results
    pub fn detect_cross_fixture_duplicates(&mut self) {
        let mut detector = DuplicateDetector::default();
        for result in &self.results {
            if let Some(ref content) = result.input_content {
                let input_file = result.input_file_name.as_deref().unwrap_or("input.svelte");
                detector.record(&result.fixture_path, content, input_file);
            }
        }
        self.cross_fixture_duplicates = detector.find_duplicates();
    }

    pub fn is_valid(&self) -> bool {
        self.failed_fixtures == 0 && self.cross_fixture_duplicates.is_empty()
    }

    pub fn failed_results(&self) -> impl Iterator<Item = &FixtureValidation> {
        self.results.iter().filter(|r| r.has_errors())
    }

    /// Count fixtures that failed due to Deno sidecar shutdown
    ///
    /// A high count indicates the sidecar crashed during the test run,
    /// causing cascading failures that aren't real fixture issues.
    ///
    /// Called from `tests/fixtures_tests.rs` (root-crate integration test).
    /// The `tsv_debug` binary doesn't use this, so the `#[allow]` silences a
    /// dead_code warning that only fires in the binary build.
    #[allow(dead_code)]
    pub fn count_sidecar_failures(&self) -> usize {
        self.results
            .iter()
            .filter(|r| {
                r.errors.iter().any(|e| {
                    matches!(e, ValidationError::FormatterError(msg) | ValidationError::ParserError(msg)
                        if msg.contains("deno actor shut down") || msg.contains("sidecar crashed"))
                })
            })
            .count()
    }

    /// Count fixtures that failed due to Deno sidecar timeout
    ///
    /// A high count indicates the sidecar is hanging on certain inputs,
    /// possibly due to a bug in prettier/acorn or resource exhaustion.
    ///
    /// Called from `tests/fixtures_tests.rs` — see note on `count_sidecar_failures`.
    #[allow(dead_code)]
    pub fn count_timeout_failures(&self) -> usize {
        self.results
            .iter()
            .filter(|r| {
                r.errors.iter().any(|e| {
                    matches!(e, ValidationError::FormatterError(msg) | ValidationError::ParserError(msg)
                        if msg.contains("timed out"))
                })
            })
            .count()
    }
}

/// Print validation results with per-fixture grouping
pub fn print_validation_results(summary: &ValidationSummary, verbose: bool) {
    let failed: Vec<_> = summary.failed_results().collect();

    // In verbose mode, print all fixtures including successes
    if verbose {
        for result in &summary.results {
            if result.is_valid() {
                println!("✓ {}", result.fixture_path);
                for success in &result.successes {
                    println!("    [OK] {success}");
                }
            } else {
                eprintln!("✗ {}", result.fixture_path);
                for success in &result.successes {
                    eprintln!("    [OK] {success}");
                }
                for error in &result.errors {
                    eprintln!("    [{}] {}", error.category(), error);
                    eprintln!("           Fix: {}", error.fix_hint());
                }
            }
            println!();
        }
    } else if !failed.is_empty() {
        // Print errors grouped by fixture with enhanced context
        eprintln!();
        for result in &failed {
            eprintln!("✗ {}", result.fixture_path);

            // Group errors by category for better scanning
            let mut by_category: std::collections::BTreeMap<&str, Vec<&ValidationError>> =
                std::collections::BTreeMap::new();
            for error in &result.errors {
                by_category.entry(error.category()).or_default().push(error);
            }

            for (category, errors) in by_category {
                let show_category_header = errors.len() > 1;
                if show_category_header {
                    eprintln!("    {category}:");
                }
                for error in &errors {
                    let prefix = if show_category_header {
                        "      "
                    } else {
                        "    "
                    };
                    eprintln!("{prefix}[{}] {}", error.category(), error);

                    // Show concrete command with actual fixture path
                    let fix_hint = error.fix_hint();
                    let concrete_cmd = fix_hint
                        .replace("<pattern>", &result.fixture_path)
                        .replace("<fixture>", &result.fixture_path);
                    eprintln!("{prefix}     → {concrete_cmd}");
                }
            }
            eprintln!();
        }
    }

    // Print cross-fixture duplicates
    if !summary.cross_fixture_duplicates.is_empty() {
        eprintln!("✗ Cross-fixture duplicates detected:");
        for group in &summary.cross_fixture_duplicates {
            eprintln!("    Duplicate input.svelte content:");
            for path in group {
                eprintln!("      - {path}");
            }
        }
        eprintln!();
    }

    // Print summary
    if failed.is_empty() && summary.cross_fixture_duplicates.is_empty() {
        let mut parts = vec![format!(
            "✓ All {} fixtures validated",
            summary.total_fixtures
        )];
        let mut variant_parts = Vec::new();
        if summary.total_unformatted > 0 {
            variant_parts.push(format!("{} unformatted_*", summary.total_unformatted));
        }
        if summary.total_unformatted_ours > 0 {
            variant_parts.push(format!(
                "{} unformatted_ours_*",
                summary.total_unformatted_ours
            ));
        }
        if summary.total_unformatted_prettier > 0 {
            variant_parts.push(format!(
                "{} unformatted_prettier_*",
                summary.total_unformatted_prettier
            ));
        }
        if summary.total_prettier_variant > 0 {
            variant_parts.push(format!(
                "{} prettier_variant_*",
                summary.total_prettier_variant
            ));
        }
        if summary.total_variant > 0 {
            variant_parts.push(format!("{} variant_*", summary.total_variant));
        }
        if summary.total_divergent_variant > 0 {
            variant_parts.push(format!(
                "{} divergent_variant_*",
                summary.total_divergent_variant
            ));
        }
        if summary.total_prettier_intermediate > 0 {
            variant_parts.push(format!(
                "{} prettier_intermediate_*",
                summary.total_prettier_intermediate
            ));
        }
        if summary.total_prettier_intermediate_to_variant > 0 {
            variant_parts.push(format!(
                "{} prettier_intermediate_to_variant_*",
                summary.total_prettier_intermediate_to_variant
            ));
        }
        if summary.total_invalid_syntax > 0 {
            variant_parts.push(format!("{} input_invalid_*", summary.total_invalid_syntax));
        }
        if !variant_parts.is_empty() {
            parts.push(format!("({})", variant_parts.join(", ")));
        }
        println!("{}", parts.join(" "));

        // N10: Print undocumented Prettier outputs as informational notes
        if summary.total_undocumented_prettier > 0 {
            println!();
            println!(
                "NOTE: {} undocumented Prettier output(s):",
                summary.total_undocumented_prettier
            );
            for result in &summary.results {
                for undoc in &result.undocumented_prettier_outputs {
                    println!("  {}/", result.fixture_path);
                    let source = &undoc.source_file;
                    println!("    Prettier({source}) produces a novel stable form");
                    // Extract fixture name for command suggestion
                    let fixture_name = result
                        .fixture_path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&result.fixture_path);
                    println!("    Investigate: deno task fixtures:audit {fixture_name}");
                }
            }
        }
    } else {
        eprintln!("════════════════════");
        eprintln!();
        eprintln!(
            "{} / {} fixtures failed:",
            summary.failed_fixtures, summary.total_fixtures
        );
        eprintln!();
        for result in &failed {
            eprintln!("  ✗ {}", result.fixture_path);
        }
        eprintln!();
        eprintln!(
            "Results Summary: {} passed, {} failed out of {} total",
            summary.passed_fixtures, summary.failed_fixtures, summary.total_fixtures
        );
    }
}
