//! Unified fixture validation with per-fixture error grouping
//!
//! All validation errors for a single fixture are collected together,
//! enabling better DX with grouped error reporting.

use crate::deno::{PrettierParser, parse_css, parse_svelte, parse_typescript, run_prettier};
use crate::diff;
use crate::fixtures::{
    self, AuditSignature, Fixture, InputType, discover_invalid_variants,
    discover_prettier_intermediate_to_variant_variants, discover_prettier_intermediate_variants,
    discover_prettier_variant_variants, discover_unformatted_ours_variants,
    discover_unformatted_prettier_variants, discover_unformatted_variants, discover_unknown_files,
    discover_variant_variants, read_file,
};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use thiserror::Error;
use tsv_cli::json_utils::to_json_with_tabs;

/// Why `audit_signature.txt` is stale.
///
/// Both cases are repaired by the same command (`fixtures:update:formatted`), but the user-facing
/// remediation differs: drift is a routine regenerate, while a collapsed chain means prettier
/// became idempotent on `output_prettier` since the signature was captured — the regenerate will
/// delete the file, and the author should revisit whether the divergence still applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditSignatureStaleness {
    /// Live chain differs from recorded chain — prettier's pass-K output drifted.
    Drift,
    /// Prettier is now idempotent on `output_prettier` — chain has collapsed to depth zero.
    Collapsed,
}

impl fmt::Display for AuditSignatureStaleness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Drift => write!(f, "prettier-chain drift from output_prettier"),
            Self::Collapsed => write!(
                f,
                "prettier is now idempotent on output_prettier — chain collapsed"
            ),
        }
    }
}

/// Validation error with self-describing names
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    /// Structure validation failure with detailed message from `validate_fixture_structure()`
    #[error("{0}")]
    StructureValidationFailed(String),

    // Parser
    #[error("expected.json is outdated")]
    ParserExpectedJsonOutdated,
    #[error("expected_ours.json is outdated")]
    ParserExpectedOursOutdated,
    #[error("expected_svelte.json is outdated")]
    ParserExpectedSvelteOutdated,
    #[error("our parser output differs from expected.json")]
    ParserOursDiffersFromExpected,
    #[error("Parser error: {0}")]
    ParserError(String),
    #[error("Parser error (svelte_divergence): {0}")]
    ParserErrorInDivergence(String),

    // Formatter
    #[error("{0} doesn't format to itself")]
    FormatterInputNotIdempotent(String),
    #[error("output_prettier file is outdated")]
    FormatterOutputPrettierOutdated,
    #[error("input file differs from prettier output")]
    FormatterInputDiffersFromPrettier(String),
    #[error("audit_signature.txt is out of date ({0})")]
    FormatterAuditSignatureOutdated(AuditSignatureStaleness),
    #[error("audit_signature.txt is malformed: {0}")]
    FormatterAuditSignatureMalformed(String),
    #[error("audit_signature.txt walk failed: {0}")]
    FormatterAuditSignatureWalkFailed(String),
    #[error("Formatter error: {0}")]
    FormatterError(String),
    #[error("Formatter error (svelte_divergence): {0}")]
    FormatterErrorInDivergence(String),

    // Normalization
    #[error("{0} not preserved by prettier")]
    NormalizationPrettierVariantNotPreserved(String),
    #[error("{0} doesn't normalize to input file")]
    NormalizationPrettierVariantNotNormalized(String),
    #[error("{0} doesn't normalize to input file (prettier)")]
    NormalizationUnformattedPrettierMismatch(String),
    #[error("{0} doesn't normalize to input file")]
    NormalizationUnformattedNotNormalized(String),
    #[error("{0} doesn't normalize to input file")]
    NormalizationUnformattedOursNotNormalized(String),
    #[error("{0} normalizes to input file with prettier (should use unformatted_* instead)")]
    NormalizationUnformattedOursPrettierAlsoNormalizes(String),

    // unformatted_prettier_* (normalization to output_prettier)
    #[error("{0} doesn't normalize to output_prettier file")]
    NormalizationUnformattedPrettierNotNormalized(String),
    #[error("{0} exists but output_prettier file is missing")]
    NormalizationUnformattedPrettierMissingTarget(String),

    // Prettier stable (dual-stable forms)
    #[error("{0} not preserved by prettier")]
    NormalizationVariantNotPreserved(String),
    #[error("{0} not idempotent with our formatter")]
    NormalizationVariantOursNotIdempotent(String),
    #[error("{0} normalizes to input with our formatter (should be prettier_variant_* instead)")]
    NormalizationVariantNormalizesToInput(String),

    // Duplicates (within fixture) - variant
    #[error("Duplicate variant files: {}", .0.join(", "))]
    DuplicateVariantWithinFixture(Vec<String>),

    // Prettier intermediate (unstable first-pass output)
    #[error(
        "{0} doesn't match prettier's first-pass output from corresponding unformatted_ours_* file"
    )]
    NormalizationPrettierIntermediateMismatch(String),
    #[error(
        "{0} is stable (prettier preserves it) - should be prettier_variant_* or variant_* instead"
    )]
    NormalizationPrettierIntermediateIsStable(String),
    #[error("{0} doesn't converge to input file after second pass")]
    NormalizationPrettierIntermediateNotConverging(String),
    #[error("{0} has no corresponding unformatted_ours_* file")]
    NormalizationPrettierIntermediateMissingSource(String),

    // Prettier intermediate to variant (unstable first-pass output, converges to a variant)
    #[error(
        "{0} doesn't match prettier's first-pass output from corresponding unformatted_ours_* file"
    )]
    NormalizationPrettierIntermediateToVariantMismatch(String),
    #[error(
        "{0} is stable (prettier preserves it) - should be prettier_variant_* or variant_* instead"
    )]
    NormalizationPrettierIntermediateToVariantIsStable(String),
    #[error(
        "{0} doesn't converge to a documented variant_* / prettier_variant_* file after second pass (converges to input — rename to prettier_intermediate_* instead)"
    )]
    NormalizationPrettierIntermediateToVariantConvergesToInput(String),
    #[error(
        "{0} doesn't converge to any documented variant_* / prettier_variant_* file after second pass"
    )]
    NormalizationPrettierIntermediateToVariantNotConverging(String),
    #[error("{0} has no corresponding unformatted_ours_* file")]
    NormalizationPrettierIntermediateToVariantMissingSource(String),
    #[error(
        "{0} requires at least one variant_* or prettier_variant_* file in the fixture (the convergence target)"
    )]
    NormalizationPrettierIntermediateToVariantNoVariantTarget(String),

    // Undocumented prettier output (N10): a fixture that pins prettier's stable
    // forms (has output_prettier / prettier_variant_* / variant_*) must account
    // for prettier's output of every unformatted_ours_* variant.
    #[error(
        "prettier output of {0} matches no documented form (output_prettier / prettier_variant_* / variant_*) — prettier may have drifted, or the target is undocumented"
    )]
    UndocumentedPrettierOutput(String),

    // Duplicates (within fixture)
    #[error("Duplicate unformatted files: {}", .0.join(", "))]
    DuplicateUnformattedWithinFixture(Vec<String>),
    #[error("Duplicate prettier_variant files: {}", .0.join(", "))]
    DuplicatePrettierVariantWithinFixture(Vec<String>),
    #[error("{0} is redundant (identical to {1})")]
    RedundantUnformattedMatchesPrettierVariant(String, String),
    #[error("{0} is redundant (identical to output_prettier)")]
    RedundantPrettierVariantMatchesOutputPrettier(String),

    // Invalid syntax (input_invalid_* files)
    #[error("{0} parsed successfully by our parser (should fail)")]
    InvalidSyntaxParsedByOurs(String),
    #[error("{0} parsed successfully by Svelte (should fail)")]
    InvalidSyntaxParsedBySvelte(String),
    #[error("{0} parsed successfully by acorn-typescript (should fail)")]
    InvalidSyntaxParsedByAcorn(String),
    #[error("{0} parsed successfully by our CSS parser (should fail)")]
    InvalidSyntaxParsedByOurCss(String),
    #[error("{0} parsed successfully by parseCss (should fail)")]
    InvalidSyntaxParsedByParseCss(String),

    // Unknown files
    #[error("Unknown file: {0}")]
    UnknownFile(String),
}

