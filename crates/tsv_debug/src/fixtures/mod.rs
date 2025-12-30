//! Helpers for managing test fixtures

pub mod validation;

use crate::deno::PrettierParser;
use std::fs;
use std::path::{Path, PathBuf};

/// Canonical error JSON format for expected_svelte.json files
///
/// This is the complete JSON content (with trailing newline) written to expected_svelte.json
/// when Svelte's parser fails to parse the input.
pub const EXPECTED_SVELTE_ERROR_JSON: &str = "{\"error\": \"failed to parse\"}\n";

/// A test fixture with its input file
/// Type of input file for a fixture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    /// Svelte file (input.svelte) - tests code in Svelte context
    Svelte,
    /// Svelte TypeScript module (input.svelte.ts) - for runes in module files
    SvelteTs,
    /// TypeScript file (input.ts) - for file-level features like hashbang
    TypeScript,
    /// CSS file (input.css) - for standalone CSS testing
    Css,
}

impl InputType {
    /// Get the file extension for this input type
    pub const fn extension(self) -> &'static str {
        match self {
            InputType::Svelte => ".svelte",
            InputType::SvelteTs => ".svelte.ts",
            InputType::TypeScript => ".ts",
            InputType::Css => ".css",
        }
    }

    /// Get the prettier parser for this input type
    pub fn prettier_parser(self) -> PrettierParser<'static> {
        match self {
            InputType::Svelte => PrettierParser::Parser("svelte"),
            // SvelteTs uses filepath-based detection so prettier-plugin-svelte handles it
            InputType::SvelteTs => PrettierParser::Filepath("file.svelte.ts"),
            InputType::TypeScript => PrettierParser::Parser("typescript"),
            InputType::Css => PrettierParser::Parser("css"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fixture {
    /// Full path to the fixture directory
    pub path: PathBuf,
    /// Relative path from fixtures root (e.g., "svelte/elements/block_text")
    pub relative_path: String,
    /// Input filename ("input.svelte", "input.ts", or "input.css")
    ///
    /// Most fixtures use `input.svelte` to test code embedded in Svelte context.
    /// Use `input.ts` or `input.css` only for features that require file-level semantics
    /// (e.g., hashbang comments, BOM at byte 0).
    pub input_file: String,
}

impl Fixture {
    /// Get the input type for this fixture
    pub fn input_type(&self) -> InputType {
        // Check .svelte.ts before .ts (more specific match first)
        if self.input_file.ends_with(".svelte.ts") {
            InputType::SvelteTs
        } else if self.input_file.ends_with(".ts") {
            InputType::TypeScript
        } else if self.input_file.ends_with(".css") {
            InputType::Css
        } else {
            InputType::Svelte
        }
    }

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

    /// Check if this fixture is in a svelte divergence directory
    pub fn is_svelte_divergence(&self) -> bool {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(has_svelte_divergence_suffix)
    }

    /// Get the output_prettier filename (e.g., "output_prettier.svelte")
    pub fn output_prettier_filename(&self) -> &'static str {
        match self.input_type() {
            InputType::Svelte => "output_prettier.svelte",
            InputType::SvelteTs => "output_prettier.svelte.ts",
            InputType::TypeScript => "output_prettier.ts",
            InputType::Css => "output_prettier.css",
        }
    }

    /// Get the full path to output_prettier file (with correct extension for input type)
    pub fn output_prettier_path(&self) -> PathBuf {
        self.path.join(self.output_prettier_filename())
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
fn find_input_file(dir: &Path) -> Option<&'static str> {
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

/// Discover unformatted_* variant files in a fixture directory
/// (excludes unformatted_ours_* files, which are handled separately)
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("unformatted_")
                && !filename.starts_with("unformatted_ours_")
                && filename.ends_with(ext)
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

/// Discover prettier_quirk_*.svelte variant files in a fixture directory
///
/// Note: prettier_quirk files are only valid for Svelte fixtures (they document
/// Svelte-specific prettier plugin quirks).
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

/// Discover unformatted_ours_* variant files in a fixture directory
/// These files test OUR formatter's normalization capability in _prettier_divergence directories
/// where prettier validation is skipped.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_ours_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("unformatted_ours_")
                && filename.ends_with(ext)
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

/// Discover input_invalid_* files in a fixture directory
///
/// These files test that parsers correctly reject invalid syntax.
/// They should fail to parse with both our parser and the canonical parser.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_invalid_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with("input_invalid_")
                && filename.ends_with(ext)
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

/// Check if directory name indicates svelte parser divergence
/// (ends with `_svelte_divergence` or `_svelte_prettier_divergence`)
pub fn has_svelte_divergence_suffix(dir_name: &str) -> bool {
    dir_name.ends_with("_svelte_divergence") || dir_name.ends_with("_svelte_prettier_divergence")
}

/// Check if directory name indicates prettier formatter divergence
/// (ends with `_prettier_divergence` or `_svelte_prettier_divergence`)
pub fn has_prettier_divergence_suffix(dir_name: &str) -> bool {
    dir_name.ends_with("_prettier_divergence") || dir_name.ends_with("_svelte_prettier_divergence")
}

/// Determine required suffix based on divergence files present
pub fn determine_required_suffix(
    has_expected_ours: bool,
    has_expected_svelte: bool,
    has_output_prettier: bool,
    has_prettier_quirks: bool,
    has_unformatted_ours: bool,
) -> Option<&'static str> {
    let needs_svelte = has_expected_ours || has_expected_svelte;
    let needs_prettier = has_output_prettier || has_prettier_quirks || has_unformatted_ours;

    match (needs_svelte, needs_prettier) {
        (true, true) => Some("_svelte_prettier_divergence"),
        (true, false) => Some("_svelte_divergence"),
        (false, true) => Some("_prettier_divergence"),
        (false, false) => None,
    }
}

/// Validate fixture structure and conventions
///
/// Checks:
/// S1:  `input.svelte` exists for every fixture
/// S2:  `expected.json` OR (`expected_ours.json` + `expected_svelte.json`) exists
/// S3:  `expected.json` cannot coexist with `expected_*.json` files
/// S4:  `unformatted_*.svelte` variants differ from `input.svelte`
/// S5:  `prettier_quirk_*.svelte` variants differ from `input.svelte`
/// S6:  `output_prettier.svelte` differs from `input.svelte`
/// S7:  `unformatted_ours_*.svelte` variants differ from `input.svelte`
/// S8:  `_prettier_divergence` or `_svelte_prettier_divergence` suffix required when prettier divergence files exist
/// S9:  Prettier divergence dirs CANNOT have `unformatted_*.svelte` files
/// S10: `prettier_quirk_*.svelte` files MUST be in prettier divergence dirs
/// S11: `unformatted_ours_*.svelte` files MUST be in prettier divergence dirs
/// S12: `_svelte_divergence` or `_svelte_prettier_divergence` suffix required when `expected_ours.json`/`expected_svelte.json` exist
/// S13: Svelte divergence dirs MUST have BOTH `expected_ours.json` AND `expected_svelte.json`
/// S14: `expected_ours.json` MUST be in svelte divergence dirs
/// S15: `expected_svelte.json` MUST be in svelte divergence dirs
/// S16: Svelte divergence dirs CANNOT have `expected.json`
/// D1:  README.md required for divergences
pub fn validate_fixture_structure(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Get input type (validates that it's a known type)
    let input_type = fixture.input_type();
    let input_ext = input_type.extension();

    // Get directory name for suffix checks
    let dir_name = fixture_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_svelte_divergence_dir = has_svelte_divergence_suffix(dir_name);
    let is_prettier_divergence_dir = has_prettier_divergence_suffix(dir_name);

    // Check expected.json OR (expected_ours.json + expected_svelte.json) exists
    let expected_path = fixture.expected_path();
    let expected_ours_path = fixture.expected_ours_path();
    let expected_svelte_path = fixture.expected_svelte_path();

    let has_expected = expected_path.exists();
    let has_expected_ours = expected_ours_path.exists();
    let has_expected_svelte = expected_svelte_path.exists();

    // S12-S16: Svelte divergence suffix rules
    if has_expected_ours || has_expected_svelte {
        // S13: Both files must exist together
        if !has_expected_ours || !has_expected_svelte {
            return Err(
                "Found either expected_ours.json or expected_svelte.json but not both.\n\
                When using the expected_ours.json + expected_svelte.json pattern, both files must exist.\n\
                - expected_ours.json: Our parser's AST (source of truth for our tests)\n\
                - expected_svelte.json: Svelte's AST (documents the difference)\n\
                Run: deno task fixtures:update:parsed".to_string()
            );
        }

        // S14/S15: expected_ours.json and expected_svelte.json MUST be in svelte divergence dirs
        if !is_svelte_divergence_dir {
            // Determine correct suffix based on what files exist (must check prettier files too)
            let output_prettier_path = fixture.output_prettier_path();
            let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir);
            let unformatted_ours_variants =
                discover_unformatted_ours_variants(fixture_dir, input_ext);
            let suggested_suffix = determine_required_suffix(
                true, // has_expected_ours (we know this is true)
                true, // has_expected_svelte (we know this is true)
                output_prettier_path.exists(),
                !prettier_quirk_variants.is_empty(),
                !unformatted_ours_variants.is_empty(),
            )
            .unwrap_or("_svelte_divergence");

            return Err(format!(
                "expected_ours.json and expected_svelte.json can only exist in directories with '{suggested_suffix}' suffix.\n\
                Found these files in directory '{dir_name}'.\n\
                Rename directory to '{dir_name}{suggested_suffix}'"
            ));
        }

        // S3/S16: expected.json cannot coexist with expected_*.json files
        if has_expected {
            return Err(
                "expected.json cannot coexist with expected_ours.json + expected_svelte.json.\n\
                Use either:\n\
                - expected.json (default: our parser matches Svelte)\n\
                - expected_ours.json + expected_svelte.json (our parser intentionally differs)\n\
                Remove expected.json"
                    .to_string(),
            );
        }

        // S17: expected_ours.json and expected_svelte.json must have different content
        // (if they're identical, there's no divergence and the pattern is pointless)
        let expected_ours_content = read_file(&expected_ours_path)?;
        let expected_svelte_content = read_file(&expected_svelte_path)?;
        if expected_ours_content == expected_svelte_content {
            return Err(
                "expected_ours.json and expected_svelte.json are identical.\n\
                The divergence pattern is only for when our parser differs from Svelte's.\n\
                If the ASTs match, use the standard expected.json pattern instead:\n\
                1. Remove _svelte_divergence suffix from directory name\n\
                2. Delete expected_ours.json and expected_svelte.json\n\
                3. Run: deno task fixtures:update:parsed"
                    .to_string(),
            );
        }
    } else if is_svelte_divergence_dir {
        // S12-rev: Svelte divergence dir MUST have expected_ours.json + expected_svelte.json
        return Err(format!(
            "Directory '{dir_name}' has '_svelte_divergence' suffix but lacks required files.\n\
            Svelte divergence directories MUST have both:\n\
            - expected_ours.json (our parser's AST)\n\
            - expected_svelte.json (Svelte parser's AST)\n\
            Either add these files or remove the '_svelte_divergence' suffix from the directory name."
        ));
    } else {
        // Standard pattern: expected.json (required)
        if !has_expected {
            return Err("Missing expected.json (required for parser tests).\n\
                Run: deno task fixtures:update:parsed"
                .to_string());
        }
    }

    let input_content = read_file(&fixture.input_path())?;

    // Check output_prettier file (if it exists)
    let output_prettier_path = fixture.output_prettier_path();
    let output_prettier_filename = fixture.output_prettier_filename();
    if output_prettier_path.exists() {
        let output_prettier_content = read_file(&output_prettier_path)?;

        // Must differ from input (no dead files)
        if output_prettier_content == input_content {
            return Err(format!(
                "{output_prettier_filename} is identical to {} (should be deleted if identical).\n\
                {output_prettier_filename} only exists when prettier formats {} differently.",
                fixture.input_file, fixture.input_file
            ));
        }

        // Note: output_prettier validation is handled by fixtures/validation.rs
        // (see validate_formatter_prettier function)
    }

    // Check unformatted_* variants are not identical to input
    let unformatted_variants = discover_unformatted_variants(fixture_dir, input_ext);
    for variant_name in &unformatted_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        if variant_content == input_content {
            return Err(format!(
                "unformatted_*{input_ext} variant '{variant_name}' is identical to {} (should be different for testing normalization)",
                fixture.input_file
            ));
        }
    }

    // Check prettier_quirk_*.svelte variants (Svelte-only)
    let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir);
    if input_type != InputType::Svelte && !prettier_quirk_variants.is_empty() {
        return Err(format!(
            "prettier_quirk_*.svelte files are not valid for {} fixtures.\n\
            Found: {}",
            fixture.input_file,
            prettier_quirk_variants.join(", ")
        ));
    }
    for variant_name in &prettier_quirk_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Rule 3: Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "prettier_quirk_*.svelte variant '{variant_name}' is identical to input.svelte (should demonstrate a quirk)"
            ));
        }
    }

    // Discover unformatted_ours_* variants (needed for S8 validation)
    let unformatted_ours_variants = discover_unformatted_ours_variants(fixture_dir, input_ext);

    // S8: Check directory naming - prettier divergence suffix required when prettier validation should be skipped
    let has_prettier_quirk_files = !prettier_quirk_variants.is_empty();
    let has_output_prettier = output_prettier_path.exists();

    // Prettier divergence suffix is required when ANY prettier divergence files exist
    let needs_prettier_divergence =
        has_output_prettier || has_prettier_quirk_files || !unformatted_ours_variants.is_empty();

    if needs_prettier_divergence && !is_prettier_divergence_dir {
        let mut reasons = Vec::new();
        if has_output_prettier {
            reasons.push(output_prettier_filename.to_string());
        }
        if has_prettier_quirk_files {
            reasons.push(format!(
                "{} prettier_quirk_*.svelte file(s)",
                prettier_quirk_variants.len()
            ));
        }
        if !unformatted_ours_variants.is_empty() {
            reasons.push(format!(
                "{} unformatted_ours_*{} file(s)",
                unformatted_ours_variants.len(),
                input_ext
            ));
        }
        let reason = reasons.join(" and ");

        // Use determine_required_suffix to suggest the correct suffix
        let suggested_suffix = determine_required_suffix(
            has_expected_ours,
            has_expected_svelte,
            has_output_prettier,
            has_prettier_quirk_files,
            !unformatted_ours_variants.is_empty(),
        )
        .unwrap_or("_prettier_divergence");

        return Err(format!(
            "Directory name must end with '{}' when prettier validation should be skipped.\n\
            Found {} but directory '{}' lacks the suffix.\n\
            This makes the 'skipped prettier validation' behavior explicit and discoverable.\n\
            Rename directory to '{}{}'",
            suggested_suffix,
            reason,
            dir_name,
            dir_name
                .trim_end_matches("_svelte_divergence")
                .trim_end_matches("_prettier_divergence"),
            suggested_suffix
        ));
    }

    // S8-rev: Prettier divergence dir MUST have prettier divergence files
    // (but only check if it's ONLY a prettier divergence dir, not combined)
    if !needs_prettier_divergence
        && is_prettier_divergence_dir
        && !is_svelte_divergence_dir
        && dir_name.ends_with("_prettier_divergence")
    {
        return Err(format!(
            "Directory name ends with '_prettier_divergence' but lacks files requiring it.\n\
            Directory '{dir_name}' should either:\n\
            - Add {output_prettier_filename} (if prettier formats input differently), OR\n\
            - Add prettier_quirk_*.svelte files (Svelte only - if prettier has quirks to document), OR\n\
            - Add unformatted_ours_*{input_ext} files (if testing our formatter only), OR\n\
            - Remove '_prettier_divergence' suffix from directory name (if testing both formatters)"
        ));
    }

    // S9: Prettier divergence directories CANNOT have unformatted_* files (only unformatted_ours_*)
    if is_prettier_divergence_dir && !unformatted_variants.is_empty() {
        let renamed_files = unformatted_variants
            .iter()
            .map(|f| {
                format!(
                    "  {} → {}",
                    f,
                    f.replace("unformatted_", "unformatted_ours_")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Strip any divergence suffix for the suggested rename
        let dir_base = dir_name
            .trim_end_matches("_svelte_prettier_divergence")
            .trim_end_matches("_prettier_divergence")
            .trim_end_matches("_svelte_divergence");

        return Err(format!(
            "Directory with prettier divergence suffix cannot have unformatted_*{input_ext} files.\n\
            Found {} unformatted_*{input_ext} file(s) in directory '{}'.\n\
            \n\
            Choose one solution:\n\
            \n\
            Option 1: Rename files to unformatted_ours_*{input_ext} (if testing our formatter only)\n\
            {}\n\
            \n\
            Option 2: Remove prettier divergence suffix from directory (if testing both formatters)\n\
            - Rename directory to: '{}'\n\
            - This will enable prettier validation for these files",
            unformatted_variants.len(),
            dir_name,
            renamed_files,
            dir_base
        ));
    }

    // Check unformatted_ours_* variants
    for variant_name in &unformatted_ours_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "unformatted_ours_*{input_ext} variant '{variant_name}' is identical to {} (should be different for testing normalization)",
                fixture.input_file
            ));
        }
    }

    // S11: unformatted_ours_* files MUST be in prettier divergence directories (Svelte-only rule)
    // For TypeScript/CSS fixtures, unformatted_ours_* doesn't make sense (no prettier-svelte plugin)
    if input_type == InputType::Svelte
        && !is_prettier_divergence_dir
        && !unformatted_ours_variants.is_empty()
    {
        // Use determine_required_suffix to get the correct suffix
        let suggested_suffix = determine_required_suffix(
            has_expected_ours,
            has_expected_svelte,
            output_prettier_path.exists(),
            has_prettier_quirk_files,
            true, // we know unformatted_ours exists
        )
        .unwrap_or("_prettier_divergence");
        let dir_base = dir_name
            .trim_end_matches("_svelte_divergence")
            .trim_end_matches("_prettier_divergence");

        return Err(format!(
            "unformatted_ours_*.svelte files can only exist in prettier divergence directories.\n\
            Found {} unformatted_ours_*.svelte file(s) in directory '{}'.\n\
            Either:\n\
            - Rename directory to '{}{}' (if prettier validation should be skipped)\n\
            - Rename files to unformatted_*.svelte (if prettier validation should run)",
            unformatted_ours_variants.len(),
            dir_name,
            dir_base,
            suggested_suffix
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
            reasons.push(
                "- Parser divergence (expected_ours.json + expected_svelte.json)".to_string(),
            );
        }
        if has_formatter_divergence {
            reasons.push(format!(
                "- Formatter divergence ({output_prettier_filename})"
            ));
        }
        if has_prettier_quirks {
            reasons.push("- Prettier quirks (prettier_quirk_*.svelte)".to_string());
        }

        return Err(format!(
            "README.md required when quirks/divergences exist.\n\
            This fixture has:\n\
            {}\n\
            README.md should document WHY we differ and provide context.\n\
            See docs/fixture_overview.md for README requirements.",
            reasons.join("\n")
        ));
    }

    Ok(())
}

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
    if filepath.ends_with(".svelte") && !filepath.ends_with(".svelte.ts") {
        let ast = tsv_svelte::parse(content).map_err(|e| format!("Format error (parse): {e:?}"))?;
        Ok(tsv_svelte::format(&ast, content))
    } else if filepath.ends_with(".svelte.ts") || filepath.ends_with(".ts") {
        let ast = tsv_ts::parse(content).map_err(|e| format!("Format error (parse): {e:?}"))?;
        // For standalone TypeScript files, don't add trailing comma for arrow type params
        // (no Svelte template syntax disambiguation needed)
        let config = tsv_lang::PrintConfig {
            arrow_type_param_trailing_comma: false,
            ..Default::default()
        };
        Ok(tsv_ts::format_with_config(&ast, content, config))
    } else if filepath.ends_with(".css") {
        let ast = tsv_css::parse(content).map_err(|e| format!("Format error (parse): {e:?}"))?;
        Ok(tsv_css::format(&ast, content))
    } else {
        Err(format!("Unsupported file type for formatting: {filepath}"))
    }
}

