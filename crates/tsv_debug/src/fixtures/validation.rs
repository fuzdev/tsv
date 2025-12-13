//! Unified fixture validation with per-fixture error grouping
//!
//! All validation errors for a single fixture are collected together,
//! enabling better DX with grouped error reporting.

use crate::diff;
use crate::deno::{parse_svelte, parse_typescript, run_prettier, PrettierParser};
use crate::fixtures::{
    self, Fixture, InputType, discover_prettier_quirk_variants, discover_unformatted_ours_variants,
    discover_unformatted_variants, has_prettier_divergence_suffix, has_svelte_divergence_suffix,
    read_file,
};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use thiserror::Error;

/// Validation error with self-describing names
///
/// Note: Specific structure variants (StructureMissing*, StructurePrettier*, etc.) are defined
/// but currently unused. Structure validation errors are wrapped in `StructureValidationFailed`
/// which preserves detailed messages from `validate_fixture_structure()`. The specific variants
/// document the intended type structure for future refactoring.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[allow(dead_code)]
pub enum ValidationError {
    // Structure - Missing/Invalid
    #[error("Missing input.svelte")]
    StructureMissingInput,
    #[error("Missing expected.json")]
    StructureMissingExpected,
    #[error("expected.json cannot coexist with expected_ours.json")]
    StructureExpectedJsonWithDivergenceFiles,

    // Structure - Svelte divergence
    #[error("Directory needs _svelte_divergence suffix")]
    StructureSvelteDivergenceMissingSuffix,
    #[error("_svelte_divergence dir missing expected_ours/svelte.json")]
    StructureSvelteDivergenceSuffixWithoutFiles,
    #[error("Missing expected_ours.json in _svelte_divergence dir")]
    StructureSvelteDivergenceMissingExpectedOurs,
    #[error("Missing expected_svelte.json in _svelte_divergence dir")]
    StructureSvelteDivergenceMissingExpectedSvelte,
    #[error("expected_ours.json requires _svelte_divergence suffix")]
    StructureExpectedOursWithoutSvelteDivergenceSuffix,
    #[error("expected_svelte.json requires _svelte_divergence suffix")]
    StructureExpectedSvelteWithoutSvelteDivergenceSuffix,
    #[error("expected_ours.json and expected_svelte.json are identical")]
    StructureSvelteDivergenceFilesIdentical,

    // Structure - Prettier divergence
    #[error("Directory needs _prettier_divergence suffix")]
    StructurePrettierDivergenceMissingSuffix,
    #[error("_prettier_divergence dir missing divergence files")]
    StructurePrettierDivergenceSuffixWithoutFiles,
    #[error("_prettier_divergence dir has unformatted_*.svelte: {}", .0.join(", "))]
    StructurePrettierDivergenceHasUnformatted(Vec<String>),
    #[error("{0} requires _prettier_divergence suffix")]
    StructurePrettierQuirkWithoutPrettierDivergenceSuffix(String),
    #[error("{0} requires _prettier_divergence suffix")]
    StructureUnformattedOursWithoutPrettierDivergenceSuffix(String),

    // Structure - Content validation
    #[error("{0} is identical to input.svelte")]
    StructureVariantIdenticalToInput(String),
    #[error("README.md required for divergence")]
    StructureMissingReadme,

    /// Generic structure validation failure with detailed message
    /// Used when validate_fixture_structure() returns an error string
    #[error("{0}")]
    StructureValidationFailed(String),

    // Parser
    #[error("expected.json is outdated")]
    ParserExpectedJsonOutdated,
    #[error("expected_ours.json is outdated")]
    ParserExpectedOursOutdated,
    #[error("expected_svelte.json is outdated")]
    ParserExpectedSvelteOutdated,
    #[error("Parser error: {0}")]
    ParserError(String),

    // Formatter
    #[error("input.svelte doesn't format to itself")]
    FormatterInputNotIdempotent,
    #[error("output_prettier file is outdated")]
    FormatterOutputPrettierOutdated,
    #[error("input file differs from prettier output")]
    FormatterInputDiffersFromPrettier,
    #[error("Formatter error: {0}")]
    FormatterError(String),

