//! Fixture discovery: walking the fixtures tree and enforcing the
//! directory hierarchy rule (every directory is a fixture or a container).

use crate::fixtures::Fixture;
use std::fs;
use std::path::Path;

/// Walk the fixtures directory and collect all fixtures
///
/// # Arguments
/// * `fixtures_dir` - Path to the fixtures directory (e.g., "tests/fixtures")
///
/// # Returns
/// A vector of all discovered fixtures
///
/// # Errors
/// Returns an error if any directory violates the hierarchy rules:
/// - Has both an input file AND subdirectories (must be one or the other)
/// - Has neither an input file nor subdirectories (orphan directory)
pub fn walk_fixtures(fixtures_dir: &Path) -> Result<Vec<Fixture>, String> {
    let mut fixtures = Vec::new();
    walk_fixtures_recursive(fixtures_dir, fixtures_dir, "", &mut fixtures)?;
    Ok(fixtures)
}

/// Check if a directory has any subdirectories
fn has_subdirectories(dir: &Path) -> bool {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().is_dir())
}

/// Get list of subdirectory names in a directory
fn get_subdirectory_names(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

/// Find the input file in a directory, if any
///
/// Prefers input.svelte, falls back to input.svelte.ts, input.ts, or input.css.
pub fn find_input_file(dir: &Path) -> Option<&'static str> {
    if dir.join("input.svelte").exists() {
        Some("input.svelte")
    } else if dir.join("input.svelte.ts").exists() {
        Some("input.svelte.ts")
    } else if dir.join("input.ts").exists() {
        Some("input.ts")
    } else if dir.join("input.css").exists() {
        Some("input.css")
    } else {
        None
    }
}

fn walk_fixtures_recursive(
    root: &Path,
    current: &Path,
    relative_base: &str,
    fixtures: &mut Vec<Fixture>,
) -> Result<(), String> {
    let entries =
        fs::read_dir(current).map_err(|e| format!("Failed to read directory {current:?}: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| format!("Invalid directory name: {path:?}"))?;

            let new_relative = if relative_base.is_empty() {
                dir_name.to_string()
            } else {
                format!("{relative_base}/{dir_name}")
            };

            // Check for input file and subdirectories
            let input_file = find_input_file(&path);
            let has_subdirs = has_subdirectories(&path);

            match (input_file, has_subdirs) {
                (Some(input_file), true) => {
                    // ERROR: has both input file and subdirectories
                    let subdir_names = get_subdirectory_names(&path);
                    return Err(format!(
                        "Directory has both input file AND subdirectories: {}\n\
                        Each directory must have EITHER:\n\
                        - An input file (input.svelte, input.svelte.ts, input.ts, or input.css) making it a fixture, OR\n\
                        - Subdirectories making it a container\n\
                        \n\
                        Found: {input_file} AND subdirectories: {}\n\
                        \n\
                        To fix, either:\n\
                        - Move the input file and related fixture files into a subdirectory, OR\n\
                        - Move the subdirectories to a different location",
                        path.display(),
                        subdir_names.join(", "),
                    ));
                }
                (Some(input_file), false) => {
                    // Valid fixture directory (has input file, no subdirectories)
                    let relative_with_prefix = format!("./{}/{}", root.display(), new_relative);
                    fixtures.push(Fixture {
                        path: path.clone(),
                        relative_path: relative_with_prefix,
                        input_file: input_file.to_string(),
                    });
                }
                (None, true) => {
                    // Valid container directory (no input file, has subdirectories) - recurse
                    walk_fixtures_recursive(root, &path, &new_relative, fixtures)?;
                }
                (None, false) => {
                    // ERROR: orphan directory (no input file, no subdirectories)
                    return Err(format!(
                        "Orphan directory (has neither input file nor subdirectories): {}\n\
                        Each directory must have EITHER:\n\
                        - An input file (input.svelte, input.svelte.ts, input.ts, or input.css) making it a fixture, OR\n\
                        - Subdirectories making it a container\n\
                        \n\
                        To fix, either:\n\
                        - Add an input file to make it a fixture, OR\n\
                        - Delete the orphan directory",
                        path.display(),
                    ));
                }
            }
        }
    }

    Ok(())
}
