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

// multi_thread flavor: the default current-thread test runtime would serialize
// the CPU-bound Rust work in the spawned per-fixture tasks below
#[tokio::test(flavor = "multi_thread")]
async fn test_all_fixtures() {
    let fixtures_dir = Path::new("tests/fixtures");
    assert!(
        fixtures_dir.exists(),
        "Fixtures directory not found: tests/fixtures"
    );

    // Bulk workload: spread the JS work across a small sidecar pool (init must
    // happen before the first sidecar call — deno::check() below spawns the pool).
    let concurrency = tsv_debug::deno::init_bulk_pool();

    // Verify Deno sidecar is healthy before running tests
    // This prevents cascading failures if the sidecar dies during tests
    if let Err(e) = tsv_debug::deno::check().await {
        panic!(
            "Deno sidecar health check failed: {e}\n\n\
            Hint: {}\n\n\
            The Deno sidecar is required for fixture validation.\n\
            All {} fixture tests would fail without it.",
            e.hint(),
            fixtures::walk_fixtures(fixtures_dir)
                .map(|f| f.len())
                .unwrap_or(0)
        );
    }

    // Discover all fixtures
    let fixture_list =
        fixtures::walk_fixtures(fixtures_dir).expect("Failed to walk fixtures directory");

    assert!(
        !fixture_list.is_empty(),
        "No fixtures found in tests/fixtures"
    );

    // Validate fixtures in parallel — tokio::spawn per fixture so the
    // CPU-bound Rust work runs on all runtime workers (buffer_unordered alone
    // only interleaves at await points on the stream-driving task)
    let results: Vec<_> = stream::iter(fixture_list)
        .map(|fixture| {
            tokio::spawn(async move { validation::validate_fixture(&fixture, false).await })
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    // Aggregate results (failure diffs are buffered per fixture — print them
    // here so `cargo test` output keeps the diagnostic detail)
    let mut summary = validation::ValidationSummary::new();
    for joined in results {
        let result = joined.expect("fixture validation task panicked");
        if !result.diff_output.is_empty() {
            eprint!("{}", result.diff_output);
        }
        summary.add(result);
    }

    // Check for cross-fixture duplicates
    summary.detect_cross_fixture_duplicates();

    // Detect Deno sidecar crash pattern (many "deno actor shut down" errors)
    let sidecar_failures = summary.count_sidecar_failures();
    assert!(
        sidecar_failures <= 5,
        "\n\nDeno sidecar crashed during test run!\n\n\
        {sidecar_failures} fixtures failed with 'deno actor shut down' errors.\n\
        This indicates the Deno process died unexpectedly during validation.\n\n\
        This is an infrastructure issue, not a fixture issue.\n\
        Try running the tests again. If this persists, check:\n\
        - Available memory\n\
        - Deno version: deno --version\n\
        - System logs for OOM killer or other process termination\n"
    );

    // Detect Deno sidecar timeout pattern (requests taking too long)
    let timeout_failures = summary.count_timeout_failures();
    assert!(
        timeout_failures == 0,
        "\n\nDeno sidecar timed out during test run!\n\n\
        {timeout_failures} fixtures failed with timeout errors.\n\
        This indicates prettier/acorn is hanging on certain inputs.\n\n\
        To identify the problematic fixture, run:\n\
        cargo run -p tsv_debug fixtures_validate --verbose 2>&1 | tee /tmp/validate.log\n\n\
        Common causes:\n\
        - Malformed input triggering infinite loop in prettier/acorn\n\
        - System under heavy load\n\
        - Resource exhaustion\n"
    );

    // Get verbose mode from environment
    let verbose = std::env::var("VERBOSE").is_ok() || std::env::var("V").is_ok();

    // Print results
    validation::print_validation_results(&summary, verbose);

    // Assert all fixtures passed
    assert!(
        summary.is_valid(),
        "{} / {} fixtures failed validation",
        summary.failed_fixtures,
        summary.total_fixtures
    );
}
