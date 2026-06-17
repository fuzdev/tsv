pub mod ast_diff;
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
