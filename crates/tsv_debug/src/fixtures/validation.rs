//! Unified fixture validation with per-fixture error grouping
//!
//! All validation errors for a single fixture are collected together,
//! enabling better DX with grouped error reporting.

use crate::fixtures::{
    self, discover_prettier_quirk_variants, discover_unformatted_ours_variants,
    discover_unformatted_variants, has_prettier_divergence_suffix, has_svelte_divergence_suffix,
    read_file, Fixture,
};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Validation error with self-describing names
///
/// Note: Specific structure variants (StructureMissing*, StructurePrettier*, etc.) are defined
/// but currently unused. Structure validation errors are wrapped in `StructureValidationFailed`
/// which preserves detailed messages from `validate_fixture_structure()`. The specific variants
/// document the intended type structure for future refactoring.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ValidationError {
    // Structure - Missing/Invalid
    StructureMissingInput,
    StructureMissingExpected,
    StructureExpectedJsonWithDivergenceFiles,

    // Structure - Svelte divergence
    StructureSvelteDivergenceMissingSuffix,
    StructureSvelteDivergenceSuffixWithoutFiles,
    StructureSvelteDivergenceMissingExpectedOurs,
    StructureSvelteDivergenceMissingExpectedSvelte,
    StructureExpectedOursWithoutSvelteDivergenceSuffix,
    StructureExpectedSvelteWithoutSvelteDivergenceSuffix,

    // Structure - Prettier divergence
    StructurePrettierDivergenceMissingSuffix,
    StructurePrettierDivergenceSuffixWithoutFiles,
    StructurePrettierDivergenceHasUnformatted(Vec<String>),
    StructurePrettierQuirkWithoutPrettierDivergenceSuffix(String),
    StructureUnformattedOursWithoutPrettierDivergenceSuffix(String),

    // Structure - Content validation
    StructureVariantIdenticalToInput(String),
    StructureMissingReadme,

    /// Generic structure validation failure with detailed message
    /// Used when validate_fixture_structure() returns an error string
    StructureValidationFailed(String),

    // Parser
    ParserExpectedJsonOutdated,
    ParserExpectedOursOutdated,
    ParserExpectedSvelteOutdated,
    ParserError(String),

    // Formatter
    FormatterInputNotIdempotent,
    FormatterOutputPrettierOutdated,
    FormatterInputDiffersFromPrettier,
    FormatterError(String),

    // Normalization
    NormalizationPrettierQuirkNotPreserved(String),
    NormalizationPrettierQuirkNotNormalized(String),
    NormalizationUnformattedPrettierMismatch(String),
    NormalizationUnformattedNotNormalized(String),
    NormalizationUnformattedOursNotNormalized(String),

    // Duplicates (within fixture)
    DuplicateUnformattedWithinFixture(Vec<String>),
    DuplicatePrettierQuirkWithinFixture(Vec<String>),
    RedundantUnformattedMatchesQuirk(String, String),
}

