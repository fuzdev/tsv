//! Helpers for managing test fixtures

pub mod audit_signature;
pub mod validation;

pub use audit_signature::{AUDIT_SIGNATURE_FILENAME, AuditSignature};

use crate::deno::PrettierParser;
use std::fs;
use std::path::{Path, PathBuf};
use tsv_cli::json_utils::to_json_with_tabs;

/// Canonical error JSON format for expected_svelte.json files
///
/// This is the complete JSON content (with trailing newline) written to expected_svelte.json
/// when Svelte's parser fails to parse the input.
pub const EXPECTED_SVELTE_ERROR_JSON: &str = "{\"error\": \"failed to parse\"}\n";

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
    /// Determine the input type from a file path by extension.
    ///
    /// The single extension-dispatch chain — every filepath→type decision
    /// goes through here so the `.svelte.ts`-before-`.ts` ordering exists
    /// once. Returns `None` for unknown extensions so callers fail loudly
    /// instead of silently misclassifying.
    pub fn from_filepath(filepath: &str) -> Option<Self> {
        if filepath.ends_with(".svelte.ts") {
            Some(InputType::SvelteTs)
        } else if filepath.ends_with(".ts") {
            Some(InputType::TypeScript)
        } else if filepath.ends_with(".svelte") {
            Some(InputType::Svelte)
        } else if filepath.ends_with(".css") {
            Some(InputType::Css)
        } else {
            None
        }
    }

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

/// A test fixture with its input file
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
        // SAFETY: input_file comes from find_input_file's closed set of
        // known input filenames
        #[allow(clippy::expect_used)]
        InputType::from_filepath(&self.input_file).expect("known fixture input filename")
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

    /// Check if this fixture is in a prettier divergence directory
    pub fn is_prettier_divergence(&self) -> bool {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(has_prettier_divergence_suffix)
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

    /// Get the full path to audit_signature.txt (sibling of output_prettier.*)
    pub fn audit_signature_path(&self) -> PathBuf {
        self.path.join(AUDIT_SIGNATURE_FILENAME)
    }

    /// Check if this fixture matches any of the given filter terms
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

/// Discover fixture files matching `prefix` (and not any of
/// `exclude_prefixes`) with the given extension, sorted by name.
///
/// Shared body of every `discover_*_variants` function. The exclusions
/// exist because some variant prefixes are prefixes of more specific
/// sibling prefixes (`unformatted_` vs `unformatted_ours_`).
fn discover_prefixed_files(
    fixture_dir: &Path,
    ext: &str,
    prefix: &str,
    exclude_prefixes: &[&str],
) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with(prefix)
                && !exclude_prefixes.iter().any(|p| filename.starts_with(p))
                && filename.ends_with(ext)
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

/// Discover unformatted_* variant files in a fixture directory
/// (excludes unformatted_ours_* files, which are handled separately)
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(
        fixture_dir,
        ext,
        "unformatted_",
        &["unformatted_ours_", "unformatted_prettier_"],
    )
}

/// Discover prettier_variant_* variant files in a fixture directory
///
/// These files document Prettier's stable variants - inputs that Prettier preserves
/// as-is rather than normalizing to a single canonical form.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_prettier_variant_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "prettier_variant_", &[])
}

/// Discover variant_* variant files in a fixture directory
///
/// These files document dual-stable forms that our formatter also keeps stable,
/// but does NOT normalize to `input`. Unlike `prettier_variant_*` (which our formatter
/// normalizes to input), these represent dual-stable forms where both formatters
/// preserve distinct canonical outputs.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_variant_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "variant_", &[])
}

/// Discover unformatted_ours_* variant files in a fixture directory
/// These files test OUR formatter's normalization capability in _prettier_divergence directories
/// where prettier validation is skipped.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_ours_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "unformatted_ours_", &[])
}

