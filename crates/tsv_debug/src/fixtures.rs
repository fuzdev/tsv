/// Helpers for managing test fixtures
use std::fs;
use std::path::{Path, PathBuf};

/// A test fixture with its input file
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Full path to the fixture directory
    pub path: PathBuf,
    /// Relative path from fixtures root (e.g., "svelte/elements/block_text")
    pub relative_path: String,
    /// Input filename (e.g., "input.svelte", "input.ts", "input.css")
    pub input_file: String,
}

impl Fixture {
    /// Get the full path to the input file
    pub fn input_path(&self) -> PathBuf {
        self.path.join(&self.input_file)
    }

    /// Get the full path to expected.json
    pub fn expected_path(&self) -> PathBuf {
        self.path.join("expected.json")
    }

    /// Get the full path to formatted file
    pub fn formatted_path(&self) -> PathBuf {
        self.path.join(format!("formatted.{}", self.extension()))
    }

    /// Get the file extension (e.g., "svelte", "ts", "css")
    pub fn extension(&self) -> &str {
        Path::new(&self.input_file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
    }

    /// Determine the file type from the input filename
    pub fn file_type(&self) -> FileType {
        if self.input_file.ends_with(".svelte") {
            FileType::Svelte
        } else if self.input_file.ends_with(".ts") {
            FileType::TypeScript
        } else if self.input_file.ends_with(".css") {
            FileType::Css
        } else {
            FileType::Unknown
        }
    }

    /// Check if this fixture matches all the given filter terms
    pub fn matches_filters(&self, filters: &[String]) -> bool {
        if filters.is_empty() {
            return true;
        }
        let lower_path = self.relative_path.to_lowercase();
        filters
            .iter()
            .all(|filter| lower_path.contains(&filter.to_lowercase()))
    }
}

/// File type for a fixture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Svelte,
    TypeScript,
    Css,
    Unknown,
}

/// Walk the fixtures directory and collect all fixtures
///
/// # Arguments
/// * `fixtures_dir` - Path to the fixtures directory (e.g., "tests/fixtures")
///
/// # Returns
/// A vector of all discovered fixtures
pub fn walk_fixtures(fixtures_dir: &Path) -> Result<Vec<Fixture>, String> {
    let mut fixtures = Vec::new();
    walk_fixtures_recursive(fixtures_dir, fixtures_dir, "", &mut fixtures)?;
    Ok(fixtures)
}

fn walk_fixtures_recursive(
    root: &Path,
    current: &Path,
    relative_base: &str,
    fixtures: &mut Vec<Fixture>,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|e| format!("Failed to read directory {:?}: {}", current, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| format!("Invalid directory name: {:?}", path))?;

            let new_relative = if relative_base.is_empty() {
                dir_name.to_string()
            } else {
                format!("{}/{}", relative_base, dir_name)
            };

            // Check for input files in this directory
            let input_files = ["input.svelte", "input.ts", "input.css"];
            let mut found_input = false;

            for input_file in &input_files {
                let input_path = path.join(input_file);
                if input_path.exists() {
                    // Create relative path with ./{root}/ prefix
                    let relative_with_prefix = format!("./{}/{}", root.display(), new_relative);

                    fixtures.push(Fixture {
                        path: path.clone(),
                        relative_path: relative_with_prefix,
                        input_file: input_file.to_string(),
                    });
                    found_input = true;
                    break; // Only one input file per fixture
                }
            }

            // If no input file found, recurse into subdirectories
            if !found_input {
                walk_fixtures_recursive(root, &path, &new_relative, fixtures)?;
            }
        }
    }

    Ok(())
}

/// Discover unformatted_* variant files in a fixture directory
pub fn discover_unformatted_variants(fixture_dir: &Path) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("unformatted_")
                && (filename.ends_with(".ts")
                    || filename.ends_with(".svelte")
                    || filename.ends_with(".css"))
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

/// Validate fixture structure and conventions
///
/// Checks:
/// 1. `input.*` exists for every fixture
/// 2. `unformatted_*` variants are NOT identical to `input.*`
/// 3. `formatted.*` files exist only when different from `input.*`
/// 4. `expected.json` exists (required for parser tests)
/// 5. `unformatted_*` variants do NOT coexist with `formatted.*` (redundant)
pub fn validate_fixture_structure(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Check expected.json exists (required for parser tests)
    let expected_path = fixture.expected_path();
    if !expected_path.exists() {
        return Err(
            "Missing expected.json (required for parser tests, run: deno task fixtures_update_expected)"
                .to_string(),
        );
    }

    let input_content = read_file(&fixture.input_path())?;

    // Check unformatted_* variants are not identical to input
    let unformatted_variants = discover_unformatted_variants(fixture_dir);
    for variant_name in &unformatted_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        if variant_content == input_content {
            return Err(format!(
                "unformatted_* variant '{}' is identical to input.{} (should be different for testing normalization)",
                variant_name,
                fixture.extension()
            ));
        }
    }

    // Check formatted.* files only exist when different from input
    let formatted_path = fixture.formatted_path();
    if formatted_path.exists() {
        let formatted_content = read_file(&formatted_path)?;

        if formatted_content == input_content {
            return Err(format!(
                "formatted.{} is identical to input.{} (should be deleted if no changes needed)",
                fixture.extension(),
                fixture.extension()
            ));
        }

        // Check that unformatted_* variants don't coexist with formatted.*
        if !unformatted_variants.is_empty() {
            return Err(format!(
                "unformatted_* variants ({}) should not coexist with formatted.{} (input.{} is already unformatted - probably rename formatted.{} → input.{} and delete or rename input.{} → unformatted_something.{})",
                unformatted_variants.join(", "),
                fixture.extension(),
                fixture.extension(),
                fixture.extension(),
                fixture.extension(),
                fixture.extension(),
                fixture.extension()
            ));
        }
    }

    Ok(())
}

/// Recursively remove location/span fields from JSON for AST comparison
#[allow(dead_code)]
pub fn remove_locations(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Object(map) => {
            map.remove("start");
            map.remove("end");
            map.remove("loc");
            for v in map.values_mut() {
                *v = remove_locations(v.clone());
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                *v = remove_locations(v.clone());
            }
        }
        _ => {}
    }
    value
}

/// Read file contents
pub fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read file {:?}: {}", path, e))
}

/// Write file contents
pub fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Failed to write file {:?}: {}", path, e))
}

/// Delete file if it exists
pub fn delete_file_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Failed to delete file {:?}: {}", path, e))?;
    }
    Ok(())
}
