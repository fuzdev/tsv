//! Validation error and success types with fix hints.

use std::fmt;
use thiserror::Error;

use crate::fixtures::InputType;

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
    #[error("convert_ast_json_string output differs from the Value path (convert_ast_json)")]
    ParserJsonStringPathDiverges,
    #[error("typed-walk parity diverges on {0}")]
    ParserTypedWalkParityDiverges(String),
    #[error("typed-walk parity probe {0} failed to parse: {1}")]
    ParserTypedWalkProbeUnparseable(String, String),
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

    // Prettier non-convergence marker (F5): the claimed behavior no longer holds
    #[error(
        "prettier_nonconvergent.txt is stale: prettier is idempotent on {0} (the divergence is gone)"
    )]
    NonconvergentMarkerButPrettierIdempotent(String),
    #[error(
        "prettier_nonconvergent.txt is stale: prettier converges after one pass on {0} (a fixed point exists)"
    )]
    NonconvergentMarkerButPrettierConverges(String),

    // Prettier-rejects marker (F6): the claimed rejection no longer holds
    #[error(
        "prettier_rejects.txt is stale: prettier accepts {0} now (the rejection is gone). Re-baseline as a normal divergence (output_prettier.*) or retire the marker."
    )]
    RejectsMarkerButPrettierAccepts(String),
    #[error(
        "prettier_rejects.txt is stale: prettier still errors on {input}, but the message no longer contains the pinned substring.\n  pinned:   {expected}\n  actual:   {actual}\nUpdate prettier_rejects.txt if the bug morphed, or re-examine."
    )]
    RejectsMarkerWrongMessage {
        input: String,
        expected: String,
        actual: String,
    },
    #[error("prettier_rejects.txt is empty on {0} — it must hold the expected-error substring")]
    RejectsMarkerEmpty(String),

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
    #[error("{0} not kept stable by our formatter (variant_* requires ours(V) == V)")]
    NormalizationVariantOursNotStable(String),
    #[error("{0} normalizes to input with our formatter (should be prettier_variant_* instead)")]
    NormalizationVariantNormalizesToInput(String),

    // Divergent-variant forms (prettier keeps V; ours rewrites V to a third stable form)
    #[error("{0} not preserved by prettier")]
    NormalizationDivergentVariantNotPreserved(String),
    #[error("{0} normalizes to input with our formatter (should be prettier_variant_* instead)")]
    NormalizationDivergentVariantOursNormalizesToInput(String),
    #[error(
        "{0} kept stable by our formatter (both formatters keep it — should be variant_* instead)"
    )]
    NormalizationDivergentVariantOursDualStable(String),
    #[error(
        "{0}: our formatter's rewrite of this form is not itself stable (ours(ours(V)) != ours(V))"
    )]
    NormalizationDivergentVariantOursNotStable(String),

    // Duplicates (within fixture) - variant
    #[error("Duplicate variant files: {}", .0.join(", "))]
    DuplicateVariantWithinFixture(Vec<String>),
    #[error("Duplicate divergent_variant files: {}", .0.join(", "))]
    DuplicateDivergentVariantWithinFixture(Vec<String>),

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
        "prettier output of {0} matches no documented form (output_prettier / prettier_variant_* / variant_* / divergent_variant_*) — prettier may have drifted, or the target is undocumented"
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

    // IO
    /// A file listed by the directory scan could not be read. Loud rather than
    /// silently skipped: a `continue` here would count the file's checks as passed.
    /// The message comes from `read_file` and already names the path.
    #[error("{0}")]
    FileReadError(String),
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
            Self::ParserJsonStringPathDiverges => {
                "Fix convert_ast_json_string (typed comment attach or typed offset translation) to stay byte-identical to convert_ast_json"
            }
            Self::ParserTypedWalkParityDiverges(_) => {
                "Fix the typed walks — a multibyte probe points at translate_typed.rs (a position-bearing field missing from the manual field enumeration); a template-comment probe points at attach_typed.rs (a comment window or reachability mismatch vs the Value dispatcher)"
            }
            Self::ParserTypedWalkProbeUnparseable(_, _) => {
                "The probe content must parse; investigate why prepending a multibyte comment, appending the template-comment expression tag, or extracting <script> content broke parsing"
            }
            Self::ParserError(_) => "Verify input is valid syntax; if valid, fix the parser",
            Self::ParserErrorInDivergence(_) => {
                "Fix the parser to support this syntax (svelte_divergence fixture)"
            }
            Self::FormatterInputNotIdempotent(input_file) => {
                // Return static str - the dynamic path is shown in the error message itself
                match InputType::from_filepath(input_file) {
                    Some(InputType::Svelte) => {
                        "Debug: cargo run -p tsv_debug compare <fixture>/input.svelte"
                    }
                    Some(InputType::SvelteTs) => {
                        "Debug: cargo run -p tsv_debug compare <fixture>/input.svelte.ts"
                    }
                    Some(InputType::TypeScript) => {
                        "Debug: cargo run -p tsv_debug compare <fixture>/input.ts"
                    }
                    Some(InputType::Css) | None => {
                        "Debug: cargo run -p tsv_debug compare <fixture>/input.css"
                    }
                }
            }
            Self::FormatterOutputPrettierOutdated => {
                "Run: deno task fixtures:update:formatted <pattern>"
            }
            Self::FormatterInputDiffersFromPrettier(input_file) => {
                match InputType::from_filepath(input_file) {
                    Some(InputType::Svelte) => {
                        "Run: cargo run -p tsv_debug compare <fixture>/input.svelte to see difference"
                    }
                    Some(InputType::SvelteTs) => {
                        "Run: cargo run -p tsv_debug compare <fixture>/input.svelte.ts to see difference"
                    }
                    Some(InputType::TypeScript) => {
                        "Run: cargo run -p tsv_debug compare <fixture>/input.ts to see difference"
                    }
                    Some(InputType::Css) | None => {
                        "Run: cargo run -p tsv_debug compare <fixture>/input.css to see difference"
                    }
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
            Self::NonconvergentMarkerButPrettierIdempotent(_) => {
                "Delete prettier_nonconvergent.txt and re-evaluate the _prettier_divergence designation (prettier formats input stably now)"
            }
            Self::NonconvergentMarkerButPrettierConverges(_) => {
                "Delete prettier_nonconvergent.txt and document the divergence normally (output_prettier.* / audit_signature.txt): deno task fixtures:update:formatted <pattern>"
            }
            Self::RejectsMarkerButPrettierAccepts(_) => {
                "Delete prettier_rejects.txt and document the divergence normally (output_prettier.*): deno task fixtures:update:formatted <pattern>"
            }
            Self::RejectsMarkerWrongMessage { .. } => {
                "Update prettier_rejects.txt with the new error substring (the upstream bug morphed), or re-examine whether the fixture still belongs"
            }
            Self::RejectsMarkerEmpty(_) => {
                "Write the expected prettier-error substring (position-stripped) into prettier_rejects.txt"
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
            Self::NormalizationVariantOursNotStable(_) => {
                "Our formatter doesn't keep this file verbatim (variant_* must be dual-stable). If ours rewrites it to a third stable form, use divergent_variant_* instead; if ours normalizes it to input, use prettier_variant_*"
            }
            Self::NormalizationVariantNormalizesToInput(_) => {
                "Our formatter normalizes this to input - rename to prettier_variant_* instead"
            }
            Self::NormalizationDivergentVariantNotPreserved(_) => {
                "Prettier doesn't preserve this file - divergent_variant_* requires prettier(V) == V"
            }
            Self::NormalizationDivergentVariantOursNormalizesToInput(_) => {
                "Our formatter normalizes this to input - use prettier_variant_* instead of divergent_variant_*"
            }
            Self::NormalizationDivergentVariantOursDualStable(_) => {
                "Both formatters keep this stable - use variant_* instead of divergent_variant_*"
            }
            Self::NormalizationDivergentVariantOursNotStable(_) => {
                "Our formatter's rewrite of this form must itself be idempotent - investigate why ours(ours(V)) != ours(V)"
            }
            Self::DuplicateVariantWithinFixture(_)
            | Self::DuplicateDivergentVariantWithinFixture(_) => {
                "Remove duplicate files (identical content)"
            }
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
                "Document prettier's output: add a variant_* / prettier_variant_* / divergent_variant_* (or prettier_intermediate*_*) sibling matching it, or update the existing one if prettier changed"
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
            Self::FileReadError(_) => {
                "Check filesystem permissions/encoding — the directory scan listed this file but it could not be read"
            }
        }
    }

    /// Get error category for grouping
    pub fn category(&self) -> &'static str {
        match self {
            Self::StructureValidationFailed(_) => "Structure",

            Self::ParserOursDiffersFromExpected
            | Self::ParserJsonStringPathDiverges
            | Self::ParserTypedWalkParityDiverges(_)
            | Self::ParserTypedWalkProbeUnparseable(_, _)
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
            | Self::FormatterErrorInDivergence(_)
            | Self::NonconvergentMarkerButPrettierIdempotent(_)
            | Self::NonconvergentMarkerButPrettierConverges(_)
            | Self::RejectsMarkerButPrettierAccepts(_)
            | Self::RejectsMarkerWrongMessage { .. }
            | Self::RejectsMarkerEmpty(_) => "Formatter",

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
            | Self::NormalizationVariantOursNotStable(_)
            | Self::NormalizationVariantNormalizesToInput(_)
            | Self::NormalizationDivergentVariantNotPreserved(_)
            | Self::NormalizationDivergentVariantOursNormalizesToInput(_)
            | Self::NormalizationDivergentVariantOursDualStable(_)
            | Self::NormalizationDivergentVariantOursNotStable(_)
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
            | Self::DuplicateDivergentVariantWithinFixture(_)
            | Self::RedundantUnformattedMatchesPrettierVariant(_, _)
            | Self::RedundantPrettierVariantMatchesOutputPrettier(_) => "Duplicates",

            Self::InvalidSyntaxParsedByOurs(_)
            | Self::InvalidSyntaxParsedBySvelte(_)
            | Self::InvalidSyntaxParsedByAcorn(_)
            | Self::InvalidSyntaxParsedByOurCss(_)
            | Self::InvalidSyntaxParsedByParseCss(_) => "InvalidSyntax",

            Self::UnknownFile(_) | Self::FileReadError(_) => "Structure",
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
    ParserJsonStringPathMatches,
    ParserTypedWalkParityOk(usize), // number of parity probes passed
    ParserExpectedSvelteMatches,
    FormatterInputIdempotent,
    FormatterMatchesPrettier,
    NormalizationVariantsOk(usize), // number of variants checked
    VariantVariantsOk(usize),       // number of variant_* checked
    DivergentVariantOursOk(usize),  // number of divergent_variant_* passing the ours-side checks
    NormalizationSkipped,           // skipped due to formatter failure
    InvalidSyntaxVariantsOk(usize), // number of invalid syntax files validated
    // Prettier-side normalization rules: one counter per rule so summaries can
    // distinguish "validated n files" from "had nothing to validate"
    PrettierVariantsStable(usize),                 // N1
    VariantsStable(usize),                         // N9a
    DivergentVariantStable(usize),                 // N11a
    UnformattedPrettierNormalized(usize),          // N3
    UnformattedOursDivergent(usize),               // N6
    PrettierIntermediatesConverge(usize),          // N7
    PrettierIntermediatesToVariantConverge(usize), // N7b
    UnformattedPrettierToOutput(usize),            // N8
    PrettierOutputsPinned(usize),                  // N10
    PrettierNonconvergenceVerified,                // F5
    PrettierRejectionVerified,                     // F6
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
            Self::ParserJsonStringPathMatches => {
                write!(f, "convert_ast_json_string matches the Value path")
            }
            Self::ParserTypedWalkParityOk(n) => {
                write!(f, "{n} typed-walk parity probes passed")
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
            Self::DivergentVariantOursOk(n) => {
                write!(f, "{n} divergent_variant_* variants validated (ours side)")
            }
            Self::NormalizationSkipped => write!(f, "SKIPPED (formatter not idempotent)"),
            Self::InvalidSyntaxVariantsOk(n) => {
                write!(f, "{n} invalid syntax files correctly rejected")
            }
            Self::PrettierVariantsStable(n) => {
                write!(f, "{n} prettier_variant_* preserved by prettier (N1)")
            }
            Self::VariantsStable(n) => write!(f, "{n} variant_* preserved by prettier (N9a)"),
            Self::DivergentVariantStable(n) => {
                write!(f, "{n} divergent_variant_* preserved by prettier (N11a)")
            }
            Self::UnformattedPrettierNormalized(n) => {
                write!(f, "{n} unformatted_* normalized to input by prettier (N3)")
            }
            Self::UnformattedOursDivergent(n) => {
                write!(
                    f,
                    "{n} unformatted_ours_* confirmed prettier-divergent (N6)"
                )
            }
            Self::PrettierIntermediatesConverge(n) => {
                write!(f, "{n} prettier_intermediate_* converge to input (N7)")
            }
            Self::PrettierIntermediatesToVariantConverge(n) => {
                write!(
                    f,
                    "{n} prettier_intermediate_to_variant_* converge to a variant (N7b)"
                )
            }
            Self::UnformattedPrettierToOutput(n) => {
                write!(
                    f,
                    "{n} unformatted_prettier_* normalize to output_prettier (N8)"
                )
            }
            Self::PrettierOutputsPinned(n) => {
                write!(
                    f,
                    "{n} unclaimed prettier outputs match documented forms (N10)"
                )
            }
            Self::PrettierNonconvergenceVerified => {
                write!(f, "prettier non-convergence verified live (F5)")
            }
            Self::PrettierRejectionVerified => {
                write!(f, "prettier rejection verified live (F6)")
            }
        }
    }
}
