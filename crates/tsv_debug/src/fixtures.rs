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
    /// Input filename (e.g., "input.svelte", "input.svelte.ts")
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

    /// Get the file extension (e.g., "svelte", "svelte.ts", "ts", "css")
    pub fn extension(&self) -> &str {
        if self.input_file.ends_with(".svelte.ts") {
            "svelte.ts"
        } else {
            Path::new(&self.input_file)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
        }
    }

    /// Determine the file type from the input filename
    pub fn file_type(&self) -> FileType {
        if self.input_file.ends_with(".svelte.ts") {
            FileType::SvelteTypeScript
        } else if self.input_file.ends_with(".svelte") {
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
    SvelteTypeScript,
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
            let input_files = ["input.svelte", "input.svelte.ts"];
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
                && (filename.ends_with(".svelte.ts")
                    || filename.ends_with(".ts")
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

// formatted.* allowlist: fixtures where input.* is intentionally malformed
//
// ⚠️  WARNING TO AI AGENTS: DO NOT UPDATE THIS ALLOWLIST ⚠️
//
// formatted.* files decouple parser tests (use malformed input) from formatter tests (need correct output).
//
// WHEN TO USE formatted.*: (RARE)
//   • input.* is intentionally malformed/weird (to test parser robustness)
//   • formatted.* shows the correct formatted output (to test formatter fixes it)
//   • Example: Section reordering - input has wrong order, formatted has correct order
//
// WHEN NOT TO USE formatted.*: (COMMON)
//   • input.* is already correctly formatted (the baseline)
//   • Use unformatted_*.* variants to test normalization instead
//   • Example: Hug mode - input uses hug mode, unformatted_compact tests normalization
//
// Current legitimate cases:
//   • Svelte section reordering: formatter reorders <script>, <style>, and markup sections
//     into canonical order (module script → instance script → markup → style).
//
// If you're an AI agent and think you need to add to this list:
//   1. STOP - Can input.* be the formatted version instead?
//   2. If yes: rename formatted.* → input.*, old input.* → unformatted_something.*
//   3. If no: explain to human why input.* must be malformed, get approval
//
const FORMATTED_FILE_ALLOWLIST: &[&str] = &[
    // Section reordering fixtures (formatter reorders script/style/markup sections)
    "svelte/sections/ordering/ordering_1_instance_module_style",
    "svelte/sections/ordering/ordering_2_instance_style_module",
    "svelte/sections/ordering/ordering_3_module_instance_style",
    "svelte/sections/ordering/ordering_4_module_style_instance",
    "svelte/sections/ordering/ordering_5_style_instance_module",
    "svelte/sections/ordering/ordering_6_style_module_instance",
];

/// Validate fixture structure and conventions
///
/// Checks:
/// 1. `input.*` exists for every fixture
/// 2. Only `.svelte` and `.svelte.ts` input files are allowed (no standalone `.ts` or `.css`)
/// 3. `unformatted_*` variants are NOT identical to `input.*`
/// 4. `formatted.*` files exist only when different from `input.*`
/// 5. `expected.json` exists (required for parser tests)
/// 6. `unformatted_*` variants do NOT coexist with `formatted.*` (redundant)
pub fn validate_fixture_structure(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Block standalone .ts and .css files - only .svelte and .svelte.ts are allowed
    if fixture.input_file.ends_with(".ts") && !fixture.input_file.ends_with(".svelte.ts") {
        return Err(
            "Standalone .ts files are not allowed. Use .svelte with <script> tags or .svelte.ts for Svelte TypeScript modules."
                .to_string(),
        );
    }
    if fixture.input_file.ends_with(".css") {
        return Err(
            "Standalone .css files are not allowed. Use .svelte with <style> tags.".to_string(),
        );
    }

    // Check for unknown file extensions
    if fixture.file_type() == FileType::Unknown {
        return Err(format!(
            "Unknown input file type: '{}'. Allowed: input.svelte, input.svelte.ts",
            fixture.input_file
        ));
    }

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
        // Check if this fixture is in the allowlist
        // Strip './tests/fixtures/' prefix from relative_path for comparison
        let normalized_path = fixture
            .relative_path
            .strip_prefix("./tests/fixtures/")
            .unwrap_or(&fixture.relative_path);

        let is_allowed = FORMATTED_FILE_ALLOWLIST
            .iter()
            .any(|allowed| normalized_path == *allowed);

        if !is_allowed {
            return Err(format!(
                "formatted.{} exists but fixture '{}' is not in FORMATTED_FILE_ALLOWLIST.\n\
                \n\
                formatted.* should only exist when input.* is intentionally malformed to test parser robustness.\n\
                Most alternative formatting patterns should use unformatted_*.{} instead.\n\
                \n\
                Consider swapping the files:\n\
                  mv input.{} unformatted_some_case.{}\n\
                  mv formatted.{} input.{}\n\
                  deno task fixtures_update_expected\n\
                \n\
                To add to allowlist: ./crates/tsv_debug/src/fixtures.rs",
                fixture.extension(),
                fixture.relative_path,
                fixture.extension(),
                fixture.extension(),
                fixture.extension(),
                fixture.extension(),
                fixture.extension()
            ));
        }

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
