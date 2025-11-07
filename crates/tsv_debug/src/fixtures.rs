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
    /// Input filename (always "input.svelte")
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

    /// Get the full path to expected_ours.json
    pub fn expected_ours_path(&self) -> PathBuf {
        self.path.join("expected_ours.json")
    }

    /// Get the full path to expected_svelte.json
    pub fn expected_svelte_path(&self) -> PathBuf {
        self.path.join("expected_svelte.json")
    }

    /// Check if this fixture uses the expected_ours.json + expected_svelte.json pattern
    #[allow(dead_code)] // Used by tests/1_parser_tests.rs
    pub fn has_expected_ours(&self) -> bool {
        self.expected_ours_path().exists()
    }

    /// Get the full path to formatted file (always formatted.svelte)
    pub fn formatted_path(&self) -> PathBuf {
        self.path.join("formatted.svelte")
    }

    /// Get the full path to output_prettier.svelte
    pub fn output_prettier_path(&self) -> PathBuf {
        self.path.join("output_prettier.svelte")
    }

    /// Check if this fixture matches all the given filter terms
    pub fn matches_filters(&self, filters: &[String]) -> bool {
        if filters.is_empty() {
            return true;
        }
        let lower_path = self.relative_path.to_lowercase();
        filters
            .iter()
            .any(|filter| lower_path.contains(&filter.to_lowercase()))
    }

    /// Check if this fixture is marked as a Svelte parser quirk
    #[allow(dead_code)] // Used by tests/1_parser_tests.rs, not by tsv_debug binary
    pub fn is_svelte_parser_quirk(&self) -> bool {
        self.path.join(".svelte_parser_quirk").exists()
    }
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

            // Check for input.svelte in this directory
            let input_path = path.join("input.svelte");
            let found_input = if input_path.exists() {
                // Create relative path with ./{root}/ prefix
                let relative_with_prefix = format!("./{}/{}", root.display(), new_relative);

                fixtures.push(Fixture {
                    path: path.clone(),
                    relative_path: relative_with_prefix,
                    input_file: "input.svelte".to_string(),
                });
                true
            } else {
                false
            };

            // If no input file found, recurse into subdirectories
            if !found_input {
                walk_fixtures_recursive(root, &path, &new_relative, fixtures)?;
            }
        }
    }

    Ok(())
}

/// Discover unformatted_*.svelte variant files in a fixture directory
pub fn discover_unformatted_variants(fixture_dir: &Path) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("unformatted_")
                && filename.ends_with(".svelte")
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

/// Discover prettier_quirk_*.svelte variant files in a fixture directory
pub fn discover_prettier_quirk_variants(fixture_dir: &Path) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("prettier_quirk_")
                && filename.ends_with(".svelte")
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

// formatted.svelte allowlist: fixtures where input.svelte is intentionally malformed
//
// ⚠️  WARNING TO AI AGENTS: DO NOT UPDATE THIS ALLOWLIST ⚠️
//
// formatted.svelte files decouple parser tests (use malformed input) from formatter tests (need correct output).
//
// WHEN TO USE formatted.svelte: (RARE)
//   • input.svelte is intentionally malformed/weird (to test parser robustness)
//   • formatted.svelte shows the correct formatted output (to test formatter fixes it)
//   • Example: Section reordering - input has wrong order, formatted has correct order
//
// WHEN NOT TO USE formatted.svelte: (COMMON)
//   • input.svelte is already correctly formatted (the baseline)
//   • Use unformatted_*.* variants to test normalization instead
//   • Example: Hug mode - input uses hug mode, unformatted_compact tests normalization
//
// Current legitimate cases:
//   • Svelte section reordering: formatter reorders <script>, <style>, and markup sections
//     into canonical order (module script → instance script → markup → style).
//
// If you're an AI agent and think you need to add to this list:
//   1. STOP - Can input.svelte be the formatted version instead?
//   2. If yes: rename formatted.svelte → input.svelte, old input.svelte → unformatted_something.*
//   3. If no: explain to human why input.svelte must be malformed, get approval