/// Discover unformatted_prettier_* variant files in a fixture directory
///
/// These files test that PRETTIER normalizes certain inputs to `output_prettier.*`.
/// Used in `_prettier_divergence` directories where `output_prettier.*` exists.
///
/// Validation rules:
/// - `prettier(unformatted_prettier_*) == output_prettier.*` (prettier normalizes to its canonical output)
/// - Our formatter validation is NOT applied (these test prettier's behavior, not ours)
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_prettier_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "unformatted_prettier_", &[])
}

/// Discover prettier_intermediate_* files in a fixture directory
///
/// These files capture Prettier's unstable intermediate output from `unformatted_ours_*` files.
/// They document what Prettier produces on the first pass before reaching a stable form.
///
/// Validation rules:
/// 1. `prettier(unformatted_ours_X) == prettier_intermediate_X` (captures first-pass output)
/// 2. `prettier(prettier_intermediate_X) != prettier_intermediate_X` (verifies it's unstable)
/// 3. `prettier(prettier_intermediate_X) == input` (converges to stable form)
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_prettier_intermediate_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(
        fixture_dir,
        ext,
        "prettier_intermediate_",
        &["prettier_intermediate_to_variant_"],
    )
}

/// Discover prettier_intermediate_to_variant_* files in a fixture directory
///
/// These files capture Prettier's unstable intermediate output from `unformatted_ours_*` files
/// when the second pass converges to a documented `variant_*`/`prettier_variant_*` file
/// rather than to `input`.
///
/// Validation rules (N7b):
/// 1. `prettier(unformatted_ours_X) == prettier_intermediate_to_variant_X` (captures first-pass output)
/// 2. `prettier(prettier_intermediate_to_variant_X) != prettier_intermediate_to_variant_X` (verifies it's unstable)
/// 3. `prettier(prettier_intermediate_to_variant_X) ∈ {variant_*, prettier_variant_*}` content
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_prettier_intermediate_to_variant_variants(
    fixture_dir: &Path,
    ext: &str,
) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "prettier_intermediate_to_variant_", &[])
}

/// Discover input_invalid_* files in a fixture directory
///
/// These files test that parsers correctly reject invalid syntax.
/// They should fail to parse with both our parser and the canonical parser.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_invalid_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "input_invalid_", &[])
}

/// Discover unknown files in a fixture directory
///
/// Returns a list of files that don't match any known fixture file pattern.
/// This helps catch typos like "unformated_*.svelte" (missing 't') or other
/// unexpected files that may have been added by accident.
///
/// Known file patterns:
/// - Input files: input.svelte, input.svelte.ts, input.ts, input.css
/// - Expected JSON: expected.json, expected_ours.json, expected_svelte.json
/// - Output prettier: output_prettier.{ext}
/// - Variants: unformatted_*.{ext}, unformatted_ours_*.{ext}, prettier_variant_*.{ext}, variant_*.{ext}, input_invalid_*.{ext}
/// - Documentation: README.md
pub fn discover_unknown_files(fixture: &Fixture) -> Vec<String> {
    let fixture_dir = &fixture.path;
    let input_ext = fixture.input_type().extension();
    let mut unknown = Vec::new();

    let Ok(entries) = fs::read_dir(fixture_dir) else {
        return unknown;
    };

    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let os_filename = entry.file_name();
        let Some(filename) = os_filename.to_str() else {
            continue;
        };
        if !is_known_fixture_file(filename, input_ext) {
            unknown.push(filename.to_string());
        }
    }

    unknown.sort();
    unknown
}

