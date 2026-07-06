//! Fixture data model: input types, the `Fixture` struct, and
//! divergence-suffix naming rules.

use crate::deno::PrettierParser;
use crate::fixtures::AUDIT_SIGNATURE_FILENAME;
use std::path::PathBuf;
use tsv_cli::cli::input::ParserType;
use tsv_ts::Goal;

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

/// Marker file asserting prettier *rejects* the fixture's input — its parser
/// throws (e.g. a typescript-estree parse error) or its printer crashes — so
/// prettier cannot serve as a formatter oracle (no `output_prettier.*`, no
/// chain to pin in `audit_signature.txt`). The validator live-verifies the
/// claim instead of running F2/F3/F4 and the prettier-side N rules (rule F6):
/// `prettier(input)` must return an error whose message contains this file's
/// trimmed content (the position-stripped error text, matched with `contains`).
/// Catches the bug being fixed upstream (prettier accepts → stale) and the
/// error morphing (different message → stale). Only sanctioned in
/// `_prettier_divergence` directories with a README; the marker holds exactly
/// the expected-error substring — prose lives in README.md. Mutually exclusive
/// with `prettier_nonconvergent.txt` (prettier either throws or oscillates).
pub const PRETTIER_REJECTS_FILENAME: &str = "prettier_rejects.txt";

/// Marker file asserting **tsv** rejects the fixture's input while the canonical
/// parser (Svelte / acorn-typescript / `parseCss`) *accepts* it — a deliberate
/// tsv over-rejection (e.g. a spec-stricter parse than acorn's). Such a
/// divergence cannot be an `input_invalid_*` fixture (which requires *both*
/// parsers to reject) nor a plain fixture (which requires tsv to parse+format
/// the input). The validator replaces the tsv-side parser/formatter phases with
/// a live rejection check (rule F7): `tsv::parse(input)` must fail with a message
/// containing this file's trimmed content (the position-stripped error text,
/// matched with `contains`), while `expected_svelte.json` pins the canonical AST
/// (the canonical parser must still accept — a dead divergence, where the
/// canonical parser starts rejecting too, surfaces here and in
/// `fixtures:update:parsed`). tsv produces no AST, so there is no
/// `expected.json` / `expected_ours.json`; the fixture makes no formatting claim,
/// so it is mutually exclusive with every format-claim file, `input_invalid_*`,
/// and the prettier no-oracle markers. Requires the `_svelte_divergence` suffix +
/// a README; the marker holds exactly the expected-error substring — prose lives
/// in README.md.
pub const TSV_REJECTS_FILENAME: &str = "tsv_rejects.txt";