/// Validate fixture structure and conventions
///
/// Checks:
/// 1. `input.svelte` exists for every fixture
/// 2. `unformatted_*.svelte` variants are NOT identical to `input.svelte`
/// 3. `unformatted_*.svelte` variants use same extension as `input.svelte`
/// 4. `expected.json` OR (`expected_ours.json` + `expected_svelte.json`) exists (required for parser tests)
/// 5. `expected.json` cannot coexist with `expected_*.json` files
/// 6. `output_prettier.svelte` differs from `input.svelte` (no dead files)
/// 7. `prettier_quirk_*.svelte` variants differ from `input.svelte`
pub fn validate_fixture_structure(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Check for unknown file extensions (should always be input.svelte)
    if fixture.input_file != "input.svelte" {
        return Err(format!(
            "Unknown input file type: '{}'. Only input.svelte is allowed",
            fixture.input_file
        ));
    }

    // Check expected.json OR (expected_ours.json + expected_svelte.json) exists
    let expected_path = fixture.expected_path();
    let expected_ours_path = fixture.expected_ours_path();
    let expected_svelte_path = fixture.expected_svelte_path();

    let has_expected = expected_path.exists();
    let has_expected_ours = expected_ours_path.exists();
    let has_expected_svelte = expected_svelte_path.exists();

    // New pattern: expected_ours.json + expected_svelte.json (both required)
    if has_expected_ours || has_expected_svelte {
        // If using new pattern, both files must exist
        if !has_expected_ours || !has_expected_svelte {
            return Err(
                "Found either expected_ours.json or expected_svelte.json but not both.\n\
                When using the expected_ours.json + expected_svelte.json pattern, both files must exist.\n\
                - expected_ours.json: Our parser's AST (source of truth for our tests)\n\
                - expected_svelte.json: Svelte's AST (documents the difference)\n\
                Run: deno task fixtures_update_parsed".to_string()
            );
        }

        // expected.json cannot coexist with expected_*.json files
        if has_expected {
            return Err(
                "expected.json cannot coexist with expected_ours.json + expected_svelte.json.\n\
                Use either:\n\
                - expected.json (default: our parser matches Svelte)\n\
                - expected_ours.json + expected_svelte.json (our parser intentionally differs)\n\
                Remove expected.json or rename it to expected_svelte.json".to_string()
            );
        }
    } else {
        // Old pattern: expected.json (required)
        if !has_expected {
            return Err(
                "Missing expected.json (required for parser tests).\n\
                Run: deno task fixtures_update_expected".to_string()
            );
        }
    }

    let input_content = read_file(&fixture.input_path())?;

    // Check output_prettier.svelte (if it exists)
    let output_prettier_path = fixture.output_prettier_path();
    if output_prettier_path.exists() {
        let output_prettier_content = read_file(&output_prettier_path)?;

        // Must differ from input.svelte (no dead files)
        if output_prettier_content == input_content {
            return Err(
                "output_prettier.svelte is identical to input.svelte (should be deleted if identical).\n\
                output_prettier.svelte only exists when prettier formats input.svelte differently.".to_string()
            );
        }

        // TODO: Validate output_prettier.svelte == prettier(input.svelte)
        // Requires calling prettier via deno, similar to fixtures_update_formatted
    }

    // Check unformatted_*.svelte variants are not identical to input
    let unformatted_variants = discover_unformatted_variants(fixture_dir);
    for variant_name in &unformatted_variants {
        // Check extension matches input file (should always be .svelte)
        if !variant_name.ends_with(".svelte") {
            return Err(format!(
                "unformatted_*.svelte variant '{}' must have .svelte extension (found extension doesn't match)",
                variant_name
            ));
        }

        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        if variant_content == input_content {
            return Err(format!(
                "unformatted_*.svelte variant '{}' is identical to input.svelte (should be different for testing normalization)",
                variant_name
            ));
        }
    }

    // Check prettier_quirk_*.svelte variants
    let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir);
    for variant_name in &prettier_quirk_variants {
        // Check extension is .svelte
        if !variant_name.ends_with(".svelte") {
            return Err(format!(
                "prettier_quirk_*.svelte variant '{}' must have .svelte extension",
                variant_name
            ));
        }

        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Rule 3: Must differ from input.svelte
        if variant_content == input_content {
            return Err(format!(
                "prettier_quirk_*.svelte variant '{}' is identical to input.svelte (should demonstrate a quirk)",
                variant_name
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