impl ValidationError {
    /// Suggested fix for this error
    pub fn fix_hint(&self) -> &'static str {
        match self {
            Self::StructureMissingInput => "Add input.svelte file",
            Self::StructureMissingExpected => "Run: deno task fixtures_update_parsed",
            Self::StructureExpectedJsonWithDivergenceFiles
            | Self::StructureSvelteDivergenceMissingSuffix
            | Self::StructureSvelteDivergenceSuffixWithoutFiles
            | Self::StructureSvelteDivergenceMissingExpectedOurs
            | Self::StructureSvelteDivergenceMissingExpectedSvelte
            | Self::StructureExpectedOursWithoutSvelteDivergenceSuffix
            | Self::StructureExpectedSvelteWithoutSvelteDivergenceSuffix => {
                "See docs/fixtures.md for _svelte_divergence naming rules"
            }
            Self::StructurePrettierDivergenceMissingSuffix
            | Self::StructurePrettierDivergenceSuffixWithoutFiles
            | Self::StructurePrettierDivergenceHasUnformatted(_)
            | Self::StructurePrettierQuirkWithoutPrettierDivergenceSuffix(_)
            | Self::StructureUnformattedOursWithoutPrettierDivergenceSuffix(_) => {
                "See docs/fixtures.md for _prettier_divergence naming rules"
            }
            Self::StructureVariantIdenticalToInput(_) => {
                "Variant files must differ from input.svelte"
            }
            Self::StructureMissingReadme => "Add README.md explaining the divergence",
            Self::StructureValidationFailed(_) => "See error message for details",
            Self::ParserExpectedJsonOutdated
            | Self::ParserExpectedOursOutdated
            | Self::ParserExpectedSvelteOutdated => "Run: deno task fixtures_update_parsed",
            Self::ParserError(_) => "Fix the parser error",
            Self::FormatterInputNotIdempotent => {
                "Run: cargo run -p tsv_debug compare <fixture>/input.svelte"
            }
            Self::FormatterOutputPrettierOutdated => "Run: deno task fixtures_update_formatted",
            Self::FormatterInputDiffersFromPrettier => {
                "Add output_prettier.svelte or format input.svelte"
            }
            Self::FormatterError(_) => "Fix the formatter error",
            Self::NormalizationPrettierQuirkNotPreserved(_) => {
                "Rename to unformatted_*.svelte (not a quirk)"
            }
            Self::NormalizationPrettierQuirkNotNormalized(_)
            | Self::NormalizationUnformattedNotNormalized(_)
            | Self::NormalizationUnformattedOursNotNormalized(_) => {
                "Fix formatter or update variant"
            }
            Self::NormalizationUnformattedPrettierMismatch(_) => "Check variant content",
            Self::DuplicateUnformattedWithinFixture(_)
            | Self::DuplicatePrettierQuirkWithinFixture(_) => "Remove duplicate files",
            Self::RedundantUnformattedMatchesQuirk(_, _) => "Remove redundant unformatted file",
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructureMissingInput => write!(f, "Missing input.svelte"),
            Self::StructureMissingExpected => write!(f, "Missing expected.json"),
            Self::StructureExpectedJsonWithDivergenceFiles => {
                write!(f, "expected.json cannot coexist with expected_ours.json")
            }
            Self::StructureSvelteDivergenceMissingSuffix => {
                write!(f, "Directory needs _svelte_divergence suffix")
            }
            Self::StructureSvelteDivergenceSuffixWithoutFiles => {
                write!(
                    f,
                    "_svelte_divergence dir missing expected_ours/svelte.json"
                )
            }
            Self::StructureSvelteDivergenceMissingExpectedOurs => {
                write!(f, "Missing expected_ours.json in _svelte_divergence dir")
            }
            Self::StructureSvelteDivergenceMissingExpectedSvelte => {
                write!(f, "Missing expected_svelte.json in _svelte_divergence dir")
            }
            Self::StructureExpectedOursWithoutSvelteDivergenceSuffix => {
                write!(
                    f,
                    "expected_ours.json requires _svelte_divergence suffix"
                )
            }
            Self::StructureExpectedSvelteWithoutSvelteDivergenceSuffix => {
                write!(
                    f,
                    "expected_svelte.json requires _svelte_divergence suffix"
                )
            }
            Self::StructurePrettierDivergenceMissingSuffix => {
                write!(f, "Directory needs _prettier_divergence suffix")
            }
            Self::StructurePrettierDivergenceSuffixWithoutFiles => {
                write!(f, "_prettier_divergence dir missing divergence files")
            }
            Self::StructurePrettierDivergenceHasUnformatted(files) => {
                write!(
                    f,
                    "_prettier_divergence dir has unformatted_*.svelte: {}",
                    files.join(", ")
                )
            }
            Self::StructurePrettierQuirkWithoutPrettierDivergenceSuffix(file) => {
                write!(f, "{} requires _prettier_divergence suffix", file)
            }
            Self::StructureUnformattedOursWithoutPrettierDivergenceSuffix(file) => {
                write!(f, "{} requires _prettier_divergence suffix", file)
            }
            Self::StructureVariantIdenticalToInput(file) => {
                write!(f, "{} is identical to input.svelte", file)
            }
            Self::StructureMissingReadme => write!(f, "README.md required for divergence"),
            Self::StructureValidationFailed(msg) => write!(f, "{}", msg),
            Self::ParserExpectedJsonOutdated => write!(f, "expected.json is outdated"),
            Self::ParserExpectedOursOutdated => write!(f, "expected_ours.json is outdated"),
            Self::ParserExpectedSvelteOutdated => write!(f, "expected_svelte.json is outdated"),
            Self::ParserError(msg) => write!(f, "Parser error: {}", msg),
            Self::FormatterInputNotIdempotent => {
                write!(f, "input.svelte doesn't format to itself")
            }
            Self::FormatterOutputPrettierOutdated => {
                write!(f, "output_prettier.svelte is outdated")
            }
            Self::FormatterInputDiffersFromPrettier => {
                write!(f, "input.svelte differs from prettier output")
            }
            Self::FormatterError(msg) => write!(f, "Formatter error: {}", msg),
            Self::NormalizationPrettierQuirkNotPreserved(file) => {
                write!(f, "{} not preserved by prettier", file)
            }
            Self::NormalizationPrettierQuirkNotNormalized(file) => {
                write!(f, "{} doesn't normalize to input.svelte", file)
            }
            Self::NormalizationUnformattedPrettierMismatch(file) => {
                write!(f, "{} doesn't normalize to input.svelte (prettier)", file)
            }
            Self::NormalizationUnformattedNotNormalized(file) => {
                write!(f, "{} doesn't normalize to input.svelte", file)
            }
            Self::NormalizationUnformattedOursNotNormalized(file) => {
                write!(f, "{} doesn't normalize to input.svelte", file)
            }
            Self::DuplicateUnformattedWithinFixture(files) => {
                write!(f, "Duplicate unformatted files: {}", files.join(", "))
            }
            Self::DuplicatePrettierQuirkWithinFixture(files) => {
                write!(f, "Duplicate prettier_quirk files: {}", files.join(", "))
            }
            Self::RedundantUnformattedMatchesQuirk(file, quirk) => {
                write!(f, "{} is redundant (identical to {})", file, quirk)
            }
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
            Self::StructureValid(n) => write!(f, "{} structure checks passed", n),
            Self::ParserExpectedJsonMatches => write!(f, "expected.json matches Svelte parser"),
            Self::ParserExpectedOursMatches => write!(f, "expected_ours.json matches our parser"),
            Self::ParserExpectedSvelteMatches => {
                write!(f, "expected_svelte.json matches Svelte parser")
            }
            Self::FormatterInputIdempotent => write!(f, "input.svelte is idempotent"),
            Self::FormatterMatchesPrettier => write!(f, "input.svelte matches prettier"),
            Self::NormalizationVariantsOk(n) => write!(f, "{} variants normalize correctly", n),
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

/// Context for cross-fixture validation (duplicate detection)
#[derive(Debug, Default)]
pub struct ValidationContext {
    /// Map from content hash to list of fixture paths with that content
    pub input_hashes: HashMap<u64, Vec<String>>,
}

impl ValidationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_input(&mut self, fixture_path: &str, content: &str) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = hasher.finish();

        self.input_hashes
            .entry(hash)
            .or_default()
            .push(fixture_path.to_string());
    }

