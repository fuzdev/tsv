pub mod arena_stats;
pub mod ast_diff;
pub mod authoring_audit;
pub mod buffer_sizes;
pub mod build_fanout_audit;
pub mod canonical_compile;
pub mod canonical_parse;
pub mod check;
pub mod compare;
pub mod compile_compare;
pub mod conformance_audit;
pub mod fixture_init;
pub mod fixtures_audit;
pub mod fixtures_update;
pub mod fixtures_update_formatted;
pub mod fixtures_update_parsed;
pub mod fixtures_validate;
pub mod format_prettier;
pub mod json_profile;
pub mod lex_diff;
pub mod line_width;
pub mod metrics;
pub mod profile;
pub mod roundtrip_audit;
pub mod scan_audit;
#[cfg(feature = "swallow_check")]
pub mod swallow_audit;
pub mod test262;
pub mod ts_fixture_audit;

use crate::cli::CliError;
use crate::fixtures::Fixture;
use futures_util::stream::{self, Stream, StreamExt};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::task::JoinError;

/// Create a tokio runtime for async operations.
///
/// Debug tools need async for Deno sidecar communication. Runtime creation
/// failure is unrecoverable, so panicking is appropriate.
#[allow(clippy::expect_used)]
pub fn create_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("Failed to create tokio runtime")
}

/// Walk `tests/fixtures`, returning [`CliError::Failed`] (after printing a
/// message) on a missing directory or a walk error. The shared front door for
/// every `fixtures_*` command.
///
/// # Errors
///
/// Returns [`CliError::Failed`] when the fixtures directory is missing or the
/// walk fails.
pub fn walk_fixtures_or_fail() -> Result<Vec<Fixture>, CliError> {
    let fixtures_dir = Path::new("tests/fixtures");
    if !fixtures_dir.exists() {
        eprintln!("Error: fixtures directory not found: tests/fixtures");
        return Err(CliError::Failed);
    }
    crate::fixtures::walk_fixtures(fixtures_dir).map_err(|e| {
        eprintln!("Error walking fixtures: {e}");
        CliError::Failed
    })
}

/// Walk and filter by the given patterns, returning [`CliError::Failed`] (after
/// the standard "no matches" message) when nothing matches. On success returns
/// `(matches, total_before_filter)` — the total feeds the "matched N of M"
/// summaries. Commands that scope further (e.g. `fixtures_audit`'s
/// divergence-only default) call [`walk_fixtures_or_fail`] and handle the filter
/// / empty case themselves.
///
/// # Errors
///
/// Returns [`CliError::Failed`] when the walk fails or no fixture matches the
/// filters.
pub fn walk_and_filter(filters: &[String]) -> Result<(Vec<Fixture>, usize), CliError> {
    filter_fixtures(walk_fixtures_or_fail()?, filters)
}

/// Filter `all` by the given patterns (OR), returning `(matches, total)` or
/// [`CliError::Failed`] (after the standard "no matches" message) when the filter
/// selects nothing. Split from [`walk_and_filter`] so the empty-match policy is
/// unit-testable without walking the tree.
fn filter_fixtures(
    all: Vec<Fixture>,
    filters: &[String],
) -> Result<(Vec<Fixture>, usize), CliError> {
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
        return Err(CliError::Failed);
    }
    Ok((matches, total))
}

/// Print the `--list` view of a fixture set: each fixture's path and input file,
/// then a `Total` (no filters) or `Matched N of M` (with filters) line. Shared by
/// the `fixtures_*` commands' list mode; `total` is the count before filtering.
pub fn print_fixture_list(fixtures: &[Fixture], filters: &[String], total: usize) {
    println!("Found fixtures:");
    for fixture in fixtures {
        println!("  {} ({})", fixture.relative_path, fixture.input_file);
    }
    if filters.is_empty() {
        println!("\nTotal: {}", fixtures.len());
    } else {
        println!("\nMatched: {} of {} fixtures", fixtures.len(), total);
    }
}

/// Recursively collect every `*.rs` file under `dir` into `out`. Shared by the
/// `metrics` and `scan_audit` commands (an unreadable directory is skipped).
pub fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The order [`spawn_fixture_stream`] yields results in.
pub enum ResultOrder {
    /// Fixture (input) order — deterministic progress lines (`buffered`).
    Fixture,
    /// Completion order — results arrive as tasks finish (`buffer_unordered`).
    Completion,
}

/// A stream of joined fixture-task results, as produced by [`spawn_fixture_stream`].
type FixtureTaskStream<T> = Pin<Box<dyn Stream<Item = Result<T, JoinError>>>>;

