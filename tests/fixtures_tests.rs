//! Unified fixture validation tests
//!
//! This single test runs ALL fixture validations that were previously split
//! across multiple test files and the fixtures_validate CLI command.
//!
//! Requires Deno to be installed for full validation.
//! Run with: cargo test --workspace --test fixtures_tests

use futures_util::stream::{self, StreamExt};
use std::path::Path;
use tsv_debug::fixtures::{self, validation};

#[tokio::test]
async fn test_all_fixtures() {
    let fixtures_dir = Path::new("tests/fixtures");
    if !fixtures_dir.exists() {
        panic!("Fixtures directory not found: tests/fixtures");
    }

    // Discover all fixtures
    let fixture_list =
        fixtures::walk_fixtures(fixtures_dir).expect("Failed to walk fixtures directory");

    if fixture_list.is_empty() {
        panic!("No fixtures found in tests/fixtures");
    }

    // Validate fixtures concurrently
    let concurrency = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(4);

    let results: Vec<_> = stream::iter(fixture_list)
        .map(|fixture| async move { validation::validate_fixture(&fixture, false).await })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    // Aggregate results
    let mut summary = validation::ValidationSummary::new();
    for result in results {
        summary.add(result);
    }

    // Check for cross-fixture duplicates
    summary.detect_cross_fixture_duplicates();

    // Get verbose mode from environment
    let verbose = std::env::var("VERBOSE").is_ok() || std::env::var("V").is_ok();

    // Print results
    validation::print_validation_results(&summary, verbose);

    // Assert all fixtures passed
    if !summary.is_valid() {
        panic!(
            "{} / {} fixtures failed validation",
            summary.failed_fixtures, summary.total_fixtures
        );
    }
}