impl ValidationError {
    /// Suggested fix for this error
    pub fn fix_hint(&self) -> &'static str {
        match self {
            Self::StructureValidationFailed(_) => "See error message for details",
            Self::ParserOursDiffersFromExpected => "Fix the parser to match expected.json",
            Self::ParserExpectedJsonOutdated
            | Self::ParserExpectedOursOutdated
            | Self::ParserExpectedSvelteOutdated => {
                "Run: deno task fixtures:update:parsed <pattern>"
            }
            Self::ParserError(_) => "Verify input is valid syntax; if valid, fix the parser",
            Self::ParserErrorInDivergence(_) => {
                "Fix the parser to support this syntax (svelte_divergence fixture)"
            }
            Self::FormatterInputNotIdempotent(input_file) => {
                // Return static str - the dynamic path is shown in the error message itself
                if input_file.ends_with(".svelte") {
                    "Debug: cargo run -p tsv_debug compare <fixture>/input.svelte"
                } else if input_file.ends_with(".svelte.ts") {
                    "Debug: cargo run -p tsv_debug compare <fixture>/input.svelte.ts"
                } else if input_file.ends_with(".ts") {
                    "Debug: cargo run -p tsv_debug compare <fixture>/input.ts"
                } else {
                    "Debug: cargo run -p tsv_debug compare <fixture>/input.css"
                }
            }
            Self::FormatterOutputPrettierOutdated => {
                "Run: deno task fixtures:update:formatted <pattern>"
            }
            Self::FormatterInputDiffersFromPrettier(input_file) => {
                if input_file.ends_with(".svelte") {
                    "Run: cargo run -p tsv_debug compare <fixture>/input.svelte to see difference"
                } else if input_file.ends_with(".svelte.ts") {
                    "Run: cargo run -p tsv_debug compare <fixture>/input.svelte.ts to see difference"
                } else if input_file.ends_with(".ts") {
                    "Run: cargo run -p tsv_debug compare <fixture>/input.ts to see difference"
                } else {
                    "Run: cargo run -p tsv_debug compare <fixture>/input.css to see difference"
                }
            }
            Self::FormatterAuditSignatureOutdated(AuditSignatureStaleness::Drift) => {
                "Run: deno task fixtures:update:formatted <pattern> (regenerates audit_signature.txt)"
            }
            Self::FormatterAuditSignatureOutdated(AuditSignatureStaleness::Collapsed) => {
                "Run: deno task fixtures:update:formatted <pattern> (deletes audit_signature.txt), then re-evaluate whether the _prettier_divergence designation still applies"
            }
            Self::FormatterAuditSignatureMalformed(_) => {
                "Delete and regenerate: deno task fixtures:update:formatted <pattern>"
            }
            Self::FormatterAuditSignatureWalkFailed(_) => {
                "Investigate prettier error or non-converging chain; check input syntax. Then: deno task fixtures:update:formatted <pattern>"
            }
            Self::FormatterError(_) => "Fix the formatter implementation",
            Self::FormatterErrorInDivergence(_) => {
                "Fix the formatter to support this syntax (svelte_divergence fixture)"
            }
            Self::NormalizationPrettierVariantNotPreserved(_) => {
                "Prettier doesn't preserve this file - rename to unformatted_*.svelte"
            }
            Self::NormalizationPrettierVariantNotNormalized(_)
            | Self::NormalizationUnformattedNotNormalized(_)
            | Self::NormalizationUnformattedOursNotNormalized(_) => {
                "Fix formatter to normalize this variant correctly"
            }
            Self::NormalizationUnformattedPrettierMismatch(_) => {
                "Prettier doesn't normalize to input file - check prettier behavior"
            }
            Self::NormalizationUnformattedOursPrettierAlsoNormalizes(_) => {
                "Rename to unformatted_*.* (prettier also normalizes this to input)"
            }
            Self::NormalizationUnformattedPrettierNotNormalized(_) => {
                "Check that prettier(file) == output_prettier content"
            }
            Self::NormalizationUnformattedPrettierMissingTarget(_) => {
                "Add output_prettier.* file or remove unformatted_prettier_* files"
            }
            Self::NormalizationVariantNotPreserved(_) => {
                "Prettier doesn't preserve this file - check if it should be a different variant type"
            }
            Self::NormalizationVariantOursNotIdempotent(_) => {
                "Our formatter doesn't preserve this file - if it normalizes to input, use prettier_variant_* instead"
            }
            Self::NormalizationVariantNormalizesToInput(_) => {
                "Our formatter normalizes this to input - rename to prettier_variant_* instead"
            }
            Self::DuplicateVariantWithinFixture(_) => "Remove duplicate files (identical content)",
            Self::NormalizationPrettierIntermediateMismatch(_) => {
                "Update prettier_intermediate_* to match prettier's actual first-pass output"
            }
            Self::NormalizationPrettierIntermediateIsStable(_) => {
                "Rename to prettier_variant_* or variant_* (prettier preserves this idempotently)"
            }
            Self::NormalizationPrettierIntermediateNotConverging(_) => {
                "Check prettier_intermediate_* content - should converge to input after re-formatting"
            }
            Self::NormalizationPrettierIntermediateMissingSource(_) => {
                "Add corresponding unformatted_ours_* file or remove prettier_intermediate_* file"
            }
            Self::NormalizationPrettierIntermediateToVariantMismatch(_) => {
                "Update prettier_intermediate_to_variant_* to match prettier's actual first-pass output"
            }
            Self::NormalizationPrettierIntermediateToVariantIsStable(_) => {
                "Rename to prettier_variant_* or variant_* (prettier preserves this idempotently)"
            }
            Self::NormalizationPrettierIntermediateToVariantConvergesToInput(_) => {
                "Rename to prettier_intermediate_* (second pass converges to input, not a variant)"
            }
            Self::NormalizationPrettierIntermediateToVariantNotConverging(_) => {
                "Check that the file's second prettier pass produces content matching some variant_* or prettier_variant_* sibling"
            }
            Self::NormalizationPrettierIntermediateToVariantMissingSource(_) => {
                "Add corresponding unformatted_ours_* file or remove prettier_intermediate_to_variant_* file"
            }
            Self::NormalizationPrettierIntermediateToVariantNoVariantTarget(_) => {
                "Add a variant_* or prettier_variant_* file documenting the convergence target"
            }
            Self::UndocumentedPrettierOutput(_) => {
                "Document prettier's output: add a variant_* / prettier_variant_* (or prettier_intermediate*_*) sibling matching it, or update the existing one if prettier changed"
            }
            Self::DuplicateUnformattedWithinFixture(_)
            | Self::DuplicatePrettierVariantWithinFixture(_) => {
                "Remove duplicate files (identical content)"
            }
            Self::RedundantUnformattedMatchesPrettierVariant(_, _) => {
                "Remove redundant file (already covered by prettier_variant_*)"
            }
            Self::RedundantPrettierVariantMatchesOutputPrettier(_) => {
                "Remove redundant file (output_prettier already documents this prettier output)"
            }
            Self::InvalidSyntaxParsedByOurs(_) => {
                "Our parser is too permissive - it accepts syntax that the canonical parser rejects. Fix the parser."
            }
            Self::InvalidSyntaxParsedBySvelte(_) => {
                "Svelte accepts this syntax - it's not actually invalid. Remove the file or fix the syntax."
            }
            Self::InvalidSyntaxParsedByAcorn(_) => {
                "Acorn-typescript accepts this syntax - it's not actually invalid. Remove the file or fix the syntax."
            }
            Self::InvalidSyntaxParsedByOurCss(_) => {
                "Our CSS parser is too permissive - it accepts invalid syntax. Fix the parser."
            }
            Self::InvalidSyntaxParsedByParseCss(_) => {
                "parseCss accepts this syntax - it's not actually invalid. Remove the file or fix the syntax."
            }
            Self::UnknownFile(_) => {
                "Remove or rename the file. Check for typos (e.g., 'unformated' vs 'unformatted')."
            }
        }
    }

    /// Get error category for grouping
    pub fn category(&self) -> &'static str {
        match self {
            Self::StructureValidationFailed(_) => "Structure",

            Self::ParserOursDiffersFromExpected
            | Self::ParserExpectedJsonOutdated
            | Self::ParserExpectedOursOutdated
            | Self::ParserExpectedSvelteOutdated
            | Self::ParserError(_)
            | Self::ParserErrorInDivergence(_) => "Parser",

            Self::FormatterInputNotIdempotent(_)
            | Self::FormatterOutputPrettierOutdated
            | Self::FormatterInputDiffersFromPrettier(_)
            | Self::FormatterAuditSignatureOutdated(_)
            | Self::FormatterAuditSignatureMalformed(_)
            | Self::FormatterAuditSignatureWalkFailed(_)
            | Self::FormatterError(_)
            | Self::FormatterErrorInDivergence(_) => "Formatter",

            Self::NormalizationPrettierVariantNotPreserved(_)
            | Self::NormalizationPrettierVariantNotNormalized(_)
            | Self::NormalizationUnformattedNotNormalized(_)
            | Self::NormalizationUnformattedOursNotNormalized(_)
            | Self::NormalizationUnformattedPrettierMismatch(_)
            | Self::NormalizationUnformattedOursPrettierAlsoNormalizes(_)
            | Self::NormalizationUnformattedPrettierNotNormalized(_)
            | Self::NormalizationUnformattedPrettierMissingTarget(_)
            | Self::NormalizationPrettierIntermediateMismatch(_)
            | Self::NormalizationVariantNotPreserved(_)
            | Self::NormalizationVariantOursNotIdempotent(_)
            | Self::NormalizationVariantNormalizesToInput(_)
            | Self::NormalizationPrettierIntermediateIsStable(_)
            | Self::NormalizationPrettierIntermediateNotConverging(_)
            | Self::NormalizationPrettierIntermediateMissingSource(_)
            | Self::NormalizationPrettierIntermediateToVariantMismatch(_)
            | Self::NormalizationPrettierIntermediateToVariantIsStable(_)
            | Self::NormalizationPrettierIntermediateToVariantConvergesToInput(_)
            | Self::NormalizationPrettierIntermediateToVariantNotConverging(_)
            | Self::NormalizationPrettierIntermediateToVariantMissingSource(_)
            | Self::NormalizationPrettierIntermediateToVariantNoVariantTarget(_)
            | Self::UndocumentedPrettierOutput(_) => "Normalization",

            Self::DuplicateUnformattedWithinFixture(_)
            | Self::DuplicatePrettierVariantWithinFixture(_)
            | Self::DuplicateVariantWithinFixture(_)
            | Self::RedundantUnformattedMatchesPrettierVariant(_, _)
            | Self::RedundantPrettierVariantMatchesOutputPrettier(_) => "Duplicates",

            Self::InvalidSyntaxParsedByOurs(_)
            | Self::InvalidSyntaxParsedBySvelte(_)
            | Self::InvalidSyntaxParsedByAcorn(_)
            | Self::InvalidSyntaxParsedByOurCss(_)
            | Self::InvalidSyntaxParsedByParseCss(_) => "InvalidSyntax",

            Self::UnknownFile(_) => "Structure",
        }
    }
}