/// Marker file selecting the parse goal for a standalone-script fixture. When
/// present (containing `script`), the fixture's `input.ts` is parsed as a
/// strict **Script** (`tsv_ts::Goal::Script`) rather than the default
/// **Module** — by both tsv and the acorn `expected.json` oracle — so `await`
/// is an ordinary identifier and `import`/`export`/`import.meta` are syntax
/// errors. Absent (the common case) means `Goal::Module`. Only meaningful on
/// `.ts` / `.svelte.ts` fixtures (Svelte `<script>` and CSS have no goal).
pub const GOAL_FILENAME: &str = "goal";

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

    /// Get the full path to the prettier-rejects marker file
    pub fn prettier_rejects_path(&self) -> PathBuf {
        self.path.join(PRETTIER_REJECTS_FILENAME)
    }

    /// Get the full path to the tsv-rejects marker file
    pub fn tsv_rejects_path(&self) -> PathBuf {
        self.path.join(TSV_REJECTS_FILENAME)
    }

    /// Get the full path to the parse-goal marker file
    pub fn goal_path(&self) -> PathBuf {
        self.path.join(GOAL_FILENAME)
    }

    /// The parse goal for this fixture's input, read lazily from the `goal`
    /// marker file. Absent or unreadable → `Goal::Module` (the default); a file
    /// trimming to `script` → `Goal::Script`. Drives both tsv's parse and the
    /// acorn `expected.json` oracle so a standalone-script fixture is graded at
    /// the same goal on both sides.
    pub fn goal(&self) -> Goal {
        // Lenient: an absent/unreadable/unrecognized marker → the `Module` default
        // (only a marker trimming to `script`/`module` selects a goal), via the
        // shared `Goal::from_source_type` vocabulary.
        std::fs::read_to_string(self.goal_path())
            .ok()
            .and_then(|s| Goal::from_source_type(s.trim()))
            .unwrap_or(Goal::Module)
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
    has_divergent_variant: bool,
) -> Option<&'static str> {
    let needs_svelte = has_expected_ours || has_expected_svelte;
    let needs_prettier = has_output_prettier
        || has_prettier_variants
        || has_unformatted_ours
        || has_variants
        || has_divergent_variant;

    match (needs_svelte, needs_prettier) {
        (true, true) => Some("_svelte_prettier_divergence"),
        (true, false) => Some("_svelte_divergence"),
        (false, true) => Some("_prettier_divergence"),
        (false, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_type_from_filepath_ordering_and_roundtrip() {
        // The critical ordering case: `.svelte.ts` must be matched before `.ts`.
        assert_eq!(
            InputType::from_filepath("input.svelte.ts"),
            Some(InputType::SvelteTs)
        );
        assert_eq!(
            InputType::from_filepath("input.ts"),
            Some(InputType::TypeScript)
        );
        assert_eq!(
            InputType::from_filepath("input.svelte"),
            Some(InputType::Svelte)
        );
        assert_eq!(InputType::from_filepath("input.css"), Some(InputType::Css));
        // Unknown extensions fail loudly (None) rather than misclassify.
        assert_eq!(InputType::from_filepath("input.json"), None);
        assert_eq!(InputType::from_filepath("README.md"), None);

        // extension() round-trips back through from_filepath() for every variant.
        for ty in [
            InputType::Svelte,
            InputType::SvelteTs,
            InputType::TypeScript,
            InputType::Css,
        ] {
            let name = format!("input{}", ty.extension());
            assert_eq!(
                InputType::from_filepath(&name),
                Some(ty),
                "round-trip {name}"
            );
        }

        // `.svelte.ts` rune modules are TypeScript to our parser.
        assert_eq!(InputType::SvelteTs.parser_type(), ParserType::TypeScript);
        assert_eq!(InputType::Svelte.parser_type(), ParserType::Svelte);
    }

    #[test]
    fn determine_required_suffix_truth_table() {
        // (ours, svelte, output_prettier, prettier_variants, unformatted_ours, variants, divergent_variant)
        // expected_ours alone ⇒ svelte divergence.
        assert_eq!(
            determine_required_suffix(true, false, false, false, false, false, false),
            Some("_svelte_divergence")
        );
        // output_prettier alone ⇒ prettier divergence.
        assert_eq!(
            determine_required_suffix(false, false, true, false, false, false, false),
            Some("_prettier_divergence")
        );
        // Both sides present ⇒ combined suffix.
        assert_eq!(
            determine_required_suffix(false, true, false, false, true, false, false),
            Some("_svelte_prettier_divergence")
        );
        // Nothing ⇒ no suffix required.
        assert_eq!(
            determine_required_suffix(false, false, false, false, false, false, false),
            None
        );
        // unformatted_ours alone flips the prettier side on.
        assert_eq!(
            determine_required_suffix(false, false, false, false, true, false, false),
            Some("_prettier_divergence")
        );
        // divergent_variant alone flips the prettier side on.
        assert_eq!(
            determine_required_suffix(false, false, false, false, false, false, true),
            Some("_prettier_divergence")
        );
        // expected_svelte alone flips the svelte side on.
        assert_eq!(
            determine_required_suffix(false, true, false, false, false, false, false),
            Some("_svelte_divergence")
        );
    }
}