    // Normalization
    #[error("{0} not preserved by prettier")]
    NormalizationPrettierQuirkNotPreserved(String),
    #[error("{0} doesn't normalize to input.svelte")]
    NormalizationPrettierQuirkNotNormalized(String),
    #[error("{0} doesn't normalize to input.svelte (prettier)")]
    NormalizationUnformattedPrettierMismatch(String),
    #[error("{0} doesn't normalize to input.svelte")]
    NormalizationUnformattedNotNormalized(String),
    #[error("{0} doesn't normalize to input.svelte")]
    NormalizationUnformattedOursNotNormalized(String),

    // Duplicates (within fixture)
    #[error("Duplicate unformatted files: {}", .0.join(", "))]
    DuplicateUnformattedWithinFixture(Vec<String>),
    #[error("Duplicate prettier_quirk files: {}", .0.join(", "))]
    DuplicatePrettierQuirkWithinFixture(Vec<String>),
    #[error("{0} is redundant (identical to {1})")]
    RedundantUnformattedMatchesQuirk(String, String),
}

impl ValidationError {
    /// Suggested fix for this error
    pub fn fix_hint(&self) -> &'static str {
        match self {
            Self::StructureMissingInput => "Add input.svelte file",
            Self::StructureMissingExpected => "Run: deno task fixtures_update_parsed <pattern>",
            Self::StructureExpectedJsonWithDivergenceFiles
            | Self::StructureSvelteDivergenceMissingSuffix
            | Self::StructureSvelteDivergenceSuffixWithoutFiles
            | Self::StructureSvelteDivergenceMissingExpectedOurs
            | Self::StructureSvelteDivergenceMissingExpectedSvelte
            | Self::StructureExpectedOursWithoutSvelteDivergenceSuffix
            | Self::StructureExpectedSvelteWithoutSvelteDivergenceSuffix => {
                "See docs/fixture_overview.md for _svelte_divergence naming rules"
            }
            Self::StructureSvelteDivergenceFilesIdentical => {
                "Remove _svelte_divergence suffix if parsers match, or check fixture input"
            }
            Self::StructurePrettierDivergenceMissingSuffix
            | Self::StructurePrettierDivergenceSuffixWithoutFiles
            | Self::StructurePrettierDivergenceHasUnformatted(_)
            | Self::StructurePrettierQuirkWithoutPrettierDivergenceSuffix(_)
            | Self::StructureUnformattedOursWithoutPrettierDivergenceSuffix(_) => {
                "See docs/fixture_overview.md for _prettier_divergence naming rules"
            }
            Self::StructureVariantIdenticalToInput(_) => {
                "Remove the variant file (it's identical to input.svelte)"
            }
            Self::StructureMissingReadme => "Add README.md explaining the divergence",
            Self::StructureValidationFailed(_) => "See error message for details",
            Self::ParserExpectedJsonOutdated
            | Self::ParserExpectedOursOutdated
            | Self::ParserExpectedSvelteOutdated => {
                "Run: deno task fixtures_update_parsed <pattern>"
            }
            Self::ParserError(_) => "Check the input file syntax",
            Self::FormatterInputNotIdempotent => {
                "Debug: cargo run -p tsv_debug compare <fixture>/input.svelte"
            }
            Self::FormatterOutputPrettierOutdated => {
                "Run: deno task fixtures_update_formatted <pattern>"
            }
            Self::FormatterInputDiffersFromPrettier => {
                "Run: cargo run -p tsv_debug compare <fixture>/input.svelte to see difference"
            }
            Self::FormatterError(_) => "Fix the formatter implementation",
            Self::NormalizationPrettierQuirkNotPreserved(_) => {
                "Prettier doesn't preserve this file - rename to unformatted_*.svelte"
            }
            Self::NormalizationPrettierQuirkNotNormalized(_)
            | Self::NormalizationUnformattedNotNormalized(_)
            | Self::NormalizationUnformattedOursNotNormalized(_) => {
                "Fix formatter to normalize this variant correctly"
            }
            Self::NormalizationUnformattedPrettierMismatch(_) => {
                "Prettier doesn't normalize to input.svelte - check prettier behavior"
            }
            Self::DuplicateUnformattedWithinFixture(_)
            | Self::DuplicatePrettierQuirkWithinFixture(_) => {
                "Remove duplicate files (identical content)"
            }
            Self::RedundantUnformattedMatchesQuirk(_, _) => {
                "Remove redundant file (already covered by prettier_quirk_*.svelte)"
            }
        }
    }

    /// Get error category for grouping
    pub fn category(&self) -> &'static str {
        match self {
            Self::StructureMissingInput
            | Self::StructureMissingExpected
            | Self::StructureExpectedJsonWithDivergenceFiles
            | Self::StructureSvelteDivergenceMissingSuffix
            | Self::StructureSvelteDivergenceSuffixWithoutFiles
            | Self::StructureSvelteDivergenceMissingExpectedOurs
            | Self::StructureSvelteDivergenceMissingExpectedSvelte
            | Self::StructureExpectedOursWithoutSvelteDivergenceSuffix
            | Self::StructureExpectedSvelteWithoutSvelteDivergenceSuffix
            | Self::StructureSvelteDivergenceFilesIdentical
            | Self::StructurePrettierDivergenceMissingSuffix
            | Self::StructurePrettierDivergenceSuffixWithoutFiles
            | Self::StructurePrettierDivergenceHasUnformatted(_)
            | Self::StructurePrettierQuirkWithoutPrettierDivergenceSuffix(_)
            | Self::StructureUnformattedOursWithoutPrettierDivergenceSuffix(_)
            | Self::StructureVariantIdenticalToInput(_)
            | Self::StructureMissingReadme
            | Self::StructureValidationFailed(_) => "Structure",

            Self::ParserExpectedJsonOutdated
            | Self::ParserExpectedOursOutdated
            | Self::ParserExpectedSvelteOutdated
            | Self::ParserError(_) => "Parser",

            Self::FormatterInputNotIdempotent
            | Self::FormatterOutputPrettierOutdated
            | Self::FormatterInputDiffersFromPrettier
            | Self::FormatterError(_) => "Formatter",

            Self::NormalizationPrettierQuirkNotPreserved(_)
            | Self::NormalizationPrettierQuirkNotNormalized(_)
            | Self::NormalizationUnformattedNotNormalized(_)
            | Self::NormalizationUnformattedOursNotNormalized(_)
            | Self::NormalizationUnformattedPrettierMismatch(_) => "Normalization",

            Self::DuplicateUnformattedWithinFixture(_)
            | Self::DuplicatePrettierQuirkWithinFixture(_)
            | Self::RedundantUnformattedMatchesQuirk(_, _) => "Duplicates",
        }
    }
}