/// Successful check for verbose reporting
#[derive(Debug, Clone)]
pub enum ValidationSuccess {
    StructureValid(usize), // number of checks passed
    ParserExpectedJsonMatches,
    ParserExpectedOursMatches,
    ParserOursMatchesExpected,
    ParserExpectedSvelteMatches,
    FormatterInputIdempotent,
    FormatterMatchesPrettier,
    NormalizationVariantsOk(usize), // number of variants checked
    VariantVariantsOk(usize),       // number of variant_* checked
    NormalizationSkipped,           // skipped due to formatter failure
    InvalidSyntaxVariantsOk(usize), // number of invalid syntax files validated
}

impl fmt::Display for ValidationSuccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructureValid(n) => write!(f, "{n} structure checks passed"),
            Self::ParserExpectedJsonMatches => write!(f, "expected.json matches Svelte parser"),
            Self::ParserExpectedOursMatches => write!(f, "expected_ours.json matches our parser"),
            Self::ParserOursMatchesExpected => {
                write!(f, "our parser output matches expected.json")
            }
            Self::ParserExpectedSvelteMatches => {
                write!(f, "expected_svelte.json matches Svelte parser")
            }
            Self::FormatterInputIdempotent => write!(f, "input file is idempotent"),
            Self::FormatterMatchesPrettier => write!(f, "input file matches prettier"),
            Self::NormalizationVariantsOk(n) => write!(f, "{n} variants normalize correctly"),
            Self::VariantVariantsOk(n) => {
                write!(f, "{n} variant_* variants validated")
            }
            Self::NormalizationSkipped => write!(f, "SKIPPED (formatter not idempotent)"),
            Self::InvalidSyntaxVariantsOk(n) => {
                write!(f, "{n} invalid syntax files correctly rejected")
            }
        }
    }
}

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

/// Context for cross-fixture duplicate detection (internal use only)
#[derive(Debug, Default)]
struct DuplicateDetector {
    /// Map from content hash to list of fixture paths with that content
    input_hashes: HashMap<u64, Vec<String>>,
}

impl DuplicateDetector {
    fn record(&mut self, fixture_path: &str, content: &str, input_file: &str) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Include input file name so different file types (input.ts, input.css, input.svelte)
        // with identical content don't collide
        input_file.hash(&mut hasher);
        content.hash(&mut hasher);
        let hash = hasher.finish();

        self.input_hashes
            .entry(hash)
            .or_default()
            .push(fixture_path.to_string());
    }

    fn find_duplicates(&self) -> Vec<Vec<String>> {
        self.input_hashes
            .values()
            .filter(|paths| paths.len() > 1)
            .cloned()
            .collect()
    }
}

