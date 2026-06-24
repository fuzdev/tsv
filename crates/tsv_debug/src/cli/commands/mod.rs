pub mod ast_diff;
pub mod authoring_audit;
pub mod canonical_parse;
pub mod check;
pub mod compare;
pub mod conformance_audit;
pub mod fixture_init;
pub mod fixtures_audit;
pub mod fixtures_update;
pub mod fixtures_update_formatted;
pub mod fixtures_update_parsed;
pub mod fixtures_validate;
pub mod format_prettier;
pub mod json_profile;
pub mod line_width;
pub mod metrics;
pub mod profile;
pub mod scan_audit;
pub mod swallow_audit;
pub mod test262;
pub mod ts_fixture_audit;

/// Create a tokio runtime for async operations.
///
/// Debug tools need async for Deno sidecar communication. Runtime creation
/// failure is unrecoverable, so panicking is appropriate.
#[allow(clippy::expect_used)]
pub fn create_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("Failed to create tokio runtime")
}

/// Walk `tests/fixtures`, exiting with a message on a missing directory or a walk
/// error. The shared front door for every `fixtures_*` command.
pub fn walk_or_exit() -> Vec<crate::fixtures::Fixture> {
    let fixtures_dir = std::path::Path::new("tests/fixtures");
    if !fixtures_dir.exists() {
        eprintln!("Error: fixtures directory not found: tests/fixtures");
        std::process::exit(1);
    }
    crate::fixtures::walk_fixtures(fixtures_dir).unwrap_or_else(|e| {
        eprintln!("Error walking fixtures: {e}");
        std::process::exit(1);
    })
}

/// Walk and filter by the given patterns, exiting with the standard "no matches"
/// message when nothing matches. Returns `(matches, total_before_filter)` — the
/// total feeds the "matched N of M" summaries. Commands that scope further (e.g.
/// `fixtures_audit`'s divergence-only default) call [`walk_or_exit`] and handle the
/// filter / empty case themselves.
pub fn walk_and_filter(filters: &[String]) -> (Vec<crate::fixtures::Fixture>, usize) {
    let all = walk_or_exit();
    let total = all.len();
    let matches: Vec<_> = all
        .into_iter()
        .filter(|f| f.matches_filters(filters))
        .collect();
    if matches.is_empty() {
        if filters.is_empty() {
            eprintln!("No fixtures found");
        } else {
            eprintln!("No fixtures found matching: {}", filters.join(" "));
        }
        std::process::exit(1);
    }
    (matches, total)
}