/// Successful check for verbose reporting
#[derive(Debug, Clone)]
pub enum ValidationSuccess {
    StructureValid(usize), // number of checks passed
    ParserExpectedJsonMatches,
    ParserExpectedOursMatches,
    ParserExpectedSvelteMatches,
    FormatterInputIdempotent,
    FormatterMatchesPrettier,
    NormalizationVariantsOk(usize), // number of variants checked
    NormalizationSkipped,           // skipped due to formatter failure
}

impl fmt::Display for ValidationSuccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructureValid(n) => write!(f, "{n} structure checks passed"),
            Self::ParserExpectedJsonMatches => write!(f, "expected.json matches Svelte parser"),
            Self::ParserExpectedOursMatches => write!(f, "expected_ours.json matches our parser"),
            Self::ParserExpectedSvelteMatches => {
                write!(f, "expected_svelte.json matches Svelte parser")
            }
            Self::FormatterInputIdempotent => write!(f, "input.svelte is idempotent"),
            Self::FormatterMatchesPrettier => write!(f, "input.svelte matches prettier"),
            Self::NormalizationVariantsOk(n) => write!(f, "{n} variants normalize correctly"),
            Self::NormalizationSkipped => write!(f, "SKIPPED (formatter not idempotent)"),
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
    pub prettier_quirk_count: usize,
    /// Input content for cross-fixture duplicate detection (populated during validation)
    pub input_content: Option<String>,
}