/// Validate a single fixture, collecting all errors
///
/// When `prettier_only` is true, skips our parser/formatter validation.
/// This is useful for validating fixture design before implementing features.
pub async fn validate_fixture(fixture: &Fixture, prettier_only: bool) -> FixtureValidation {
    let mut result = FixtureValidation::new(fixture.relative_path.clone());

    // Phase 1: Structure validation (pure Rust)
    if let Err(e) = fixtures::validate_fixture_structure(fixture) {
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
        // Phase 2: Our Parser validation - P2 (pure Rust)
        validate_parser_ours(&mut result, fixture, &input);

        // Phase 2b: Our parser matches expected.json (non-divergence only, pure Rust)
        validate_parser_ours_matches_expected(&mut result, fixture, &input);

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

/// P2: Validate expected_ours.json matches our parser output
fn validate_parser_ours(result: &mut FixtureValidation, fixture: &Fixture, input: &str) {
    let expected_ours_path = fixture.expected_ours_path();
    if !expected_ours_path.exists() {
        return;
    }

    let expected = match read_file(&expected_ours_path) {
        Ok(s) => s,
        Err(e) => {
            result.add_error(ValidationError::ParserError(format!(
                "Failed to read expected_ours.json: {e}"
            )));
            return;
        }
    };

    match fixtures::parse_with_our_parser_to_string(input, &fixture.input_file) {
        Ok(actual) => {
            if actual != expected {
                result.add_error(ValidationError::ParserExpectedOursOutdated);
            } else {
                result.add_success(ValidationSuccess::ParserExpectedOursMatches);
            }
        }
        Err(e) => {
            // Use context-aware error for svelte_divergence fixtures
            if fixture.is_svelte_divergence() {
                result.add_error(ValidationError::ParserErrorInDivergence(e));
            } else {
                result.add_error(ValidationError::ParserError(e));
            }
        }
    }
}

/// Validate our parser output matches expected.json (non-divergence fixtures only)
///
/// For non-divergence fixtures, expected.json should match both the canonical parser
/// AND our parser. Uses semantic (serde_json::Value) comparison to ignore field ordering
/// differences between our parser and the canonical parser.
fn validate_parser_ours_matches_expected(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
) {
    // Only for non-divergence fixtures that have expected.json but not expected_ours.json
    if fixture.expected_ours_path().exists() {
        return; // Divergence fixture — P2 handles this
    }
    let expected_path = fixture.expected_path();
    if !expected_path.exists() {
        return;
    }

    let expected_str = match read_file(&expected_path) {
        Ok(s) => s,
        Err(e) => {
            result.add_error(ValidationError::ParserError(format!(
                "Failed to read expected.json: {e}"
            )));
            return;
        }
    };

    let expected_json: serde_json::Value = match serde_json::from_str(&expected_str) {
        Ok(v) => v,
        Err(e) => {
            result.add_error(ValidationError::ParserError(format!(
                "Failed to parse expected.json: {e}"
            )));
            return;
        }
    };

    match fixtures::parse_with_our_parser_to_string(input, &fixture.input_file) {
        Ok(actual_str) => {
            let actual_json: serde_json::Value = match serde_json::from_str(&actual_str) {
                Ok(v) => v,
                Err(e) => {
                    result.add_error(ValidationError::ParserError(format!(
                        "Failed to parse our parser output as JSON: {e}"
                    )));
                    return;
                }
            };

            if actual_json == expected_json {
                result.add_success(ValidationSuccess::ParserOursMatchesExpected);
            } else {
                result.add_error(ValidationError::ParserOursDiffersFromExpected);
            }
        }
        Err(e) => {
            result.add_error(ValidationError::ParserError(e));
        }
    }
}

/// F1: Validate input file formats to itself
fn validate_formatter_idempotent(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
) -> bool {
    match fixtures::format_with_our_formatter(input, &fixture.input_file) {
        Ok(formatted) => {
            if formatted != *input {
                result.add_error(ValidationError::FormatterInputNotIdempotent(
                    fixture.input_file.clone(),
                ));
                diff::print_diff_with_options(
                    &format!(
                        "idempotency: {}/{}",
                        fixture.relative_path, fixture.input_file
                    ),
                    &formatted,
                    input,
                    &diff::DiffOptions::idempotency(),
                );
                false
            } else {
                result.add_success(ValidationSuccess::FormatterInputIdempotent);
                true
            }
        }
        Err(e) => {
            // Use context-aware error for svelte_divergence fixtures
            if fixture.is_svelte_divergence() {
                result.add_error(ValidationError::FormatterErrorInDivergence(e));
            } else {
                result.add_error(ValidationError::FormatterError(e));
            }
            false
        }
    }
}

/// N2, N4, N5: Validate our formatter normalizes variants to input file
fn validate_normalization_ours(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
) {
    let fixture_dir = &fixture.path;
    let mut total_variants = 0;

    // N2: prettier_variant_* → input file (our formatter)
    let prettier_variant_variants = discover_prettier_variant_variants(fixture_dir, input_ext);
    result.prettier_variant_count = prettier_variant_variants.len();
    let mut pv_contents: HashMap<String, Vec<String>> = HashMap::new();

    for pv_name in &prettier_variant_variants {
        let pv_path = fixture_dir.join(pv_name);
        let Ok(pv_content) = read_file(&pv_path) else {
            continue;
        };

        // Track for duplicate detection
        pv_contents
            .entry(pv_content.clone())
            .or_default()
            .push(pv_name.clone());

        match fixtures::format_with_our_formatter(&pv_content, pv_name) {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationPrettierVariantNotNormalized(
                        pv_name.clone(),
                    ));
                    diff::print_diff_with_options(
                        &format!("normalization: {}/{}", fixture.relative_path, pv_name),
                        &formatted,
                        input,
                        &diff::DiffOptions::idempotency(),
                    );
                } else {
                    total_variants += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!("{pv_name}: {e}")));
            }
        }
    }

    // Check for duplicate prettier_variant files
    for variants in pv_contents.values() {
        if variants.len() > 1 {
            result.add_error(ValidationError::DuplicatePrettierVariantWithinFixture(
                variants.clone(),
            ));
        }
    }

    // Check for prettier_variant files identical to output_prettier (redundant)
    let output_prettier_path = fixture.output_prettier_path();
    if output_prettier_path.exists()
        && let Ok(output_prettier_content) = read_file(&output_prettier_path)
    {
        for (pv_content, pv_files) in &pv_contents {
            if *pv_content == output_prettier_content {
                for pv_file in pv_files {
                    result.add_error(
                        ValidationError::RedundantPrettierVariantMatchesOutputPrettier(
                            pv_file.clone(),
                        ),
                    );
                }
            }
        }
    }

    // N4: unformatted_* → input file (our formatter)
    let unformatted_variants = discover_unformatted_variants(fixture_dir, input_ext);
    result.unformatted_count = unformatted_variants.len();
    let mut unformatted_contents: HashMap<String, Vec<String>> = HashMap::new();

    for variant_name in &unformatted_variants {
        let variant_path = fixture_dir.join(variant_name);
        let Ok(variant_content) = read_file(&variant_path) else {
            continue;
        };

        // Track for duplicate detection
        unformatted_contents
            .entry(variant_content.clone())
            .or_default()
            .push(variant_name.clone());

        match fixtures::format_with_our_formatter(&variant_content, &fixture.input_file) {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedNotNormalized(
                        variant_name.clone(),
                    ));
                    diff::print_diff_with_options(
                        &format!("normalization: {}/{}", fixture.relative_path, variant_name),
                        &formatted,
                        input,
                        &diff::DiffOptions::idempotency(),
                    );
                } else {
                    total_variants += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "{variant_name}: {e}"
                )));
            }
        }
    }

    // Check for duplicate unformatted files
    for variants in unformatted_contents.values() {
        if variants.len() > 1 {
            result.add_error(ValidationError::DuplicateUnformattedWithinFixture(
                variants.clone(),
            ));
        }
    }

    // Check for redundant unformatted files (identical to prettier_variant)
    for (unformatted_content, unformatted_files) in &unformatted_contents {
        for (pv_content, pv_files) in &pv_contents {
            if unformatted_content == pv_content {
                for unformatted_file in unformatted_files {
                    result.add_error(ValidationError::RedundantUnformattedMatchesPrettierVariant(
                        unformatted_file.clone(),
                        pv_files[0].clone(),
                    ));
                }
            }
        }
    }

    // N5: unformatted_ours_* → input file (our formatter only)
    let unformatted_ours_variants = discover_unformatted_ours_variants(fixture_dir, input_ext);
    result.unformatted_ours_count = unformatted_ours_variants.len();

    for variant_name in unformatted_ours_variants {
        let variant_path = fixture_dir.join(&variant_name);
        let Ok(variant_content) = read_file(&variant_path) else {
            continue;
        };

        match fixtures::format_with_our_formatter(&variant_content, &fixture.input_file) {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedOursNotNormalized(
                        variant_name.clone(),
                    ));
                    diff::print_diff_with_options(
                        &format!("normalization: {}/{}", fixture.relative_path, variant_name),
                        &formatted,
                        input,
                        &diff::DiffOptions::idempotency(),
                    );
                } else {
                    total_variants += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "{variant_name}: {e}"
                )));
            }
        }
    }

    // N9b, N9c: variant_* validation (our formatter)
    // N9b: ours(ours(file)) == ours(file) — our output is idempotent
    // N9c: ours(file) != input — must NOT normalize to input (else should be prettier_variant_*)
    let variant_variants = discover_variant_variants(fixture_dir, input_ext);
    result.variant_count = variant_variants.len();
    let mut variant_contents: HashMap<String, Vec<String>> = HashMap::new();
    let mut variant_ok = 0;

    for stable_name in &variant_variants {
        let stable_path = fixture_dir.join(stable_name);
        let Ok(stable_content) = read_file(&stable_path) else {
            continue;
        };

        // Track for duplicate detection
        variant_contents
            .entry(stable_content.clone())
            .or_default()
            .push(stable_name.clone());

        match fixtures::format_with_our_formatter(&stable_content, &fixture.input_file) {
            Ok(formatted) => {
                // N9c: Must NOT normalize to input
                if formatted == *input {
                    result.add_error(ValidationError::NormalizationVariantNormalizesToInput(
                        stable_name.clone(),
                    ));
                    continue;
                }

                // N9b: Our output must be idempotent (format the result again)
                match fixtures::format_with_our_formatter(&formatted, &fixture.input_file) {
                    Ok(second_pass) => {
                        if second_pass != formatted {
                            result.add_error(
                                ValidationError::NormalizationVariantOursNotIdempotent(
                                    stable_name.clone(),
                                ),
                            );
                            diff::print_diff_with_options(
                                &format!(
                                    "variant idempotency: {}/{}",
                                    fixture.relative_path, stable_name
                                ),
                                &formatted,
                                &second_pass,
                                &diff::DiffOptions::idempotency(),
                            );
                        } else {
                            variant_ok += 1;
                        }
                    }
                    Err(e) => {
                        result.add_error(ValidationError::FormatterError(format!(
                            "{stable_name} (second pass): {e}"
                        )));
                    }
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "{stable_name}: {e}"
                )));
            }
        }
    }

    // Check for duplicate variant files
    for variants in variant_contents.values() {
        if variants.len() > 1 {
            result.add_error(ValidationError::DuplicateVariantWithinFixture(
                variants.clone(),
            ));
        }
    }

    if variant_ok > 0 {
        result.add_success(ValidationSuccess::VariantVariantsOk(variant_ok));
    }

    if total_variants > 0 {
        result.add_success(ValidationSuccess::NormalizationVariantsOk(total_variants));
    }
}

