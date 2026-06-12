//! Helpers for managing test fixtures

pub mod audit_signature;
mod discovery;
mod model;
pub mod validation;
mod variants;

pub use audit_signature::{AUDIT_SIGNATURE_FILENAME, AuditSignature};
pub use discovery::{find_input_file, walk_fixtures};
pub use model::{
    EXPECTED_SVELTE_ERROR_JSON, Fixture, InputType, PRETTIER_NONCONVERGENT_FILENAME,
    determine_required_suffix, has_prettier_divergence_suffix, has_svelte_divergence_suffix,
};
pub use variants::FixtureFiles;

use std::fs;
use std::path::Path;
use tsv_cli::cli::format_source::format_source;

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

/// Write file contents
pub fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Failed to write file {path:?}: {e}"))
}

/// Delete file if it exists
pub fn delete_file_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Failed to delete file {path:?}: {e}"))?;
    }
    Ok(())
}

/// Format content using our formatter
///
/// Determines file type from filepath extension and calls the appropriate formatter.
/// Supports .svelte, .svelte.ts, .ts, and .css files.
pub fn format_with_our_formatter(content: &str, filepath: &str) -> Result<String, String> {
    let Some(input_type) = InputType::from_filepath(filepath) else {
        return Err(format!("Unsupported file type for formatting: {filepath}"));
    };
    format_source(content, input_type.parser_type())
        .map_err(|e| format!("Format error (parse): {e}"))
}
