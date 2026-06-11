//! Unified fixture validation with per-fixture error grouping
//!
//! All validation errors for a single fixture are collected together,
//! enabling better DX with grouped error reporting.
//!
//! - errors.rs: `ValidationError` / `ValidationSuccess` and fix hints
//! - structure.rs: structure validation (S* rules — file layout, divergence suffixes)
//! - phases.rs: per-phase validation functions (P* parser, F* formatter, N* normalization)
//! - summary.rs: cross-fixture aggregation and result printing
//!
//! This mod.rs keeps the per-fixture result type (`FixtureValidation`) and the
//! `validate_fixture` orchestrator.

pub mod errors;
mod phases;
mod structure;
mod summary;

pub use errors::{ValidationError, ValidationSuccess};
pub use structure::validate_fixture_structure;
pub use summary::{ValidationSummary, print_validation_results};

use crate::fixtures::{self, Fixture, InputType, discover_unknown_files, read_file};

use phases::{
    validate_formatter_idempotent, validate_formatter_prettier, validate_invalid_syntax,
    validate_normalization_ours, validate_normalization_prettier, validate_parser_external,
    validate_parser_ours, validate_parser_ours_matches_expected, validate_typed_walk_parity,
};

/// Result of validating a single fixture
#[derive(Debug)]
pub struct FixtureValidation {
    pub fixture_path: String,
    pub errors: Vec<ValidationError>,
    pub successes: Vec<ValidationSuccess>,
    /// Variants that were checked (for reporting)
    pub unformatted_count: usize,
    pub unformatted_ours_count: usize,
    pub unformatted_prettier_count: usize,
    pub prettier_variant_count: usize,
    pub variant_count: usize,
    pub prettier_intermediate_count: usize,
    pub prettier_intermediate_to_variant_count: usize,
    pub invalid_syntax_count: usize,
    /// Input content for cross-fixture duplicate detection (populated during validation)
    pub input_content: Option<String>,
    /// Input file name for cross-fixture duplicate detection (e.g., "input.svelte", "input.ts")
    pub input_file_name: Option<String>,
    /// Undocumented Prettier outputs from unformatted_ours_* files (informational, not blocking)
    pub undocumented_prettier_outputs: Vec<UndocumentedPrettierOutput>,
}

/// An undocumented Prettier output discovered during N10 cross-path analysis
#[derive(Debug)]
#[allow(dead_code)]
pub struct UndocumentedPrettierOutput {
    /// The unformatted_ours_* source file that produced this output
    pub source_file: String,
    /// The suffix (e.g., "compact" from "unformatted_ours_compact.svelte")
    pub suffix: String,
}

impl FixtureValidation {
    pub fn new(fixture_path: String) -> Self {
        Self {
            fixture_path,
            errors: Vec::new(),
            successes: Vec::new(),
            unformatted_count: 0,
            unformatted_ours_count: 0,
            unformatted_prettier_count: 0,
            prettier_variant_count: 0,
            variant_count: 0,
            prettier_intermediate_count: 0,
            prettier_intermediate_to_variant_count: 0,
            invalid_syntax_count: 0,
            input_content: None,
            input_file_name: None,
            undocumented_prettier_outputs: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_success(&mut self, success: ValidationSuccess) {
        self.successes.push(success);
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Validate a single fixture, collecting all errors
///
/// When `prettier_only` is true, skips our parser/formatter validation.
/// This is useful for validating fixture design before implementing features.
pub async fn validate_fixture(fixture: &Fixture, prettier_only: bool) -> FixtureValidation {
    let mut result = FixtureValidation::new(fixture.relative_path.clone());

    // Phase 1: Structure validation (pure Rust)
    if let Err(e) = validate_fixture_structure(fixture) {
        result.add_error(ValidationError::StructureValidationFailed(e));
        return result; // Stop early if structure is invalid
    }

    // Check for unknown files (catches typos like "unformated_*.svelte")
    let unknown_files = discover_unknown_files(fixture);
    for unknown_file in unknown_files {
        result.add_error(ValidationError::UnknownFile(unknown_file));
    }

    result.add_success(ValidationSuccess::StructureValid(16));

    // Read input file
    let input = match read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => {
            result.add_error(ValidationError::ParserError(format!(
                "Failed to read input: {e}"
            )));
            return result;
        }
    };

    // Store input content and file name for cross-fixture duplicate detection
    result.input_content = Some(input.clone());
    result.input_file_name = Some(fixture.input_file.clone());

    let input_type = fixture.input_type();
    let input_ext = input_type.extension();

    // Phases 2-4: Our parser/formatter validation (skip in prettier_only mode)
    if !prettier_only {
        // Phases 2/2b/2c/2d share one parse of the input (and one
        // convert_ast_json materialization for 2/2b/2c)
        match fixtures::parse_input(&input, input_type) {
            Ok(parsed) => {
                match fixtures::input_ast_paths(&parsed, &input) {
                    Ok(paths) => {
                        // Phase 2: Our Parser validation - P2 (pure Rust)
                        validate_parser_ours(&mut result, fixture, &paths);

                        // Phase 2b: Our parser matches expected.json
                        // (non-divergence only, pure Rust)
                        validate_parser_ours_matches_expected(&mut result, fixture, &paths);

                        // Phase 2c: Compact wire path matches the Value path (pure Rust)
                        if paths.wire_path_matches {
                            result.add_success(ValidationSuccess::ParserJsonStringPathMatches);
                        } else {
                            result.add_error(ValidationError::ParserJsonStringPathDiverges);
                        }
                    }
                    Err(e) => result.add_error(ValidationError::ParserError(e)),
                }

                // Phase 2d: Typed-walk parity probes — synthesized multibyte
                // variants and extracted <script> contents (pure Rust)
                validate_typed_walk_parity(&mut result, &input, &parsed);
            }
            Err(e) => {
                // One parse failure, one error — context-aware for
                // svelte_divergence fixtures (matches the pre-merge phases)
                if fixture.is_svelte_divergence() {
                    result.add_error(ValidationError::ParserErrorInDivergence(e));
                } else {
                    result.add_error(ValidationError::ParserError(e));
                }
            }
        }

        // Phase 3: Our Formatter validation - F1 (pure Rust)
        let format_ok = validate_formatter_idempotent(&mut result, fixture, &input);

        // Phase 4: Our Normalization (skip if F1 failed)
        if format_ok {
            validate_normalization_ours(&mut result, fixture, &input, input_ext);
        } else {
            result.add_success(ValidationSuccess::NormalizationSkipped);
        }
    }

    // Phase 5: Deno sidecar validations (prettier + Svelte/TypeScript parser)
    // P1, P3: Parser freshness
    validate_parser_external(&mut result, fixture, &input, input_type).await;

    // F2, F3: Prettier freshness and baseline (Svelte and SvelteTs)
    // TypeScript/CSS fixtures don't use prettier-svelte plugin
    if input_type == InputType::Svelte || input_type == InputType::SvelteTs {
        validate_formatter_prettier(&mut result, fixture, &input).await;

        // N1, N3: Prettier normalization (Svelte and SvelteTs)
        validate_normalization_prettier(&mut result, fixture, &input, input_ext).await;
    }

    // Phase 6: Invalid syntax validation (input_invalid_* files)
    // Skip in prettier_only mode (these test our parser rejection, not prettier)
    if !prettier_only {
        validate_invalid_syntax(&mut result, fixture, input_type, input_ext).await;
    }

    result
}