/// P1, P3: Validate expected.json and expected_svelte.json match external parser
///
/// For Svelte fixtures: uses Svelte's parser
/// For TypeScript and SvelteTs fixtures: uses acorn+typescript parser
async fn validate_parser_external(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_type: InputType,
) {
    // CSS fixtures use Svelte's parseCss as the external canonical source
    if input_type == InputType::Css {
        let expected_path = fixture.expected_path();
        if !expected_path.exists() {
            return;
        }
        let Ok(expected_content) = read_file(&expected_path) else {
            return;
        };
        match parse_css(input).await {
            Ok(css_ast) => {
                let css_ast_json = match to_json_with_tabs(&css_ast) {
                    Ok(json) => format!("{json}\n"),
                    Err(e) => {
                        result.add_error(ValidationError::ParserError(format!(
                            "Failed to serialize CSS AST: {e}"
                        )));
                        return;
                    }
                };
                if expected_content != css_ast_json {
                    result.add_error(ValidationError::ParserExpectedJsonOutdated);
                } else {
                    result.add_success(ValidationSuccess::ParserExpectedJsonMatches);
                }
            }
            Err(e) => {
                result.add_error(ValidationError::ParserError(format!(
                    "CSS parser (parseCss) failed: {e}"
                )));
            }
        }
        return;
    }
    let expected_path = fixture.expected_path();
    let expected_svelte_path = fixture.expected_svelte_path();

    let expected_content = if expected_path.exists() {
        read_file(&expected_path).ok()
    } else {
        None
    };

    let expected_svelte_content = if expected_svelte_path.exists() {
        read_file(&expected_svelte_path).ok()
    } else {
        None
    };

    // Check if expected_svelte.json signals expected parse failure
    let expected_svelte_failure = matches!(
        &expected_svelte_content,
        Some(content) if content == fixtures::EXPECTED_SVELTE_ERROR_JSON
    );

    if expected_content.is_none() && expected_svelte_content.is_none() {
        return;
    }

    // TypeScript and SvelteTs fixtures use acorn+typescript, not Svelte parser
    if input_type == InputType::TypeScript || input_type == InputType::SvelteTs {
        let Some(expected_str) = &expected_content else {
            return; // No expected.json to validate
        };

        match parse_typescript(input).await {
            Ok(ts_ast) => {
                let ts_ast_json = match to_json_with_tabs(&ts_ast) {
                    Ok(json) => format!("{json}\n"),
                    Err(e) => {
                        result.add_error(ValidationError::ParserError(format!(
                            "Failed to serialize TypeScript AST: {e}"
                        )));
                        return;
                    }
                };

                if *expected_str != ts_ast_json {
                    result.add_error(ValidationError::ParserExpectedJsonOutdated);
                } else {
                    result.add_success(ValidationSuccess::ParserExpectedJsonMatches);
                }
            }
            Err(e) => {
                result.add_error(ValidationError::ParserError(format!(
                    "TypeScript parser (acorn) failed: {e}"
                )));
            }
        }
        return;
    }

    // Svelte fixtures use Svelte's parser
    match parse_svelte(input).await {
        Ok(svelte_ast) => {
            if expected_svelte_failure {
                result.add_error(ValidationError::ParserExpectedSvelteOutdated);
                return;
            }

            let svelte_ast_json = match to_json_with_tabs(&svelte_ast) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    result.add_error(ValidationError::ParserError(format!(
                        "Failed to serialize Svelte AST: {e}"
                    )));
                    return;
                }
            };

            // P1: Check expected.json (only if not in svelte divergence dir)
            if let Some(expected_str) = &expected_content {
                if *expected_str != svelte_ast_json {
                    result.add_error(ValidationError::ParserExpectedJsonOutdated);
                } else {
                    result.add_success(ValidationSuccess::ParserExpectedJsonMatches);
                }
            }

            // P3: Check expected_svelte.json
            if let Some(expected_svelte_str) = &expected_svelte_content {
                if !expected_svelte_failure && *expected_svelte_str != svelte_ast_json {
                    result.add_error(ValidationError::ParserExpectedSvelteOutdated);
                } else if !expected_svelte_failure {
                    result.add_success(ValidationSuccess::ParserExpectedSvelteMatches);
                }
            }
        }
        Err(_) => {
            // Svelte parse failed - check if this was expected
            if !expected_svelte_failure && expected_svelte_content.is_some() {
                result.add_error(ValidationError::ParserExpectedSvelteOutdated);
            } else if expected_svelte_failure {
                result.add_success(ValidationSuccess::ParserExpectedSvelteMatches);
            }
        }
    }
}

/// F2, F3: Validate formatter output matches prettier
async fn validate_formatter_prettier(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
) {
    let output_prettier_path = fixture.output_prettier_path();
    let output_prettier_filename = fixture.output_prettier_filename();

    let formatted = match run_prettier(input, fixture.input_type().prettier_parser()).await {
        Ok(f) => f,
        Err(e) => {
            result.add_error(ValidationError::FormatterError(format!("Prettier: {e}")));
            return;
        }
    };

    if output_prettier_path.exists() {
        // F2: Check output_prettier file matches prettier
        if let Ok(expected_prettier) = read_file(&output_prettier_path) {
            if expected_prettier != formatted {
                result.add_error(ValidationError::FormatterOutputPrettierOutdated);
                diff::print_diff_with_options(
                    &format!(
                        "outdated: {}/{}",
                        fixture.relative_path, output_prettier_filename
                    ),
                    &expected_prettier,
                    &formatted,
                    &diff::DiffOptions::freshness(),
                );
            } else {
                result.add_success(ValidationSuccess::FormatterMatchesPrettier);
            }

            // F4: When audit_signature.txt exists, byte-equality-check the entire
            // prettier-chain from output_prettier to its fixed point. Catches drift
            // in pass-2+ outputs that F2's pass-1 check would miss.
            validate_audit_signature(result, fixture, &expected_prettier).await;
        }
    } else {
        // F3: No output_prettier file - prettier(input) must equal input
        // This applies to ALL directories (including _prettier_divergence)
        if formatted != *input {
            result.add_error(ValidationError::FormatterInputDiffersFromPrettier(
                fixture.input_file.clone(),
            ));
            diff::print_diff_with_options(
                &format!(
                    "prettier mismatch: {}/{}",
                    fixture.relative_path, fixture.input_file
                ),
                input,
                &formatted,
                &diff::DiffOptions::input_vs_prettier(),
            );
        } else {
            result.add_success(ValidationSuccess::FormatterMatchesPrettier);
        }
    }
}

/// F4: Validate audit_signature.txt against the live prettier chain.
///
/// When the signature file exists, it pins prettier's multi-pass chain from
/// `output_prettier.*` to its fixed point. This catches drift that F2 (pass-1 only)
/// would miss — if prettier's pass-2+ output changes byte-for-byte, F4 fails.
///
/// When the signature file is absent, this check is skipped: most fixtures have
/// prettier idempotent on `output_prettier`, so no signature is needed.
async fn validate_audit_signature(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    output_prettier_content: &str,
) {
    let signature_path = fixture.audit_signature_path();
    if !signature_path.exists() {
        return;
    }

    let recorded_raw = match read_file(&signature_path) {
        Ok(s) => s,
        Err(e) => {
            result.add_error(ValidationError::FormatterAuditSignatureMalformed(e));
            return;
        }
    };
    let recorded = match AuditSignature::parse(&recorded_raw) {
        Ok(s) => s,
        Err(e) => {
            result.add_error(ValidationError::FormatterAuditSignatureMalformed(e));
            return;
        }
    };

    let parser = fixture.input_type().prettier_parser();
    let live = match AuditSignature::walk(output_prettier_content, parser).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            // Prettier idempotent on output_prettier but signature file exists →
            // chain collapsed since capture; the regenerate will delete the file.
            result.add_error(ValidationError::FormatterAuditSignatureOutdated(
                AuditSignatureStaleness::Collapsed,
            ));
            return;
        }
        Err(e) => {
            // Distinct from `Malformed`: the signature parsed fine, but walking the
            // live chain failed (prettier error or non-converging chain). The remediation
            // differs — investigate the prettier failure or the input, don't blindly regenerate.
            result.add_error(ValidationError::FormatterAuditSignatureWalkFailed(e));
            return;
        }
    };

    if live.passes != recorded.passes {
        result.add_error(ValidationError::FormatterAuditSignatureOutdated(
            AuditSignatureStaleness::Drift,
        ));
        // Diff the first differing pass for actionable output
        let max_len = recorded.passes.len().max(live.passes.len());
        for i in 0..max_len {
            let recorded_step = recorded.passes.get(i).map_or("", String::as_str);
            let live_step = live.passes.get(i).map_or("", String::as_str);
            if recorded_step != live_step {
                let pass_num = i + 2;
                diff::print_diff_with_options(
                    &format!(
                        "audit_signature drift (pass={pass_num}): {}/audit_signature.txt",
                        fixture.relative_path
                    ),
                    recorded_step,
                    live_step,
                    &diff::DiffOptions::freshness(),
                );
                break;
            }
        }
    } else {
        result.add_success(ValidationSuccess::FormatterMatchesPrettier);
    }
}