/// Check if a filename is a known fixture file pattern
fn is_known_fixture_file(filename: &str, input_ext: &str) -> bool {
    // Static files (input, expected, output_prettier, README, audit_signature)
    if matches!(
        filename,
        "input.svelte"
            | "input.svelte.ts"
            | "input.ts"
            | "input.css"
            | "expected.json"
            | "expected_ours.json"
            | "expected_svelte.json"
            | "output_prettier.svelte"
            | "output_prettier.svelte.ts"
            | "output_prettier.ts"
            | "output_prettier.css"
            | "README.md"
            | AUDIT_SIGNATURE_FILENAME
    ) {
        return true;
    }

    // Variant files must have correct extension matching input type
    // unformatted_*.{ext} (but not unformatted_ours_*)
    if filename.starts_with("unformatted_")
        && !filename.starts_with("unformatted_ours_")
        && filename.ends_with(input_ext)
    {
        return true;
    }

    // unformatted_ours_*.{ext}
    if filename.starts_with("unformatted_ours_") && filename.ends_with(input_ext) {
        return true;
    }

    // unformatted_prettier_*.{ext}
    if filename.starts_with("unformatted_prettier_") && filename.ends_with(input_ext) {
        return true;
    }

    // prettier_variant_*.{ext}
    if filename.starts_with("prettier_variant_") && filename.ends_with(input_ext) {
        return true;
    }

    // variant_*.{ext}
    if filename.starts_with("variant_") && filename.ends_with(input_ext) {
        return true;
    }

    // prettier_intermediate_*.{ext}
    if filename.starts_with("prettier_intermediate_") && filename.ends_with(input_ext) {
        return true;
    }

    // input_invalid_*.{ext}
    if filename.starts_with("input_invalid_") && filename.ends_with(input_ext) {
        return true;
    }

    false
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
    has_prettier_variants: bool,
    has_unformatted_ours: bool,
    has_variants: bool,
) -> Option<&'static str> {
    let needs_svelte = has_expected_ours || has_expected_svelte;
    let needs_prettier =
        has_output_prettier || has_prettier_variants || has_unformatted_ours || has_variants;

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
/// S5:  `prettier_variant_*` variants differ from input file
/// S6:  `output_prettier.svelte` differs from `input.svelte`
/// S7:  `unformatted_ours_*.svelte` variants differ from `input.svelte`
/// S8:  `_prettier_divergence` or `_svelte_prettier_divergence` suffix required when prettier divergence files exist
/// S9:  Prettier divergence dirs CANNOT have `unformatted_*.svelte` files
/// S10: `prettier_variant_*` files MUST be in prettier divergence dirs (enforced by S8)
/// S11: `unformatted_ours_*` files MUST be in prettier divergence dirs (enforced by S8)
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
            let prettier_variant_variants =
                discover_prettier_variant_variants(fixture_dir, input_ext);
            let unformatted_ours_variants =
                discover_unformatted_ours_variants(fixture_dir, input_ext);
            let variant_variants = discover_variant_variants(fixture_dir, input_ext);
            let suggested_suffix = determine_required_suffix(
                true, // has_expected_ours (we know this is true)
                true, // has_expected_svelte (we know this is true)
                output_prettier_path.exists(),
                !prettier_variant_variants.is_empty(),
                !unformatted_ours_variants.is_empty(),
                !variant_variants.is_empty(),
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

    // audit_signature.txt is only meaningful alongside output_prettier.*
    // (it captures prettier's chain from there). Reject orphans.
    let audit_signature_path = fixture.audit_signature_path();
    if audit_signature_path.exists() && !output_prettier_path.exists() {
        return Err(format!(
            "{AUDIT_SIGNATURE_FILENAME} exists without {output_prettier_filename}.\n\
            The audit signature pins prettier's multi-pass chain anchored at output_prettier.*\n\
            and only applies when that file exists. Either:\n\
            - Generate {output_prettier_filename} (run: deno task fixtures:update:formatted), or\n\
            - Delete {AUDIT_SIGNATURE_FILENAME} if no longer relevant."
        ));
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

    // Check prettier_variant_* variants
    let prettier_variant_variants = discover_prettier_variant_variants(fixture_dir, input_ext);
    for variant_name in &prettier_variant_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Rule 3: Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "prettier_variant_*{input_ext} variant '{variant_name}' is identical to {} (should demonstrate a prettier variant)",
                fixture.input_file
            ));
        }
    }

    // Check variant_* variants
    let variant_variants = discover_variant_variants(fixture_dir, input_ext);

    // Collect prettier_variant_* contents for cross-checking against variant_*
    let mut prettier_variant_contents: Vec<(String, String)> = Vec::new();
    for variant_name in &prettier_variant_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;
        prettier_variant_contents.push((variant_name.clone(), variant_content));
    }

    for variant_name in &variant_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "variant_*{input_ext} variant '{variant_name}' is identical to {} (should be a distinct stable form)",
                fixture.input_file
            ));
        }

        // Must differ from all prettier_variant_* files
        for (pv_name, pv_content) in &prettier_variant_contents {
            if variant_content == *pv_content {
                return Err(format!(
                    "variant_*{input_ext} variant '{variant_name}' is identical to prettier_variant file '{pv_name}'.\n\
                    variant_* files must have distinct content from prettier_variant_* files.\n\
                    If our formatter normalizes this to input, use prettier_variant_* instead."
                ));
            }
        }
    }

    // Discover unformatted_ours_* variants (needed for S8 validation)
    let unformatted_ours_variants = discover_unformatted_ours_variants(fixture_dir, input_ext);

    // Discover unformatted_prettier_* variants (needed for S8 validation)
    let unformatted_prettier_variants =
        discover_unformatted_prettier_variants(fixture_dir, input_ext);

    // S8: Check directory naming - prettier divergence suffix required when prettier validation should be skipped
    let has_prettier_variant_files = !prettier_variant_variants.is_empty();
    let has_variant_files = !variant_variants.is_empty();
    let has_output_prettier = output_prettier_path.exists();

    // Divergence documentation: files that show what prettier produces
    // (unformatted_ours_* tests OUR formatter, doesn't document prettier's output)
    let has_divergence_documentation =
        has_output_prettier || has_prettier_variant_files || has_variant_files;

    // Discover prettier_intermediate_* variants (needed for S8 validation)
    let prettier_intermediate_variants =
        discover_prettier_intermediate_variants(fixture_dir, input_ext);
    let has_prettier_intermediate_files = !prettier_intermediate_variants.is_empty();

    // Discover prettier_intermediate_to_variant_* variants (needed for S8 validation)
    let prettier_intermediate_to_variant_variants =
        discover_prettier_intermediate_to_variant_variants(fixture_dir, input_ext);
    let has_prettier_intermediate_to_variant_files =
        !prettier_intermediate_to_variant_variants.is_empty();

    // Prettier divergence suffix is required when ANY prettier divergence files exist
    let needs_prettier_divergence_suffix = has_output_prettier
        || has_prettier_variant_files
        || has_variant_files
        || !unformatted_ours_variants.is_empty()
        || !unformatted_prettier_variants.is_empty()
        || has_prettier_intermediate_files
        || has_prettier_intermediate_to_variant_files;

    if needs_prettier_divergence_suffix && !is_prettier_divergence_dir {
        let mut reasons = Vec::new();
        if has_output_prettier {
            reasons.push(output_prettier_filename.to_string());
        }
        if has_prettier_variant_files {
            reasons.push(format!(
                "{} prettier_variant_*{} file(s)",
                prettier_variant_variants.len(),
                input_ext
            ));
        }
        if has_variant_files {
            reasons.push(format!(
                "{} variant_*{} file(s)",
                variant_variants.len(),
                input_ext
            ));
        }
        if !unformatted_ours_variants.is_empty() {
            reasons.push(format!(
                "{} unformatted_ours_*{} file(s)",
                unformatted_ours_variants.len(),
                input_ext
            ));
        }
        if has_prettier_intermediate_files {
            reasons.push(format!(
                "{} prettier_intermediate_*{} file(s)",
                prettier_intermediate_variants.len(),
                input_ext
            ));
        }
        if has_prettier_intermediate_to_variant_files {
            reasons.push(format!(
                "{} prettier_intermediate_to_variant_*{} file(s)",
                prettier_intermediate_to_variant_variants.len(),
                input_ext
            ));
        }
        if !unformatted_prettier_variants.is_empty() {
            reasons.push(format!(
                "{} unformatted_prettier_*{} file(s)",
                unformatted_prettier_variants.len(),
                input_ext
            ));
        }
        let reason = reasons.join(" and ");

        // Use determine_required_suffix to suggest the correct suffix
        let suggested_suffix = determine_required_suffix(
            has_expected_ours,
            has_expected_svelte,
            has_output_prettier,
            has_prettier_variant_files,
            !unformatted_ours_variants.is_empty(),
            has_variant_files,
        )
        .unwrap_or("_prettier_divergence");

        // Build specific suggestions based on what files are causing the issue
        let base_dir_name = dir_name
            .trim_end_matches("_svelte_divergence")
            .trim_end_matches("_prettier_divergence");

        let mut suggestions = vec![format!(
            "Rename directory to '{base_dir_name}{suggested_suffix}' (keeps prettier validation skipped)"
        )];

        // If the only issue is unformatted_ours_* files, offer the rename alternative
        if !unformatted_ours_variants.is_empty()
            && !has_output_prettier
            && !has_prettier_variant_files
        {
            let file_renames: Vec<String> = unformatted_ours_variants
                .iter()
                .map(|f| {
                    let new_name = f.replace("unformatted_ours_", "unformatted_");
                    format!("  {f} → {new_name}")
                })
                .collect();
            suggestions.push(format!(
                "Rename file(s) to enable prettier validation:\n{}",
                file_renames.join("\n")
            ));
        }

        return Err(format!(
            "Directory name must end with '{suggested_suffix}' when prettier validation should be skipped.\n\
            Found {reason} but directory '{dir_name}' lacks the suffix.\n\n\
            Options:\n\
            - {}\n\n\
            The 'unformatted_ours_*' naming skips prettier validation, which requires the divergence suffix.\n\
            Use 'unformatted_*' (without 'ours') if both formatters should validate the file.",
            suggestions.join("\n- ")
        ));
    }

    // S8-rev: Prettier divergence dir MUST document the divergence
    // Acceptable documentation:
    // - output_prettier.* (shows prettier formats input differently)
    // - prettier_variant_*.* (shows prettier's stable variants)
    // - unformatted_ours_*.* + README.md (for normalization divergence where prettier(input)==input)
    let readme_path = fixture_dir.join("README.md");
    let has_readme = readme_path.exists();
    let has_unformatted_ours = !unformatted_ours_variants.is_empty();

    // unformatted_ours_* + README is acceptable when prettier(input) == input
    // (divergence is about normalization behavior, not formatting the canonical input)
    let has_normalization_divergence_docs = has_unformatted_ours && has_readme;
    let has_any_divergence_docs = has_divergence_documentation || has_normalization_divergence_docs;

    if !has_any_divergence_docs && is_prettier_divergence_dir {
        // For pure _prettier_divergence dirs (not combined with _svelte)
        if !is_svelte_divergence_dir && dir_name.ends_with("_prettier_divergence") {
            return Err(format!(
                "Directory '{dir_name}' claims prettier divergence but lacks documentation.\n\
                The '_prettier_divergence' suffix means we differ from Prettier - that claim must be documented.\n\n\
                Required: Add one of these:\n\
                - {output_prettier_filename} (if prettier formats input differently)\n\
                - prettier_variant_*{input_ext} files (if prettier has stable variants our formatter normalizes)\n\
                - variant_*{input_ext} files (if both formatters keep the form stable)\n\
                - unformatted_ours_*{input_ext} files + README.md (if divergence is about normalization)"
            ));
        }
        // For combined _svelte_prettier_divergence dirs
        if is_svelte_divergence_dir && dir_name.ends_with("_svelte_prettier_divergence") {
            return Err(format!(
                "Directory '{dir_name}' claims both parser AND formatter divergence.\n\
                Parser divergence is documented (expected_ours.json + expected_svelte.json).\n\
                Formatter divergence is NOT documented.\n\n\
                Either:\n\
                - Add {output_prettier_filename} or prettier_variant_*{input_ext} or variant_*{input_ext} to document formatter divergence, OR\n\
                - Add unformatted_ours_*{input_ext} + README.md for normalization divergence, OR\n\
                - Rename to '{}_svelte_divergence' if there's no formatter divergence",
                dir_name.trim_end_matches("_svelte_prettier_divergence")
            ));
        }
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

    // Check unformatted_prettier_* variants
    // These require output_prettier.* to exist (they normalize to prettier's output)
    if !unformatted_prettier_variants.is_empty() && !has_output_prettier {
        return Err(format!(
            "unformatted_prettier_*{input_ext} files require {} to exist.\n\
            Found {} unformatted_prettier_*{input_ext} file(s) but no {}.\n\
            These files test that prettier normalizes to its canonical output.\n\
            Either:\n\
            - Add {} (run: deno task fixtures:update:formatted), OR\n\
            - Remove unformatted_prettier_*{input_ext} files",
            output_prettier_filename,
            unformatted_prettier_variants.len(),
            output_prettier_filename,
            output_prettier_filename,
        ));
    }

    for variant_name in &unformatted_prettier_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "unformatted_prettier_*{input_ext} variant '{variant_name}' is identical to {} (should be different for testing normalization)",
                fixture.input_file
            ));
        }
    }

    // Check if README.md should exist (D1 validation)
    let has_parser_divergence = has_expected_ours && has_expected_svelte;
    let has_formatter_divergence = output_prettier_path.exists();
    let has_prettier_variants = !prettier_variant_variants.is_empty();
    let has_variants = !variant_variants.is_empty();
    let has_prettier_intermediate = !prettier_intermediate_variants.is_empty();
    let has_prettier_intermediate_to_variant =
        !prettier_intermediate_to_variant_variants.is_empty();

    let needs_readme = has_parser_divergence
        || has_formatter_divergence
        || has_prettier_variants
        || has_variants
        || has_prettier_intermediate
        || has_prettier_intermediate_to_variant;

    if needs_readme && !has_readme {
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
        if has_prettier_variants {
            reasons.push(format!(
                "- Prettier variants (prettier_variant_*{input_ext})"
            ));
        }
        if has_variants {
            reasons.push(format!("- Prettier stable variants (variant_*{input_ext})"));
        }
        if has_prettier_intermediate {
            reasons.push(format!(
                "- Prettier intermediate (prettier_intermediate_*{input_ext})"
            ));
        }
        if has_prettier_intermediate_to_variant {
            reasons.push(format!(
                "- Prettier intermediate to variant (prettier_intermediate_to_variant_*{input_ext})"
            ));
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
    match InputType::from_filepath(filepath) {
        Some(InputType::Svelte) => {
            let ast =
                tsv_svelte::parse(content).map_err(|e| format!("Format error (parse): {e:?}"))?;
            Ok(tsv_svelte::format(&ast, content))
        }
        Some(InputType::SvelteTs | InputType::TypeScript) => {
            let ast = tsv_ts::parse(content).map_err(|e| format!("Format error (parse): {e:?}"))?;
            Ok(tsv_ts::format(&ast, content))
        }
        Some(InputType::Css) => {
            let ast =
                tsv_css::parse(content).map_err(|e| format!("Format error (parse): {e:?}"))?;
            Ok(tsv_css::format(&ast, content))
        }
        None => Err(format!("Unsupported file type for formatting: {filepath}")),
    }
}

/// A fixture input parsed once with our parser.
///
/// The parser-side validation phases (expected.json comparison, wire-path
/// identity, typed-walk parity probes) all need the same AST — sharing one
/// parse keeps `fixtures_validate` from re-parsing every fixture per phase.
pub enum ParsedInput {
    Svelte(tsv_svelte::Root),
    Ts(tsv_ts::Program),
    Css(tsv_css::CssStyleSheet),
}

/// Parse fixture content once for the parser-side validation phases.
pub fn parse_input(content: &str, input_type: InputType) -> Result<ParsedInput, String> {
    match input_type {
        InputType::Svelte => tsv_svelte::parse(content)
            .map(ParsedInput::Svelte)
            .map_err(|e| format!("Parse error: {e:?}")),
        InputType::SvelteTs | InputType::TypeScript => tsv_ts::parse(content)
            .map(ParsedInput::Ts)
            .map_err(|e| format!("Parse error: {e:?}")),
        InputType::Css => tsv_css::parse(content)
            .map(ParsedInput::Css)
            .map_err(|e| format!("Parse error: {e:?}")),
    }
}

/// Both JSON-AST outputs derived from one `convert_ast_json` call.
pub struct InputAstPaths {
    /// `convert_ast_json`'s `Value` (for semantic comparison against
    /// `expected.json`, ignoring key-order differences).
    pub ast_json: serde_json::Value,
    /// The same `Value` serialized with tabs + trailing newline — the exact
    /// bytes `expected*.json` files store (matches `fixtures_update_parsed`).
    pub ast_json_tabs: String,
    /// Whether the compact wire path (`convert_ast_json_string` — what
    /// FFI/WASM/CLI-compact ship, with its own fast-path eligibility gates
    /// and multibyte offset translation) is byte-identical to the `Value`
    /// path. The expected.json comparisons go through `convert_ast_json`,
    /// so without this check the shipped path would be fixture-blind.
    pub wire_path_matches: bool,
}

/// Compute the `Value`-path AST and the wire-path identity check from an
/// already-parsed input, materializing `convert_ast_json` once.
#[allow(clippy::expect_used)] // Value serialization cannot fail
pub fn input_ast_paths(parsed: &ParsedInput, content: &str) -> Result<InputAstPaths, String> {
    let (ast_json, wire) = match parsed {
        ParsedInput::Svelte(ast) => (
            tsv_svelte::convert_ast_json(ast, content),
            tsv_svelte::convert_ast_json_string(ast, content),
        ),
        ParsedInput::Ts(ast) => (
            tsv_ts::convert_ast_json(ast, content),
            tsv_ts::convert_ast_json_string(ast, content),
        ),
        ParsedInput::Css(ast) => (
            tsv_css::convert_ast_json(ast, content),
            tsv_css::convert_ast_json_string(ast, content),
        ),
    };
    let tabs = to_json_with_tabs(&ast_json)
        .map_err(|e| format!("Failed to serialize AST to JSON: {e}"))?;
    let value_compact = serde_json::to_string(&ast_json).expect("Value serialization cannot fail");
    Ok(InputAstPaths {
        ast_json,
        // Trailing newline matches the fixtures_update_parsed format
        ast_json_tabs: format!("{tabs}\n"),
        wire_path_matches: wire == value_compact,
    })
}

/// Multibyte comment prepended to synthesize probe variants — shifts every
/// downstream byte offset away from its UTF-16 offset, so the typed
/// offset-translation walk is exercised on the whole AST shape.
const TYPED_WALK_SYNTH_PREFIX: &str = "// 中文😀\n";

/// How a typed-walk parity probe failed.
#[derive(Debug)]
pub enum TypedWalkParityFailure {
    /// The probe content failed to parse. This is an error (not a skip): a
    /// silently dropped probe would reopen the coverage hole the probes exist
    /// to close.
    Parse(String),
    /// `convert_ast_json_string` differs from the `Value` path.
    Diverged,
}

/// Outcome of the typed-walk parity probes for one fixture input.
#[derive(Debug, Default)]
pub struct TypedWalkParity {
    /// Probes that ran and matched.
    pub checked: usize,
    /// Failed probes: (probe description, failure).
    pub failures: Vec<(String, TypedWalkParityFailure)>,
}

/// Probe `tsv_ts`'s typed offset-translation walk for parity with the `Value`
/// walk, beyond what the fixture's own content exercises.
///
/// The typed walk (`translate_byte_to_char_offsets_typed`) enumerates struct
/// fields manually, so a position-bearing field missing from it stays green on
/// every ASCII fixture (translation is a no-op on both paths) and on every
/// multibyte `.svelte` fixture (Svelte's gate routes those to the `Value`
/// fallback). These probes close that hole:
///
/// - `.ts` / `.svelte.ts` inputs get a synthesized multibyte variant (a
///   prepended multibyte comment shifts all downstream offsets). Inputs with
///   byte-0 features (hashbang, BOM) are skipped — prepending would change
///   their semantics.
/// - `.svelte` inputs have their `<script>` contents extracted and run
///   through `tsv_ts`'s two paths as standalone TS — as-is when already
///   multibyte, plus a synthesized multibyte variant — so every AST shape in
///   the corpus gets typed-walk coverage, not just the few standalone-TS
///   fixtures.
///
/// Each probe asserts `convert_ast_json_string` is byte-identical to
/// `serde_json::to_string(&convert_ast_json(..))`. Probes are independent of
/// `expected.json`, so they don't affect parser conformance. Returns an empty
/// result for `.css` (no typed pipeline). Takes the already-parsed input so
/// `.svelte` script-span extraction reuses the fixture's one parse.
#[allow(clippy::expect_used)] // Value serialization cannot fail
pub fn typed_walk_parity_probes(content: &str, parsed: &ParsedInput) -> TypedWalkParity {
    let mut parity = TypedWalkParity::default();

    let mut probe = |ts_content: &str, description: &str| match tsv_ts::parse(ts_content) {
        Ok(ast) => {
            let string_path = tsv_ts::convert_ast_json_string(&ast, ts_content);
            let value_path = serde_json::to_string(&tsv_ts::convert_ast_json(&ast, ts_content))
                .expect("Value serialization cannot fail");
            if string_path == value_path {
                parity.checked += 1;
            } else {
                parity
                    .failures
                    .push((description.to_string(), TypedWalkParityFailure::Diverged));
            }
        }
        Err(e) => {
            parity.failures.push((
                description.to_string(),
                TypedWalkParityFailure::Parse(format!("{e:?}")),
            ));
        }
    };

    match parsed {
        ParsedInput::Ts(_) => {
            // Byte-0 features (hashbang, BOM) can't take a prepended comment
            if content.starts_with("#!") || content.starts_with('\u{feff}') {
                return parity;
            }
            // The as-is input is already covered by the string-path identity
            // check; only the synthesized multibyte variant is new coverage.
            let synthesized = format!("{TYPED_WALK_SYNTH_PREFIX}{content}");
            probe(&synthesized, "synthesized multibyte input");
        }
        ParsedInput::Svelte(root) => {
            for (i, (start, end)) in tsv_svelte::script_content_spans(root)
                .into_iter()
                .enumerate()
            {
                let script = &content[start as usize..end as usize];
                if !script.is_ascii() {
                    // Multibyte .svelte inputs take the Value fallback in
                    // tsv_svelte, so this standalone-TS run is the only
                    // typed-walk coverage their script content gets
                    probe(script, &format!("extracted script {i} (as-is)"));
                }
                let synthesized = format!("{TYPED_WALK_SYNTH_PREFIX}{script}");
                probe(
                    &synthesized,
                    &format!("extracted script {i} (synthesized multibyte)"),
                );
            }
        }
        ParsedInput::Css(_) => {} // no typed pipeline for CSS
    }

    parity
}