    pub fn find_duplicates(&self) -> Vec<Vec<String>> {
        self.input_hashes
            .values()
            .filter(|paths| paths.len() > 1)
            .cloned()
            .collect()
    }
}

/// Validate a single fixture, collecting all errors
pub async fn validate_fixture(
    fixture: &Fixture,
    context: &mut ValidationContext,
) -> FixtureValidation {
    let mut result = FixtureValidation::new(fixture.relative_path.clone());

    // Phase 1: Structure validation (no daemon)
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
                "Failed to read input: {}",
                e
            )));
            return result;
        }
    };

    // Record input hash for cross-fixture duplicate detection
    context.record_input(&fixture.relative_path, &input);

    // Get directory info
    let fixture_dir = &fixture.path;
    let dir_name = fixture_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_svelte_divergence_dir = has_svelte_divergence_suffix(dir_name);
    let is_prettier_divergence_dir = has_prettier_divergence_suffix(dir_name);

    // Phase 2: Our Parser validation - P2 (no daemon)
    validate_parser_ours(&mut result, fixture, &input);

    // Phase 3: Our Formatter validation - F1 (no daemon)
    let format_ok = validate_formatter_idempotent(&mut result, &input);

    // Phase 4: Our Normalization (skip if F1 failed)
    if format_ok {
        validate_normalization_ours(&mut result, fixture, &input);
    } else {
        result.add_success(ValidationSuccess::NormalizationSkipped);
    }

    // Phase 5: Daemon validations (prettier + Svelte parser)
    // P1, P3: Parser freshness
    validate_parser_external(&mut result, fixture, &input, is_svelte_divergence_dir).await;

    // F2, F3: Prettier freshness and baseline
    validate_formatter_prettier(&mut result, fixture, &input, is_prettier_divergence_dir).await;

    // N1, N3: Prettier normalization
    validate_normalization_prettier(&mut result, fixture, &input).await;

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
                "Failed to read expected_ours.json: {}",
                e
            )));
            return;
        }
    };

    match fixtures::parse_with_our_parser_to_string(input, "temp.svelte") {
        Ok(actual) => {
            if actual != expected {
                result.add_error(ValidationError::ParserExpectedOursOutdated);
            } else {
                result.add_success(ValidationSuccess::ParserExpectedOursMatches);
            }
        }
        Err(e) => {
            result.add_error(ValidationError::ParserError(e));
        }
    }
}