/// N1, N3: Validate prettier normalization behavior
async fn validate_normalization_prettier(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
) {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    // N1: prettier(prettier_variant_*) == prettier_variant_* (prettier preserves its stable variants)
    let prettier_variant_variants = discover_prettier_variant_variants(fixture_dir, input_ext);
    for pv_name in &prettier_variant_variants {
        let pv_path = fixture_dir.join(pv_name);
        let Ok(pv_content) = read_file(&pv_path) else {
            continue;
        };

        match run_prettier(&pv_content, prettier_parser).await {
            Ok(formatted) => {
                if formatted != pv_content {
                    result.add_error(ValidationError::NormalizationPrettierVariantNotPreserved(
                        pv_name.clone(),
                    ));
                    diff::print_diff_with_options(
                        &format!(
                            "prettier_variant not preserved: {}/{}",
                            fixture.relative_path, pv_name
                        ),
                        &pv_content,
                        &formatted,
                        &diff::DiffOptions::prettier_behavior(),
                    );
                }
            }
            Err(_) => continue,
        }
    }

    // N9a: prettier(variant_*) == variant_* (prettier preserves these too)
    let variant_variants = discover_variant_variants(fixture_dir, input_ext);
    for stable_name in &variant_variants {
        let stable_path = fixture_dir.join(stable_name);
        let Ok(stable_content) = read_file(&stable_path) else {
            continue;
        };

        match run_prettier(&stable_content, prettier_parser).await {
            Ok(formatted) => {
                if formatted != stable_content {
                    result.add_error(ValidationError::NormalizationVariantNotPreserved(
                        stable_name.clone(),
                    ));
                    diff::print_diff_with_options(
                        &format!(
                            "variant not preserved: {}/{}",
                            fixture.relative_path, stable_name
                        ),
                        &stable_content,
                        &formatted,
                        &diff::DiffOptions::prettier_behavior(),
                    );
                }
            }
            Err(_) => continue,
        }
    }

    // N3: prettier(unformatted_*) == input
    // Skip if prettier_variants exist (prettier won't normalize due to variants)
    if !prettier_variant_variants.is_empty() {
        return;
    }

    let unformatted_variants = discover_unformatted_variants(fixture_dir, input_ext);
    // Update count for reporting (may not be set if validate_normalization_ours was skipped)
    if result.unformatted_count == 0 {
        result.unformatted_count = unformatted_variants.len();
    }
    for variant_name in &unformatted_variants {
        let variant_path = fixture_dir.join(variant_name);
        let Ok(variant_content) = read_file(&variant_path) else {
            continue;
        };

        match run_prettier(
            &variant_content,
            PrettierParser::Filepath(&fixture.input_file),
        )
        .await
        {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedPrettierMismatch(
                        variant_name.clone(),
                    ));
                    diff::print_diff_with_options(
                        &format!(
                            "prettier normalization: {}/{}",
                            fixture.relative_path, variant_name
                        ),
                        input,
                        &formatted,
                        &diff::DiffOptions::prettier_behavior(),
                    );
                }
            }
            Err(_) => continue,
        }
    }

    // N6: prettier(unformatted_ours_*) != input
    // unformatted_ours_* files claim that only our formatter normalizes them to input,
    // so prettier should NOT normalize them to input (otherwise they should be unformatted_*)
    let unformatted_ours_variants = discover_unformatted_ours_variants(fixture_dir, input_ext);

    // Build a map of unformatted_ours_* outputs for prettier_intermediate_* validation
    let mut unformatted_ours_prettier_outputs: HashMap<String, String> = HashMap::new();

    for variant_name in &unformatted_ours_variants {
        let variant_path = fixture_dir.join(variant_name);
        let Ok(variant_content) = read_file(&variant_path) else {
            continue;
        };

        match run_prettier(
            &variant_content,
            PrettierParser::Filepath(&fixture.input_file),
        )
        .await
        {
            Ok(formatted) => {
                if formatted == *input {
                    // Prettier also normalizes to input - this should be unformatted_*, not unformatted_ours_*
                    result.add_error(
                        ValidationError::NormalizationUnformattedOursPrettierAlsoNormalizes(
                            variant_name.clone(),
                        ),
                    );
                } else {
                    // Store for prettier_intermediate_* validation
                    // Extract suffix: unformatted_ours_X.svelte -> X
                    let suffix = variant_name
                        .strip_prefix("unformatted_ours_")
                        .and_then(|s| s.strip_suffix(input_ext))
                        .unwrap_or("");
                    unformatted_ours_prettier_outputs.insert(suffix.to_string(), formatted);
                }
            }
            Err(_) => continue,
        }
    }

    // N7: prettier_intermediate_* validation
    // These files capture prettier's unstable first-pass output from unformatted_ours_* files
    let prettier_intermediate_variants =
        discover_prettier_intermediate_variants(fixture_dir, input_ext);
    result.prettier_intermediate_count = prettier_intermediate_variants.len();

    for intermediate_name in &prettier_intermediate_variants {
        let intermediate_path = fixture_dir.join(intermediate_name);
        let Ok(intermediate_content) = read_file(&intermediate_path) else {
            continue;
        };

        // Extract suffix: prettier_intermediate_X.svelte -> X
        let suffix = intermediate_name
            .strip_prefix("prettier_intermediate_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("");

        // Check 1: Must have corresponding unformatted_ours_* file
        let Some(expected_content) = unformatted_ours_prettier_outputs.get(suffix) else {
            result.add_error(
                ValidationError::NormalizationPrettierIntermediateMissingSource(
                    intermediate_name.clone(),
                ),
            );
            continue;
        };

        // Check 2: prettier(unformatted_ours_X) == prettier_intermediate_X
        if *expected_content != intermediate_content {
            result.add_error(ValidationError::NormalizationPrettierIntermediateMismatch(
                intermediate_name.clone(),
            ));
            diff::print_diff_with_options(
                &format!(
                    "prettier_intermediate mismatch: {}/{}",
                    fixture.relative_path, intermediate_name
                ),
                &intermediate_content,
                expected_content,
                &diff::DiffOptions::freshness(),
            );
            continue;
        }

        // Check 3: prettier(prettier_intermediate_X) != prettier_intermediate_X (must be unstable)
        match run_prettier(&intermediate_content, prettier_parser).await {
            Ok(second_pass) => {
                if second_pass == intermediate_content {
                    // It's stable - should be prettier_variant_* instead
                    result.add_error(ValidationError::NormalizationPrettierIntermediateIsStable(
                        intermediate_name.clone(),
                    ));
                    continue;
                }

                // Check 4: prettier(prettier_intermediate_X) == input (converges to stable form)
                if second_pass != *input {
                    result.add_error(
                        ValidationError::NormalizationPrettierIntermediateNotConverging(
                            intermediate_name.clone(),
                        ),
                    );
                    diff::print_diff_with_options(
                        &format!(
                            "prettier_intermediate not converging: {}/{}",
                            fixture.relative_path, intermediate_name
                        ),
                        &second_pass,
                        input,
                        &diff::DiffOptions::prettier_behavior(),
                    );
                }
            }
            Err(_) => continue,
        }
    }

    // N7b: prettier_intermediate_to_variant_* validation
    // Like N7, but the second pass must converge to a documented variant_*/prettier_variant_*
    // file (not input).
    let prettier_intermediate_to_variant_variants =
        discover_prettier_intermediate_to_variant_variants(fixture_dir, input_ext);
    result.prettier_intermediate_to_variant_count = prettier_intermediate_to_variant_variants.len();

    // Pre-read variant_*/prettier_variant_* contents — these are the allowed convergence targets.
    let mut variant_target_contents: Vec<String> = Vec::new();
    for pv_name in &prettier_variant_variants {
        if let Ok(content) = read_file(&fixture_dir.join(pv_name)) {
            variant_target_contents.push(content);
        }
    }
    for v_name in &variant_variants {
        if let Ok(content) = read_file(&fixture_dir.join(v_name)) {
            variant_target_contents.push(content);
        }
    }

    for intermediate_name in &prettier_intermediate_to_variant_variants {
        let intermediate_path = fixture_dir.join(intermediate_name);
        let Ok(intermediate_content) = read_file(&intermediate_path) else {
            continue;
        };

        // Extract suffix: prettier_intermediate_to_variant_X.svelte -> X
        let suffix = intermediate_name
            .strip_prefix("prettier_intermediate_to_variant_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("");

        // Check 1: Must have corresponding unformatted_ours_* file
        let Some(expected_content) = unformatted_ours_prettier_outputs.get(suffix) else {
            result.add_error(
                ValidationError::NormalizationPrettierIntermediateToVariantMissingSource(
                    intermediate_name.clone(),
                ),
            );
            continue;
        };

        // Check 2: must have at least one variant_*/prettier_variant_* file as convergence target
        if variant_target_contents.is_empty() {
            result.add_error(
                ValidationError::NormalizationPrettierIntermediateToVariantNoVariantTarget(
                    intermediate_name.clone(),
                ),
            );
            continue;
        }

        // Check 3: prettier(unformatted_ours_X) == prettier_intermediate_to_variant_X
        if *expected_content != intermediate_content {
            result.add_error(
                ValidationError::NormalizationPrettierIntermediateToVariantMismatch(
                    intermediate_name.clone(),
                ),
            );
            diff::print_diff_with_options(
                &format!(
                    "prettier_intermediate_to_variant mismatch: {}/{}",
                    fixture.relative_path, intermediate_name
                ),
                &intermediate_content,
                expected_content,
                &diff::DiffOptions::freshness(),
            );
            continue;
        }

        // Check 4: prettier(prettier_intermediate_to_variant_X) != prettier_intermediate_to_variant_X (unstable)
        match run_prettier(&intermediate_content, prettier_parser).await {
            Ok(second_pass) => {
                if second_pass == intermediate_content {
                    result.add_error(
                        ValidationError::NormalizationPrettierIntermediateToVariantIsStable(
                            intermediate_name.clone(),
                        ),
                    );
                    continue;
                }

                // Check 5: second pass must NOT equal input (else use prettier_intermediate_* instead)
                if second_pass == *input {
                    result.add_error(
                        ValidationError::NormalizationPrettierIntermediateToVariantConvergesToInput(
                            intermediate_name.clone(),
                        ),
                    );
                    continue;
                }

                // Check 6: second pass must match some variant_* / prettier_variant_* content
                let hits_variant = variant_target_contents.contains(&second_pass);
                if !hits_variant {
                    result.add_error(
                        ValidationError::NormalizationPrettierIntermediateToVariantNotConverging(
                            intermediate_name.clone(),
                        ),
                    );
                    if let Some(first_target) = variant_target_contents.first() {
                        diff::print_diff_with_options(
                            &format!(
                                "prettier_intermediate_to_variant not converging: {}/{}",
                                fixture.relative_path, intermediate_name
                            ),
                            &second_pass,
                            first_target,
                            &diff::DiffOptions::prettier_behavior(),
                        );
                    }
                }
            }
            Err(_) => continue,
        }
    }

    // N8: unformatted_prettier_* validation
    // These files test that prettier normalizes certain inputs to output_prettier.*
    let unformatted_prettier_variants =
        discover_unformatted_prettier_variants(fixture_dir, input_ext);
    result.unformatted_prettier_count = unformatted_prettier_variants.len();

    if !unformatted_prettier_variants.is_empty() {
        // Must have output_prettier.* to validate against
        let output_prettier_path = fixture.output_prettier_path();
        let output_prettier_content = if output_prettier_path.exists() {
            read_file(&output_prettier_path).ok()
        } else {
            None
        };

        for variant_name in &unformatted_prettier_variants {
            let variant_path = fixture_dir.join(variant_name);
            let Ok(variant_content) = read_file(&variant_path) else {
                continue;
            };

            // Check that output_prettier.* exists
            let Some(ref expected_output) = output_prettier_content else {
                result.add_error(
                    ValidationError::NormalizationUnformattedPrettierMissingTarget(
                        variant_name.clone(),
                    ),
                );
                continue;
            };

            // prettier(unformatted_prettier_*) == output_prettier.*
            match run_prettier(
                &variant_content,
                PrettierParser::Filepath(&fixture.input_file),
            )
            .await
            {
                Ok(formatted) => {
                    if formatted != *expected_output {
                        result.add_error(
                            ValidationError::NormalizationUnformattedPrettierNotNormalized(
                                variant_name.clone(),
                            ),
                        );
                        diff::print_diff_with_options(
                            &format!(
                                "prettier normalization to output_prettier: {}/{}",
                                fixture.relative_path, variant_name
                            ),
                            expected_output,
                            &formatted,
                            &diff::DiffOptions::prettier_behavior(),
                        );
                    }
                }
                Err(_) => continue,
            }
        }
    }

    // N10: Cross-path discovery — find undocumented Prettier outputs
    // After N7, check which unformatted_ours_* prettier outputs weren't consumed by prettier_intermediate_*
    // Then check if those outputs match any known file content (output_prettier, prettier_variant_*, variant_*)
    {
        // Build set of suffixes claimed by prettier_intermediate_* and prettier_intermediate_to_variant_*
        let mut claimed_suffixes: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for intermediate_name in &prettier_intermediate_variants {
            let suffix = intermediate_name
                .strip_prefix("prettier_intermediate_")
                .and_then(|s| s.strip_suffix(input_ext))
                .unwrap_or("")
                .to_string();
            claimed_suffixes.insert(suffix);
        }
        for intermediate_name in &prettier_intermediate_to_variant_variants {
            let suffix = intermediate_name
                .strip_prefix("prettier_intermediate_to_variant_")
                .and_then(|s| s.strip_suffix(input_ext))
                .unwrap_or("")
                .to_string();
            claimed_suffixes.insert(suffix);
        }

        // Also claim suffixes where prettier(unformatted_ours_*) == input (those got N6 errors, not novel)
        // These are already not in unformatted_ours_prettier_outputs (they were flagged as errors)

        // Build known content set from output_prettier, prettier_variant_*, variant_*
        let mut known_contents: Vec<String> = Vec::new();

        // output_prettier content
        let output_prettier_path = fixture.output_prettier_path();
        if output_prettier_path.exists()
            && let Ok(content) = read_file(&output_prettier_path)
        {
            known_contents.push(content);
        }

        // prettier_variant_* contents
        for pv_name in &prettier_variant_variants {
            let pv_path = fixture_dir.join(pv_name);
            if let Ok(content) = read_file(&pv_path) {
                known_contents.push(content);
            }
        }

        // variant_* contents
        for stable_name in &variant_variants {
            let stable_path = fixture_dir.join(stable_name);
            if let Ok(content) = read_file(&stable_path) {
                known_contents.push(content);
            }
        }

        // Check unclaimed outputs
        for (suffix, prettier_output) in &unformatted_ours_prettier_outputs {
            if claimed_suffixes.contains(suffix) {
                continue;
            }

            // Check against input
            if *prettier_output == *input {
                continue; // Already flagged by N6
            }

            // Check against known contents
            let is_known = known_contents.iter().any(|c| c == prettier_output);
            if !is_known {
                let source_file = format!("unformatted_ours_{suffix}{input_ext}");
                // When the fixture documents prettier's stable forms (it has
                // output_prettier / prettier_variant_* / variant_* files), every
                // unformatted_ours_* prettier output must match one of them — an
                // unmatched output means prettier drifted or the target is
                // undocumented, so block. Fixtures that document the divergence by
                // README alone (no stable-form files) keep this informational.
                if known_contents.is_empty() {
                    result
                        .undocumented_prettier_outputs
                        .push(UndocumentedPrettierOutput {
                            source_file,
                            suffix: suffix.clone(),
                        });
                } else {
                    result.add_error(ValidationError::UndocumentedPrettierOutput(source_file));
                }
            }
        }
    }
}

/// Validate input_invalid_* files: must fail to parse with both our parser and canonical parser
///
/// For Svelte files: both our parser and Svelte's parser must fail
/// For TypeScript and SvelteTs files: both our parser and acorn-typescript must fail
/// For CSS files: our parser must fail (no canonical source)
async fn validate_invalid_syntax(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input_type: InputType,
    input_ext: &str,
) {
    let fixture_dir = &fixture.path;
    let invalid_variants = discover_invalid_variants(fixture_dir, input_ext);

    if invalid_variants.is_empty() {
        return;
    }

    result.invalid_syntax_count = invalid_variants.len();
    let mut valid_count = 0;

    for variant_name in &invalid_variants {
        let variant_path = fixture_dir.join(variant_name);
        let Ok(variant_content) = read_file(&variant_path) else {
            continue;
        };

        // Check our parser
        let ours_failed = match input_type {
            InputType::Svelte => tsv_svelte::parse(&variant_content).is_err(),
            InputType::SvelteTs | InputType::TypeScript => tsv_ts::parse(&variant_content).is_err(),
            InputType::Css => tsv_css::parse(&variant_content).is_err(),
        };

        // Check canonical parser
        let canonical_failed = match input_type {
            InputType::Svelte => parse_svelte(&variant_content).await.is_err(),
            InputType::SvelteTs | InputType::TypeScript => {
                parse_typescript(&variant_content).await.is_err()
            }
            InputType::Css => parse_css(&variant_content).await.is_err(),
        };

        // Evaluate results - both must fail for a valid invalid-syntax test
        match (ours_failed, canonical_failed) {
            (true, true) => {
                // Good - both parsers reject it
                valid_count += 1;
            }
            (false, true) => {
                // Our parser is too permissive
                let error = if input_type == InputType::Css {
                    ValidationError::InvalidSyntaxParsedByOurCss(variant_name.clone())
                } else {
                    ValidationError::InvalidSyntaxParsedByOurs(variant_name.clone())
                };
                result.add_error(error);
            }
            (true, false) => {
                // Canonical accepts it - file isn't actually invalid
                let error = match input_type {
                    InputType::Svelte => {
                        ValidationError::InvalidSyntaxParsedBySvelte(variant_name.clone())
                    }
                    InputType::Css => {
                        ValidationError::InvalidSyntaxParsedByParseCss(variant_name.clone())
                    }
                    InputType::SvelteTs | InputType::TypeScript => {
                        ValidationError::InvalidSyntaxParsedByAcorn(variant_name.clone())
                    }
                };
                result.add_error(error);
            }
            (false, false) => {
                // Both accept it - file isn't actually invalid
                // Report the canonical parser accepting it (more authoritative)
                let error = match input_type {
                    InputType::Svelte => {
                        ValidationError::InvalidSyntaxParsedBySvelte(variant_name.clone())
                    }
                    InputType::Css => {
                        ValidationError::InvalidSyntaxParsedByParseCss(variant_name.clone())
                    }
                    InputType::SvelteTs | InputType::TypeScript => {
                        ValidationError::InvalidSyntaxParsedByAcorn(variant_name.clone())
                    }
                };
                result.add_error(error);
            }
        }
    }

    if valid_count > 0 {
        result.add_success(ValidationSuccess::InvalidSyntaxVariantsOk(valid_count));
    }
}

/// Aggregate results from validating multiple fixtures
#[derive(Debug, Default)]
pub struct ValidationSummary {
    pub total_fixtures: usize,
    pub passed_fixtures: usize,
    pub failed_fixtures: usize,
    pub total_unformatted: usize,
    pub total_unformatted_ours: usize,
    pub total_unformatted_prettier: usize,
    pub total_prettier_variant: usize,
    pub total_variant: usize,
    pub total_prettier_intermediate: usize,
    pub total_prettier_intermediate_to_variant: usize,
    pub total_invalid_syntax: usize,
    pub results: Vec<FixtureValidation>,
    pub cross_fixture_duplicates: Vec<Vec<String>>,
    pub total_undocumented_prettier: usize,
}

impl ValidationSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, result: FixtureValidation) {
        self.total_fixtures += 1;
        self.total_unformatted += result.unformatted_count;
        self.total_unformatted_ours += result.unformatted_ours_count;
        self.total_unformatted_prettier += result.unformatted_prettier_count;
        self.total_prettier_variant += result.prettier_variant_count;
        self.total_variant += result.variant_count;
        self.total_prettier_intermediate += result.prettier_intermediate_count;
        self.total_prettier_intermediate_to_variant +=
            result.prettier_intermediate_to_variant_count;
        self.total_invalid_syntax += result.invalid_syntax_count;
        self.total_undocumented_prettier += result.undocumented_prettier_outputs.len();

