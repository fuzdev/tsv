//! Helpers for managing test fixtures

pub mod audit_signature;
mod discovery;
mod model;
pub mod validation;
mod variants;

pub use audit_signature::{
    AUDIT_SIGNATURE_FILENAME, AuditSignature, ChainAnchor, ChainWalk, SingleFormPins,
    audit_signature_variant_filename, audit_signature_variant_suffix,
};
pub use discovery::{find_input_file, walk_fixtures};
pub use model::{
    EXPECTED_SVELTE_ERROR_JSON, Fixture, GOAL_FILENAME, InputType, PRETTIER_NONCONVERGENT_FILENAME,
    PRETTIER_REJECTS_FILENAME, TSV_REJECTS_FILENAME, determine_required_suffix, goal_marker_path,
    has_prettier_divergence_suffix, has_svelte_divergence_suffix, read_goal_marker,
};
pub use variants::{
    FixtureFiles, StableFormMarker, classify_stable_form, unformatted_ours_filename,
    unformatted_ours_suffix,
};

use std::fs;
use std::path::Path;
use tsv_cli::cli::format_source::format_source_with_source_type;

/// Recursively remove location/span fields from JSON for AST comparison
pub fn remove_locations(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Object(map) => {
            map.remove("start");
            map.remove("end");
            map.remove("loc");
            for v in map.values_mut() {
                *v = remove_locations(std::mem::take(v));
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                *v = remove_locations(std::mem::take(v));
            }
        }
        _ => {}
    }
    value
}

/// Read file contents
pub fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read file {path:?}: {e}"))
}

/// Create a fixture directory, refusing the wrong-cwd footgun first.
///
/// A relative fixture path resolved from a repo *subdirectory* silently creates a
/// NESTED fixture root (`tests/fixtures/svelte/tests/fixtures/…`) — `create_dir_all`
/// makes the whole chain without complaint and the stray tree only surfaces later as
/// unmatched fixtures. A resolved target whose absolute path contains more than one
/// `tests/fixtures` (or `tests/fixtures_compile`) component run is never intended, so
/// refuse it with the actual fix.
///
/// The check and the `create_dir_all` are one function so the two `*fixture_init`
/// commands cannot acquire the directory by different routes — a caller that reached
/// for `create_dir_all` alone would be back to creating the nested tree.
pub fn create_fixture_dir(dir: &Path) -> Result<(), String> {
    let absolute = match std::env::current_dir() {
        Ok(cwd) => cwd.join(dir),
        Err(_) => dir.to_path_buf(),
    };
    let components: Vec<&str> = absolute
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    let fixture_roots = components
        .windows(2)
        .filter(|w| w[0] == "tests" && (w[1] == "fixtures" || w[1] == "fixtures_compile"))
        .count();
    if fixture_roots > 1 {
        return Err(format!(
            "refusing to create a NESTED fixture root: {} resolves under another \
             tests/fixtures tree — run from the repo root",
            absolute.display()
        ));
    }
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory {dir:?}: {e}"))
}

/// Write file contents
pub fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Failed to write file {path:?}: {e}"))
}

/// Remove `path` if it exists, reporting whether there was a file to remove — the pair to
/// [`write_if_changed`]. One `remove_file`, with a missing file read as `false`, so there is
/// no `exists()` check to race.
///
/// # Errors
///
/// Returns the removal failure's message; a missing file is not one.
pub fn remove_if_present(path: &Path) -> Result<bool, String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("Failed to delete file {path:?}: {e}")),
    }
}

/// How [`write_if_changed`] left a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The file did not exist and was written.
    Created,
    /// The file existed with other content and was rewritten.
    Updated,
    /// The file already held exactly this content; nothing was written.
    Unchanged,
}

/// Write `content` to `path` unless the file already holds exactly that.
///
/// # Errors
///
/// Returns the write failure's message.
pub fn write_if_changed(path: &Path, content: &str) -> Result<WriteOutcome, String> {
    let existed = path.exists();
    if existed && read_file(path).is_ok_and(|existing| existing == content) {
        return Ok(WriteOutcome::Unchanged);
    }
    write_file(path, content)?;
    // Keyed on existence, not readability: a file that existed but failed to read was
    // overwritten, not created.
    Ok(if existed {
        WriteOutcome::Updated
    } else {
        WriteOutcome::Created
    })
}

/// Whether `relative_path` matches any of `filters` — case-insensitive substrings, where no
/// filters match everything. The one rule every fixture tree's pattern arguments share.
pub fn path_matches_filters(relative_path: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let lower = relative_path.to_lowercase();
    filters
        .iter()
        .any(|filter| lower.contains(&filter.to_lowercase()))
}

/// Format content using our formatter at an explicit TypeScript parse goal.
///
/// Determines file type from filepath extension and calls the appropriate formatter
/// (.svelte, .svelte.ts, .ts, and .css). The goal is consulted only for `.ts` /
/// `.svelte.ts` inputs; a fixture passes its own [`Fixture::goal`], so a
/// standalone-script (`Goal::Script`) fixture is formatted the way it is validated.
pub fn format_with_our_formatter(
    content: &str,
    filepath: &str,
    goal: tsv_ts::Goal,
) -> Result<String, String> {
    let Some(input_type) = InputType::from_filepath(filepath) else {
        return Err(format!("Unsupported file type for formatting: {filepath}"));
    };
    format_source_with_source_type(content, input_type.parser_type(), Some(goal))
        .map_err(|e| format!("Format error (parse): {e}"))
}