/// Spawn `work(fixture)` for every fixture on the bulk sidecar pool, returning a
/// stream of join results — join each with [`task_result`].
///
/// Each task is `tokio::spawn`ed so the CPU-bound Rust work (parse, format,
/// serde, diff) runs across all runtime workers: `buffered`/`buffer_unordered`
/// alone only interleaves at await points on the single stream-driving task,
/// which would serialize that work on one core. The JS side (prettier/parsers)
/// is spread over the small sidecar pool sized by `init_bulk_pool`. `order`
/// picks fixture order (deterministic progress lines) vs completion order.
///
/// The stream is boxed to unify the two combinator types behind one return; the
/// per-item vtable poll is negligible against each fixture's parse/format cost.
pub fn spawn_fixture_stream<F, Fut>(
    fixtures: Vec<Fixture>,
    order: ResultOrder,
    work: F,
) -> FixtureTaskStream<Fut::Output>
where
    F: Fn(Fixture) -> Fut + 'static,
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    let concurrency = crate::deno::init_bulk_pool();
    let tasks = stream::iter(fixtures).map(move |fixture| tokio::spawn(work(fixture)));
    match order {
        ResultOrder::Fixture => Box::pin(tasks.buffered(concurrency)),
        ResultOrder::Completion => Box::pin(tasks.buffer_unordered(concurrency)),
    }
}

/// Unwrap a joined `tokio::spawn` result, mapping a task panic to
/// [`CliError::TaskPanic`] (the message reads `fixture {what} task panicked`).
/// The shared join-error arm of the `fixtures_*` commands' concurrent drivers.
///
/// # Errors
///
/// Returns [`CliError::TaskPanic`] when the joined task panicked.
pub fn task_result<T>(joined: Result<T, JoinError>, what: &str) -> Result<T, CliError> {
    joined.map_err(|e| {
        eprintln!("fixture {what} task panicked: {e}");
        CliError::TaskPanic
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(relative_path: &str) -> Fixture {
        Fixture {
            path: relative_path.into(),
            relative_path: relative_path.to_string(),
            input_file: "input.svelte".to_string(),
        }
    }

    #[test]
    fn filter_fixtures_no_matches_without_filters_fails() {
        // Empty input → no matches → the "No fixtures found" failure path.
        assert_eq!(
            filter_fixtures(Vec::new(), &[]).err(),
            Some(CliError::Failed)
        );
    }

    #[test]
    fn filter_fixtures_no_matches_with_filters_fails() {
        assert_eq!(
            filter_fixtures(Vec::new(), &["zzz_no_match".to_string()]).err(),
            Some(CliError::Failed)
        );
    }

    #[test]
    fn filter_fixtures_empty_filters_keep_all() {
        // No filters means everything matches; total equals the input count.
        let all = vec![fixture("a/b"), fixture("c/d")];
        let got = filter_fixtures(all, &[]).map(|(matches, total)| (matches.len(), total));
        assert_eq!(got, Ok((2, 2)));
    }

    #[test]
    fn filter_fixtures_keeps_matches_case_insensitively_and_reports_total() {
        let all = vec![
            fixture("svelte/elements/block"),
            fixture("typescript/calls/chain"),
            fixture("css/at_rules/media"),
        ];
        // OR over case-insensitive substrings of relative_path; `total` is the
        // pre-filter count for the "matched N of M" summaries.
        let got = filter_fixtures(all, &["CALLS".to_string(), "css".to_string()]).map(
            |(matches, total)| {
                (
                    matches
                        .into_iter()
                        .map(|f| f.relative_path)
                        .collect::<Vec<_>>(),
                    total,
                )
            },
        );
        assert_eq!(
            got,
            Ok((
                vec![
                    "typescript/calls/chain".to_string(),
                    "css/at_rules/media".to_string()
                ],
                3
            ))
        );
    }

    #[test]
    fn task_result_passes_through_ok() {
        assert_eq!(task_result(Ok::<i32, JoinError>(5), "validation"), Ok(5));
    }

    #[test]
    fn task_result_maps_join_failure_to_task_panic() {
        // Manufacture a JoinError by aborting a never-completing task (no panic
        // trace on stderr). task_result maps any JoinError to TaskPanic — the
        // exit-code-2 contract.
        let rt = create_runtime();
        let joined = rt.block_on(async {
            let handle = tokio::spawn(std::future::pending::<i32>());
            handle.abort();
            handle.await
        });
        assert_eq!(
            task_result(joined, "validation").err(),
            Some(CliError::TaskPanic)
        );
    }
}