        if result.is_valid() {
            self.passed_fixtures += 1;
        } else {
            self.failed_fixtures += 1;
        }

        self.results.push(result);
    }

    /// Build duplicate detection from collected results
    pub fn detect_cross_fixture_duplicates(&mut self) {
        let mut detector = DuplicateDetector::default();
        for result in &self.results {
            if let Some(ref content) = result.input_content {
                let input_file = result.input_file_name.as_deref().unwrap_or("input.svelte");
                detector.record(&result.fixture_path, content, input_file);
            }
        }
        self.cross_fixture_duplicates = detector.find_duplicates();
    }

    pub fn is_valid(&self) -> bool {
        self.failed_fixtures == 0 && self.cross_fixture_duplicates.is_empty()
    }

    pub fn failed_results(&self) -> impl Iterator<Item = &FixtureValidation> {
        self.results.iter().filter(|r| r.has_errors())
    }

    /// Count fixtures that failed due to Deno sidecar shutdown
    ///
    /// A high count indicates the sidecar crashed during the test run,
    /// causing cascading failures that aren't real fixture issues.
    ///
    /// Called from `tests/fixtures_tests.rs` (root-crate integration test).
    /// The `tsv_debug` binary doesn't use this, so the `#[allow]` silences a
    /// dead_code warning that only fires in the binary build.
    #[allow(dead_code)]
    pub fn count_sidecar_failures(&self) -> usize {
        self.results
            .iter()
            .filter(|r| {
                r.errors.iter().any(|e| {
                    matches!(e, ValidationError::FormatterError(msg) | ValidationError::ParserError(msg)
                        if msg.contains("deno actor shut down") || msg.contains("sidecar crashed"))
                })
            })
            .count()
    }

    /// Count fixtures that failed due to Deno sidecar timeout
    ///
    /// A high count indicates the sidecar is hanging on certain inputs,
    /// possibly due to a bug in prettier/acorn or resource exhaustion.
    ///
    /// Called from `tests/fixtures_tests.rs` — see note on `count_sidecar_failures`.
    #[allow(dead_code)]
    pub fn count_timeout_failures(&self) -> usize {
        self.results
            .iter()
            .filter(|r| {
                r.errors.iter().any(|e| {
                    matches!(e, ValidationError::FormatterError(msg) | ValidationError::ParserError(msg)
                        if msg.contains("timed out"))
                })
            })
            .count()
    }
}