impl FixtureValidation {
    pub fn new(fixture_path: String) -> Self {
        Self {
            fixture_path,
            errors: Vec::new(),
            successes: Vec::new(),
            unformatted_count: 0,
            unformatted_ours_count: 0,
            prettier_quirk_count: 0,
            input_content: None,
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
    fn record(&mut self, fixture_path: &str, content: &str) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
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

    // Store input content for cross-fixture duplicate detection (done after collection)
    result.input_content = Some(input.clone());

    // Get directory info and input type
    let fixture_dir = &fixture.path;
    let dir_name = fixture_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_svelte_divergence_dir = has_svelte_divergence_suffix(dir_name);
    let is_prettier_divergence_dir = has_prettier_divergence_suffix(dir_name);
    let input_type = fixture.input_type();
    let input_ext = input_type.extension();

    // Phases 2-4: Our parser/formatter validation (skip in prettier_only mode)
    if !prettier_only {
        // Phase 2: Our Parser validation - P2 (pure Rust)
        validate_parser_ours(&mut result, fixture, &input);

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
    validate_parser_external(
        &mut result,
        fixture,
        &input,
        is_svelte_divergence_dir,
        input_type,
    )
    .await;

    // F2, F3: Prettier freshness and baseline (Svelte only)
    // TypeScript/CSS fixtures don't use prettier-svelte plugin
    if input_type == InputType::Svelte {
        validate_formatter_prettier(&mut result, fixture, &input, is_prettier_divergence_dir).await;

        // N1, N3: Prettier normalization (Svelte only)
        validate_normalization_prettier(&mut result, fixture, &input, input_ext).await;
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
                diff::print_diff("expected_ours.json diff", &expected, &actual);
            } else {
                result.add_success(ValidationSuccess::ParserExpectedOursMatches);
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
                result.add_error(ValidationError::FormatterInputNotIdempotent);
                diff::print_diff(
                    &format!("{} idempotency diff", fixture.input_file),
                    input,
                    &formatted,
                );
                false
            } else {
                result.add_success(ValidationSuccess::FormatterInputIdempotent);
                true
            }
        }
        Err(e) => {
            result.add_error(ValidationError::FormatterError(e));
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

    // N2: prettier_quirk_*.svelte → input.svelte (our formatter)
    // Note: prettier_quirk files are Svelte-only, always use .svelte
    let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir);
    result.prettier_quirk_count = prettier_quirk_variants.len();
    let mut quirk_contents: HashMap<String, Vec<String>> = HashMap::new();

    for quirk_name in &prettier_quirk_variants {
        let quirk_path = fixture_dir.join(quirk_name);
        let Ok(quirk_content) = read_file(&quirk_path) else {
            continue;
        };

        // Track for duplicate detection
        quirk_contents
            .entry(quirk_content.clone())
            .or_default()
            .push(quirk_name.clone());

        match fixtures::format_with_our_formatter(&quirk_content, "temp.svelte") {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationPrettierQuirkNotNormalized(
                        quirk_name.clone(),
                    ));
                    diff::print_diff(&format!("{quirk_name} normalization"), input, &formatted);
                } else {
                    total_variants += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "{quirk_name}: {e}"
                )));
            }
        }
    }

    // Check for duplicate prettier_quirk files
    for variants in quirk_contents.values() {
        if variants.len() > 1 {
            result.add_error(ValidationError::DuplicatePrettierQuirkWithinFixture(
                variants.clone(),
            ));
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
                    diff::print_diff(&format!("{variant_name} normalization"), input, &formatted);
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

    // Check for redundant unformatted files (identical to prettier_quirk)
    for (unformatted_content, unformatted_files) in &unformatted_contents {
        for (quirk_content, quirk_files) in &quirk_contents {
            if unformatted_content == quirk_content {
                for unformatted_file in unformatted_files {
                    result.add_error(ValidationError::RedundantUnformattedMatchesQuirk(
                        unformatted_file.clone(),
                        quirk_files[0].clone(),
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
                    diff::print_diff(&format!("{variant_name} normalization"), input, &formatted);
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

    if total_variants > 0 {
        result.add_success(ValidationSuccess::NormalizationVariantsOk(total_variants));
    }
}

/// P1, P3: Validate expected.json and expected_svelte.json match external parser
///
/// For Svelte fixtures: uses Svelte's parser
/// For TypeScript fixtures: uses acorn+typescript parser
async fn validate_parser_external(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    _is_svelte_divergence_dir: bool,
    input_type: InputType,
) {
    // CSS fixtures use our own parser (no external canonical source)
    // Skip external parser validation - only our parser validation applies
    if input_type == InputType::Css {
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

    // TypeScript fixtures use acorn+typescript, not Svelte parser
    if input_type == InputType::TypeScript {
        let Some(expected_str) = &expected_content else {
            return; // No expected.json to validate
        };

        match parse_typescript(input).await {
            Ok(ts_ast_str) => {
                let ts_ast_normalized = tsv_cli::json_utils::ensure_trailing_newline(ts_ast_str);

                if *expected_str != ts_ast_normalized {
                    result.add_error(ValidationError::ParserExpectedJsonOutdated);
                    diff::print_diff("expected.json diff", expected_str, &ts_ast_normalized);
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
        Ok(svelte_ast_str) => {
            if expected_svelte_failure {
                result.add_error(ValidationError::ParserExpectedSvelteOutdated);
                return;
            }

            let svelte_ast_normalized =
                tsv_cli::json_utils::ensure_trailing_newline(svelte_ast_str);

            // P1: Check expected.json (only if not in svelte divergence dir)
            if let Some(expected_str) = &expected_content {
                if *expected_str != svelte_ast_normalized {
                    result.add_error(ValidationError::ParserExpectedJsonOutdated);
                    diff::print_diff("expected.json diff", expected_str, &svelte_ast_normalized);
                } else {
                    result.add_success(ValidationSuccess::ParserExpectedJsonMatches);
                }
            }

            // P3: Check expected_svelte.json
            if let Some(expected_svelte_str) = &expected_svelte_content {
                if !expected_svelte_failure && *expected_svelte_str != svelte_ast_normalized {
                    result.add_error(ValidationError::ParserExpectedSvelteOutdated);
                    diff::print_diff(
                        "expected_svelte.json diff",
                        expected_svelte_str,
                        &svelte_ast_normalized,
                    );
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
    is_prettier_divergence_dir: bool,
) {
    let output_prettier_path = fixture.output_prettier_path();
    let output_prettier_filename = fixture.output_prettier_filename();

    let formatted =
        match run_prettier(input, fixture.input_type().prettier_parser()).await {
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
                diff::print_diff(
                    &format!("{output_prettier_filename} diff"),
                    &expected_prettier,
                    &formatted,
                );
            } else {
                result.add_success(ValidationSuccess::FormatterMatchesPrettier);
            }
        }
    } else if !is_prettier_divergence_dir {
        // F3: No output_prettier file but input differs from prettier (not in divergence dir)
        if formatted != *input {
            result.add_error(ValidationError::FormatterInputDiffersFromPrettier);
            diff::print_diff("input vs prettier", input, &formatted);
        } else {
            result.add_success(ValidationSuccess::FormatterMatchesPrettier);
        }
    }
}

/// N1, N3: Validate prettier normalization behavior (Svelte only)
async fn validate_normalization_prettier(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
) {
    let fixture_dir = &fixture.path;

    // N1: prettier(prettier_quirk_*.svelte) == prettier_quirk_*.svelte
    let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir);
    for quirk_name in &prettier_quirk_variants {
        let quirk_path = fixture_dir.join(quirk_name);
        let Ok(quirk_content) = read_file(&quirk_path) else {
            continue;
        };

        match run_prettier(&quirk_content, PrettierParser::Parser("svelte"))
            .await
        {
            Ok(formatted) => {
                if formatted != quirk_content {
                    result.add_error(ValidationError::NormalizationPrettierQuirkNotPreserved(
                        quirk_name.clone(),
                    ));
                    diff::print_diff(
                        &format!("{quirk_name} prettier preservation"),
                        &quirk_content,
                        &formatted,
                    );
                }
            }
            Err(_) => continue,
        }
    }

    // N3: prettier(unformatted_*) == input
    // Skip if prettier_quirks exist (prettier won't normalize due to quirks)
    if !prettier_quirk_variants.is_empty() {
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
                    diff::print_diff(
                        &format!("{variant_name} prettier normalization"),
                        input,
                        &formatted,
                    );
                }
            }
            Err(_) => continue,
        }
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
    pub total_prettier_quirk: usize,
    pub results: Vec<FixtureValidation>,
    pub cross_fixture_duplicates: Vec<Vec<String>>,
}

impl ValidationSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, result: FixtureValidation) {
        self.total_fixtures += 1;
        self.total_unformatted += result.unformatted_count;
        self.total_unformatted_ours += result.unformatted_ours_count;
        self.total_prettier_quirk += result.prettier_quirk_count;

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
                detector.record(&result.fixture_path, content);
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
                    eprintln!("    [{}] {}", error_type_name(error), error);
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
                    eprintln!("{prefix}[{}] {}", error_type_name(error), error);

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
        println!(
            "✓ All {} fixtures validated ({} unformatted_*.svelte, {} unformatted_ours_*.svelte, {} prettier_quirk_*.svelte)",
            summary.total_fixtures,
            summary.total_unformatted,
            summary.total_unformatted_ours,
            summary.total_prettier_quirk
        );
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

/// Get short type name for error (for display)
fn error_type_name(error: &ValidationError) -> &'static str {
    match error {
        ValidationError::StructureMissingInput
        | ValidationError::StructureMissingExpected
        | ValidationError::StructureExpectedJsonWithDivergenceFiles
        | ValidationError::StructureSvelteDivergenceMissingSuffix
        | ValidationError::StructureSvelteDivergenceSuffixWithoutFiles
        | ValidationError::StructureSvelteDivergenceMissingExpectedOurs
        | ValidationError::StructureSvelteDivergenceMissingExpectedSvelte
        | ValidationError::StructureExpectedOursWithoutSvelteDivergenceSuffix
        | ValidationError::StructureExpectedSvelteWithoutSvelteDivergenceSuffix
        | ValidationError::StructureSvelteDivergenceFilesIdentical
        | ValidationError::StructurePrettierDivergenceMissingSuffix
        | ValidationError::StructurePrettierDivergenceSuffixWithoutFiles
        | ValidationError::StructurePrettierDivergenceHasUnformatted(_)
        | ValidationError::StructurePrettierQuirkWithoutPrettierDivergenceSuffix(_)
        | ValidationError::StructureUnformattedOursWithoutPrettierDivergenceSuffix(_)
        | ValidationError::StructureVariantIdenticalToInput(_)
        | ValidationError::StructureMissingReadme
        | ValidationError::StructureValidationFailed(_) => "Structure",
        ValidationError::ParserExpectedJsonOutdated
        | ValidationError::ParserExpectedOursOutdated
        | ValidationError::ParserExpectedSvelteOutdated
        | ValidationError::ParserError(_) => "Parser",
        ValidationError::FormatterInputNotIdempotent
        | ValidationError::FormatterOutputPrettierOutdated
        | ValidationError::FormatterInputDiffersFromPrettier
        | ValidationError::FormatterError(_) => "Formatter",
        ValidationError::NormalizationPrettierQuirkNotPreserved(_)
        | ValidationError::NormalizationPrettierQuirkNotNormalized(_)
        | ValidationError::NormalizationUnformattedPrettierMismatch(_)
        | ValidationError::NormalizationUnformattedNotNormalized(_)
        | ValidationError::NormalizationUnformattedOursNotNormalized(_) => "Normalization",
        ValidationError::DuplicateUnformattedWithinFixture(_)
        | ValidationError::DuplicatePrettierQuirkWithinFixture(_)
        | ValidationError::RedundantUnformattedMatchesQuirk(_, _) => "Duplicate",
    }
}