/// F1: Validate input.svelte formats to itself
fn validate_formatter_idempotent(result: &mut FixtureValidation, input: &str) -> bool {
    match fixtures::format_with_our_formatter(input, "temp.svelte") {
        Ok(formatted) => {
            if formatted != *input {
                result.add_error(ValidationError::FormatterInputNotIdempotent);
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

/// N2, N4, N5: Validate our formatter normalizes variants to input.svelte
fn validate_normalization_ours(result: &mut FixtureValidation, fixture: &Fixture, input: &str) {
    let fixture_dir = &fixture.path;
    let mut total_variants = 0;

    // N2: prettier_quirk_*.svelte → input.svelte (our formatter)
    let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir);
    result.prettier_quirk_count = prettier_quirk_variants.len();
    let mut quirk_contents: HashMap<String, Vec<String>> = HashMap::new();

    for quirk_name in &prettier_quirk_variants {
        let quirk_path = fixture_dir.join(quirk_name);
        let quirk_content = match read_file(&quirk_path) {
            Ok(s) => s,
            Err(_) => continue,
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
                } else {
                    total_variants += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "{}: {}",
                    quirk_name, e
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

    // N4: unformatted_*.svelte → input.svelte (our formatter)
    let unformatted_variants = discover_unformatted_variants(fixture_dir);
    result.unformatted_count = unformatted_variants.len();
    let mut unformatted_contents: HashMap<String, Vec<String>> = HashMap::new();

    for variant_name in &unformatted_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Track for duplicate detection
        unformatted_contents
            .entry(variant_content.clone())
            .or_default()
            .push(variant_name.clone());

        match fixtures::format_with_our_formatter(&variant_content, "temp.svelte") {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedNotNormalized(
                        variant_name.clone(),
                    ));
                } else {
                    total_variants += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "{}: {}",
                    variant_name, e
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

    // N5: unformatted_ours_*.svelte → input.svelte (our formatter only)
    let unformatted_ours_variants = discover_unformatted_ours_variants(fixture_dir);
    result.unformatted_ours_count = unformatted_ours_variants.len();

    for variant_name in unformatted_ours_variants {
        let variant_path = fixture_dir.join(&variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match fixtures::format_with_our_formatter(&variant_content, "temp.svelte") {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedOursNotNormalized(
                        variant_name,
                    ));
                } else {
                    total_variants += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "{}: {}",
                    variant_name, e
                )));
            }
        }
    }

    if total_variants > 0 {
        result.add_success(ValidationSuccess::NormalizationVariantsOk(total_variants));
    }
}

/// P1, P3: Validate expected.json and expected_svelte.json match Svelte parser
async fn validate_parser_external(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    _is_svelte_divergence_dir: bool,
) {
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

    match fuz_client::parse_svelte(input).await {
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
                } else {
                    result.add_success(ValidationSuccess::ParserExpectedJsonMatches);
                }
            }

            // P3: Check expected_svelte.json
            if let Some(expected_svelte_str) = &expected_svelte_content {
                if !expected_svelte_failure && *expected_svelte_str != svelte_ast_normalized {
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
    is_prettier_divergence_dir: bool,
) {
    let output_prettier_path = fixture.output_prettier_path();

    let formatted = match fuz_client::run_prettier(input, "temp.svelte").await {
        Ok(f) => f,
        Err(e) => {
            result.add_error(ValidationError::FormatterError(format!("Prettier: {}", e)));
            return;
        }
    };

    if output_prettier_path.exists() {
        // F2: Check output_prettier.svelte matches prettier
        if let Ok(expected_prettier) = read_file(&output_prettier_path) {
            if expected_prettier != formatted {
                result.add_error(ValidationError::FormatterOutputPrettierOutdated);
            } else {
                result.add_success(ValidationSuccess::FormatterMatchesPrettier);
            }
        }
    } else if !is_prettier_divergence_dir {
        // F3: No output_prettier.svelte but input differs from prettier (not in divergence dir)
        if formatted != *input {
            result.add_error(ValidationError::FormatterInputDiffersFromPrettier);
        } else {
            result.add_success(ValidationSuccess::FormatterMatchesPrettier);
        }
    }
}

/// N1, N3: Validate prettier normalization behavior
async fn validate_normalization_prettier(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
) {
    let fixture_dir = &fixture.path;

    // N1: prettier(prettier_quirk_*.svelte) == prettier_quirk_*.svelte
    let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir);
    for quirk_name in &prettier_quirk_variants {
        let quirk_path = fixture_dir.join(quirk_name);
        let quirk_content = match read_file(&quirk_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match fuz_client::run_prettier(&quirk_content, "temp.svelte").await {
            Ok(formatted) => {
                if formatted != quirk_content {
                    result.add_error(ValidationError::NormalizationPrettierQuirkNotPreserved(
                        quirk_name.clone(),
                    ));
                }
            }
            Err(_) => continue,
        }
    }

    // N3: prettier(unformatted_*.svelte) == input.svelte
    // Skip if prettier_quirks exist (prettier won't normalize due to quirks)
    if !prettier_quirk_variants.is_empty() {
        return;
    }

    let unformatted_variants = discover_unformatted_variants(fixture_dir);
    for variant_name in &unformatted_variants {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match fuz_client::run_prettier(&variant_content, "temp.svelte").await {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedPrettierMismatch(
                        variant_name.clone(),
                    ));
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
                    println!("    [OK] {}", success);
                }
            } else {
                eprintln!("✗ {}", result.fixture_path);
                for success in &result.successes {
                    eprintln!("    [OK] {}", success);
                }
                for error in &result.errors {
                    eprintln!("    [{}] {}", error_type_name(error), error);
                    eprintln!("           Fix: {}", error.fix_hint());
                }
            }
            println!();
        }
    } else if !failed.is_empty() {
        // Print errors grouped by fixture
        eprintln!();
        for result in &failed {
            eprintln!("✗ {}", result.fixture_path);
            for error in &result.errors {
                eprintln!("    [{}] {}", error_type_name(error), error);
                eprintln!("           Fix: {}", error.fix_hint());
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
                eprintln!("      - {}", path);
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