/// Print validation results with per-fixture grouping
pub fn print_validation_results(summary: &ValidationSummary, verbose: bool) {
    let failed: Vec<_> = summary.failed_results().collect();

    // In verbose mode, print all fixtures including successes
    if verbose {
        for result in &summary.results {
            if result.is_valid() {
                println!("✓ {}", result.fixture_path);
                for success in &result.successes {
                    println!("    [OK] {success}");
                }
            } else {
                eprintln!("✗ {}", result.fixture_path);
                for success in &result.successes {
                    eprintln!("    [OK] {success}");
                }
                for error in &result.errors {
                    eprintln!("    [{}] {}", error.category(), error);
                    eprintln!("           Fix: {}", error.fix_hint());
                }
            }
            println!();
        }
    } else if !failed.is_empty() {
        // Print errors grouped by fixture with enhanced context
        eprintln!();
        for result in &failed {
            eprintln!("✗ {}", result.fixture_path);

            // Group errors by category for better scanning
            let mut by_category: std::collections::BTreeMap<&str, Vec<&ValidationError>> =
                std::collections::BTreeMap::new();
            for error in &result.errors {
                by_category.entry(error.category()).or_default().push(error);
            }

            for (category, errors) in by_category {
                let show_category_header = errors.len() > 1;
                if show_category_header {
                    eprintln!("    {category}:");
                }
                for error in &errors {
                    let prefix = if show_category_header {
                        "      "
                    } else {
                        "    "
                    };
                    eprintln!("{prefix}[{}] {}", error.category(), error);

                    // Show concrete command with actual fixture path
                    let fix_hint = error.fix_hint();
                    let concrete_cmd = fix_hint
                        .replace("<pattern>", &result.fixture_path)
                        .replace("<fixture>", &result.fixture_path);
                    eprintln!("{prefix}     → {concrete_cmd}");
                }
            }
            eprintln!();
        }
    }

    // Print cross-fixture duplicates
    if !summary.cross_fixture_duplicates.is_empty() {
        eprintln!("✗ Cross-fixture duplicates detected:");
        for group in &summary.cross_fixture_duplicates {
            eprintln!("    Duplicate input.svelte content:");
            for path in group {
                eprintln!("      - {path}");
            }
        }
        eprintln!();
    }

    // Print summary
    if failed.is_empty() && summary.cross_fixture_duplicates.is_empty() {
        let mut parts = vec![format!(
            "✓ All {} fixtures validated",
            summary.total_fixtures
        )];
        let mut variant_parts = Vec::new();
        if summary.total_unformatted > 0 {
            variant_parts.push(format!("{} unformatted_*", summary.total_unformatted));
        }
        if summary.total_unformatted_ours > 0 {
            variant_parts.push(format!(
                "{} unformatted_ours_*",
                summary.total_unformatted_ours
            ));
        }
        if summary.total_unformatted_prettier > 0 {
            variant_parts.push(format!(
                "{} unformatted_prettier_*",
                summary.total_unformatted_prettier
            ));
        }
        if summary.total_prettier_variant > 0 {
            variant_parts.push(format!(
                "{} prettier_variant_*",
                summary.total_prettier_variant
            ));
        }
        if summary.total_variant > 0 {
            variant_parts.push(format!("{} variant_*", summary.total_variant));
        }
        if summary.total_prettier_intermediate > 0 {
            variant_parts.push(format!(
                "{} prettier_intermediate_*",
                summary.total_prettier_intermediate
            ));
        }
        if summary.total_prettier_intermediate_to_variant > 0 {
            variant_parts.push(format!(
                "{} prettier_intermediate_to_variant_*",
                summary.total_prettier_intermediate_to_variant
            ));
        }
        if summary.total_invalid_syntax > 0 {
            variant_parts.push(format!("{} input_invalid_*", summary.total_invalid_syntax));
        }
        if !variant_parts.is_empty() {
            parts.push(format!("({})", variant_parts.join(", ")));
        }
        println!("{}", parts.join(" "));

        // N10: Print undocumented Prettier outputs as informational notes
        if summary.total_undocumented_prettier > 0 {
            println!();
            println!(
                "NOTE: {} undocumented Prettier output(s):",
                summary.total_undocumented_prettier
            );
            for result in &summary.results {
                for undoc in &result.undocumented_prettier_outputs {
                    println!("  {}/", result.fixture_path);
                    let source = &undoc.source_file;
                    println!("    Prettier({source}) produces a novel stable form");
                    // Extract fixture name for command suggestion
                    let fixture_name = result
                        .fixture_path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&result.fixture_path);
                    println!("    Investigate: deno task fixtures:audit {fixture_name}");
                }
            }
        }
    } else {
        eprintln!("════════════════════");
        eprintln!();
        eprintln!(
            "{} / {} fixtures failed:",
            summary.failed_fixtures, summary.total_fixtures
        );
        eprintln!();
        for result in &failed {
            eprintln!("  ✗ {}", result.fixture_path);
        }
        eprintln!();
        eprintln!(
            "Results Summary: {} passed, {} failed out of {} total",
            summary.passed_fixtures, summary.failed_fixtures, summary.total_fixtures
        );
    }
}
