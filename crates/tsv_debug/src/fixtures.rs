/// Helpers for managing test fixtures
use std::fs;
use std::path::{Path, PathBuf};

/// Canonical error message used in expected_svelte.json when Svelte's parser fails
///
/// When Svelte's parser cannot parse a fixture (e.g., spec-compliant CSS that Svelte doesn't support),
/// the expected_svelte.json file contains this error marker instead of an AST.
pub const EXPECTED_SVELTE_ERROR_MARKER: &str = "failed to parse";

/// Canonical error JSON format for expected_svelte.json files
///
/// This is the complete JSON content (with trailing newline) written to expected_svelte.json
/// when Svelte's parser fails to parse the input.
pub const EXPECTED_SVELTE_ERROR_JSON: &str = "{\"error\": \"failed to parse\"}\n";

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
    pub fn has_expected_ours(&self) -> bool {
        self.expected_ours_path().exists()
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
/// (excludes unformatted_ours_*.svelte files, which are handled separately)
pub fn discover_unformatted_variants(fixture_dir: &Path) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("unformatted_")
                && !filename.starts_with("unformatted_ours_")
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

/// Discover unformatted_ours_*.svelte variant files in a fixture directory
/// These files test OUR formatter's normalization capability in _prettier_divergence directories
/// where prettier validation is skipped.
pub fn discover_unformatted_ours_variants(fixture_dir: &Path) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("unformatted_ours_")
                && filename.ends_with(".svelte")
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
/// 1. `input.svelte` exists for every fixture
/// 2. `unformatted_*.svelte` variants are NOT identical to `input.svelte`
/// 3. `unformatted_*.svelte` variants use same extension as `input.svelte`
/// 4. `expected.json` OR (`expected_ours.json` + `expected_svelte.json`) exists (required for parser tests)
/// 5. `expected.json` cannot coexist with `expected_*.json` files
/// 6. `output_prettier.svelte` differs from `input.svelte` (no dead files)
/// 7. `prettier_quirk_*.svelte` variants differ from `input.svelte`
/// 8. Directory name must end with `_prettier_divergence` when prettier validation should be skipped:
///    - `output_prettier.svelte` exists (prettier formats input differently), OR
///    - `prettier_quirk_*.svelte` files exist (documents quirks), OR
///    - `unformatted_ours_*.svelte` files exist (tests our normalization only)
/// 9. `_prettier_divergence` directories CANNOT have `unformatted_*.svelte` files (only `unformatted_ours_*`)
/// 10. `prettier_quirk_*.svelte` files MUST be in `_prettier_divergence` directories
/// 11. `unformatted_ours_*.svelte` files MUST be in `_prettier_divergence` directories
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
                Remove expected.json or rename it to expected_svelte.json"
                    .to_string(),
            );
        }
    } else {
        // Old pattern: expected.json (required)
        if !has_expected {
            return Err("Missing expected.json (required for parser tests).\n\
                Run: deno task fixtures_update_parsed"
                .to_string());
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

    // Discover unformatted_ours_*.svelte variants (needed for S8 validation)
    let unformatted_ours_variants = discover_unformatted_ours_variants(fixture_dir);

    // S8: Check directory naming - _prettier_divergence suffix required when prettier validation should be skipped
    let has_prettier_quirk_files = !prettier_quirk_variants.is_empty();
    let has_output_prettier = output_prettier_path.exists();
    let dir_name = fixture_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let has_prettier_divergence_suffix = dir_name.ends_with("_prettier_divergence");

    // Divergence suffix is required when ANY prettier divergence files exist
    let needs_divergence_suffix = has_output_prettier
        || has_prettier_quirk_files
        || !unformatted_ours_variants.is_empty();

    if needs_divergence_suffix && !has_prettier_divergence_suffix {
        let mut reasons = Vec::new();
        if has_output_prettier {
            reasons.push("output_prettier.svelte".to_string());
        }
        if has_prettier_quirk_files {
            reasons.push(format!(
                "{} prettier_quirk_*.svelte file(s)",
                prettier_quirk_variants.len()
            ));
        }
        if !unformatted_ours_variants.is_empty() {
            reasons.push(format!(
                "{} unformatted_ours_*.svelte file(s)",
                unformatted_ours_variants.len()
            ));
        }
        let reason = reasons.join(" and ");

        return Err(format!(
            "Directory name must end with '_prettier_divergence' when prettier validation should be skipped.\n\
            Found {} but directory '{}' lacks the suffix.\n\
            This makes the 'skipped prettier validation' behavior explicit and discoverable.\n\
            Rename directory to '{}_prettier_divergence'",
            reason,
            dir_name,
            dir_name
        ));
    }

    if !needs_divergence_suffix && has_prettier_divergence_suffix {
        return Err(format!(
            "Directory name ends with '_prettier_divergence' but lacks files requiring it.\n\
            Directory '{}' should either:\n\
            - Add output_prettier.svelte (if prettier formats input differently), OR\n\
            - Add prettier_quirk_*.svelte files (if prettier has quirks to document), OR\n\
            - Add unformatted_ours_*.svelte files (if testing our formatter only), OR\n\
            - Remove '_prettier_divergence' suffix from directory name (if testing both formatters)",
            dir_name
        ));
    }

    // S9: _prettier_divergence directories CANNOT have unformatted_*.svelte files (only unformatted_ours_*)
    if has_prettier_divergence_suffix && !unformatted_variants.is_empty() {
        let renamed_files = unformatted_variants
            .iter()
            .map(|f| format!("  {} → {}", f, f.replace("unformatted_", "unformatted_ours_")))
            .collect::<Vec<_>>()
            .join("\n");

        let dir_without_suffix = dir_name.trim_end_matches("_prettier_divergence");

        return Err(format!(
            "Directory with '_prettier_divergence' suffix cannot have unformatted_*.svelte files.\n\
            Found {} unformatted_*.svelte file(s) in directory '{}'.\n\
            \n\
            Choose one solution:\n\
            \n\
            Option 1: Rename files to unformatted_ours_*.svelte (if testing our formatter only)\n\
            {}\n\
            \n\
            Option 2: Remove '_prettier_divergence' suffix from directory (if testing both formatters)\n\
            - Rename directory: '{}' → '{}'\n\
            - This will enable prettier validation for these files",
            unformatted_variants.len(),
            dir_name,
            renamed_files,
            dir_name,
            dir_without_suffix
        ));
    }

    // Check unformatted_ours_*.svelte variants
    for variant_name in &unformatted_ours_variants {
        // Check extension is .svelte
        if !variant_name.ends_with(".svelte") {
            return Err(format!(
                "unformatted_ours_*.svelte variant '{}' must have .svelte extension",
                variant_name
            ));
        }

        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Must differ from input.svelte
        if variant_content == input_content {
            return Err(format!(
                "unformatted_ours_*.svelte variant '{}' is identical to input.svelte (should be different for testing normalization)",
                variant_name
            ));
        }
    }

    // S11: unformatted_ours_*.svelte files MUST be in _prettier_divergence directories
    if !has_prettier_divergence_suffix && !unformatted_ours_variants.is_empty() {
        return Err(format!(
            "unformatted_ours_*.svelte files can only exist in '_prettier_divergence' directories.\n\
            Found {} unformatted_ours_*.svelte file(s) in directory '{}'.\n\
            Either:\n\
            - Rename directory to '{}_prettier_divergence' (if prettier validation should be skipped)\n\
            - Rename files to unformatted_*.svelte (if prettier validation should run)",
            unformatted_ours_variants.len(),
            dir_name,
            dir_name
        ));
    }

    // Check if README.md should exist (D1 validation)
    let has_parser_divergence = has_expected_ours && has_expected_svelte;
    let has_formatter_divergence = output_prettier_path.exists();
    let has_prettier_quirks = !prettier_quirk_variants.is_empty();

    let needs_readme = has_parser_divergence || has_formatter_divergence || has_prettier_quirks;
    let readme_path = fixture_dir.join("README.md");

    if needs_readme && !readme_path.exists() {
        let mut reasons = Vec::new();
        if has_parser_divergence {
            reasons.push("- Parser divergence (expected_ours.json + expected_svelte.json)");
        }
        if has_formatter_divergence {
            reasons.push("- Formatter divergence (output_prettier.svelte)");
        }
        if has_prettier_quirks {
            reasons.push("- Prettier quirks (prettier_quirk_*.svelte)");
        }

        return Err(format!(
            "README.md required when quirks/divergences exist.\n\
            This fixture has:\n\
            {}\n\
            README.md should document WHY we differ and provide context.\n\
            See docs/fixtures.md for README requirements.",
            reasons.join("\n")
        ));
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

/// Format content using our formatter
///
/// Determines file type from filepath extension and calls the appropriate formatter.
/// Only supports .svelte files currently.
pub fn format_with_our_formatter(content: &str, filepath: &str) -> Result<String, String> {
    if filepath.ends_with(".svelte") {
        let ast =
            tsv_svelte::parse(content).map_err(|e| format!("Format error (parse): {:?}", e))?;
        Ok(tsv_svelte::format(&ast, content))
    } else {
        Err(format!(
            "Unsupported file type for formatting: {}",
            filepath
        ))
    }
}

/// Parse content using our parser and return JSON AST
///
/// Determines file type from filepath extension and calls the appropriate parser.
/// Only supports .svelte files currently.
pub fn parse_with_our_parser(content: &str, filepath: &str) -> Result<serde_json::Value, String> {
    if filepath.ends_with(".svelte") {
        let ast = tsv_svelte::parse(content).map_err(|e| format!("Parse error: {:?}", e))?;
        let public_ast = tsv_svelte::convert_ast(&ast, content);
        serde_json::to_value(&public_ast)
            .map_err(|e| format!("Failed to serialize AST to JSON: {}", e))
    } else {
        Err(format!("Unsupported file type for parsing: {}", filepath))
    }
}

/// Read and parse JSON file
///
/// Returns the parsed JSON value for comparison with generated ASTs.
pub fn read_json_file(path: &Path) -> Result<serde_json::Value, String> {
    let content = read_file(path)?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON from {:?}: {}", path, e))
}