/// Parse content using our parser and return JSON string with tab indentation
///
/// This matches the format used by fixtures_update_parsed, ensuring string-level
/// comparison catches both semantic and formatting differences.
/// Supports .svelte, .svelte.ts, and .ts files.
pub fn parse_with_our_parser_to_string(content: &str, filepath: &str) -> Result<String, String> {
    use tsv_cli::json_utils::to_json_with_tabs;

    if filepath.ends_with(".svelte") && !filepath.ends_with(".svelte.ts") {
        let ast = tsv_svelte::parse(content).map_err(|e| format!("Parse error: {e:?}"))?;
        let public_ast = tsv_svelte::convert_ast(&ast, content);
        let json = to_json_with_tabs(&public_ast)
            .map_err(|e| format!("Failed to serialize AST to JSON: {e}"))?;
        // Add trailing newline to match fixtures_update_parsed format
        Ok(format!("{json}\n"))
    } else if filepath.ends_with(".svelte.ts") || filepath.ends_with(".ts") {
        let ast = tsv_ts::parse(content).map_err(|e| format!("Parse error: {e:?}"))?;
        let public_ast = tsv_ts::convert_ast(&ast, content);
        let json = to_json_with_tabs(&public_ast)
            .map_err(|e| format!("Failed to serialize AST to JSON: {e}"))?;
        // Add trailing newline to match fixtures_update_parsed format
        Ok(format!("{json}\n"))
    } else {
        Err(format!("Unsupported file type for parsing: {filepath}"))
    }
}
