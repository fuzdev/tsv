//! Fixture data model: input types, the `Fixture` struct, and
//! divergence-suffix naming rules.

use crate::deno::PrettierParser;
use crate::fixtures::AUDIT_SIGNATURE_FILENAME;
use std::path::PathBuf;
use tsv_cli::cli::input::ParserType;

/// Canonical error JSON format for expected_svelte.json files
///
/// This is the complete JSON content (with trailing newline) written to expected_svelte.json
/// when Svelte's parser fails to parse the input.
pub const EXPECTED_SVELTE_ERROR_JSON: &str = "{\"error\": \"failed to parse\"}\n";

/// Marker file asserting prettier has NO fixed point on the fixture's input —
/// each pass keeps changing the output forever, so prettier cannot serve as a
/// formatter oracle (no `output_prettier.*`, no chain to pin in
/// `audit_signature.txt`). The validator live-verifies the claim instead of
/// running F2/F3/F4 and the prettier-side N rules (rule F5): `prettier(input)`
/// must differ from input AND `prettier^2(input)` must differ from
/// `prettier(input)`. Only sanctioned in `_prettier_divergence` directories
/// with a README; content is free-form prose describing the non-convergence.
pub const PRETTIER_NONCONVERGENT_FILENAME: &str = "prettier_nonconvergent.txt";

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

    /// The `ParserType` our parser/formatter handles this input type as
    /// (`.svelte.ts` rune modules are plain TypeScript to tsv).
    pub const fn parser_type(self) -> ParserType {
        match self {
            InputType::Svelte => ParserType::Svelte,
            InputType::SvelteTs | InputType::TypeScript => ParserType::TypeScript,
            InputType::Css => ParserType::Css,
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

    /// Get the full path to the prettier non-convergence marker file
    pub fn prettier_nonconvergent_path(&self) -> PathBuf {
        self.path.join(PRETTIER_NONCONVERGENT_FILENAME)
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
