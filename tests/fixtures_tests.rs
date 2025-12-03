//! Unified fixture validation tests
//!
//! This single test runs ALL fixture validations that were previously split
//! across multiple test files and the fixtures_validate CLI command.
//!
//! Requires the fuz daemon to be running for full validation.
//! Run with: cargo test --workspace --test fixtures_tests

use std::path::Path;
use tsv_debug::fixtures::{self, validation};
use tsv_debug::fuz_client;

#[tokio::test]
async fn test_all_fixtures() {
    // Fail fast if daemon is not available
    match fuz_client::check_daemon().await {
        Ok(info) => {
            eprintln!("Using fuz daemon on port {} (pid {})", info.port, info.pid);
        }
        Err(e) => panic!(
            "Fuz daemon not available: {e}\n\
            The fixture tests require the daemon for full validation.\n\
            Hint: {}",
            e.hint()
        ),
    };

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

    // Create validation context for cross-fixture duplicate detection
    let mut context = validation::ValidationContext::new();
    let mut summary = validation::ValidationSummary::new();

    // Validate each fixture
    for fixture in &fixture_list {
        let result = validation::validate_fixture(fixture, &mut context, false).await;
        summary.add(result);
    }

    // Check for cross-fixture duplicates
    summary.cross_fixture_duplicates = context.find_duplicates();

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
