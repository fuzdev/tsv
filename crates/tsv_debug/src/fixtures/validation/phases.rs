//! Per-phase validation functions (P* parser, F* formatter, N* normalization).
//!
//! Each phase appends errors/successes to the shared `FixtureValidation`;
//! `validate_fixture` in mod.rs orchestrates the sequence.

use std::collections::HashMap;

use crate::deno::{PrettierParser, parse_css, parse_svelte, parse_typescript, run_prettier};
use crate::diff;
use crate::fixtures::{
    self, AuditSignature, Fixture, InputType, discover_invalid_variants,
    discover_prettier_intermediate_to_variant_variants, discover_prettier_intermediate_variants,
    discover_prettier_variant_variants, discover_unformatted_ours_variants,
    discover_unformatted_prettier_variants, discover_unformatted_variants,
    discover_variant_variants, read_file,
};
use tsv_cli::json_utils::to_json_with_tabs;

use super::errors::{AuditSignatureStaleness, ValidationError, ValidationSuccess};
use super::{FixtureValidation, UndocumentedPrettierOutput};

/// P2: Validate expected_ours.json matches our parser output
pub(super) fn validate_parser_ours(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    paths: &fixtures::InputAstPaths,
) {
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

    if paths.ast_json_tabs == expected {
        result.add_success(ValidationSuccess::ParserExpectedOursMatches);
    } else {
        result.add_error(ValidationError::ParserExpectedOursOutdated);
    }
}

/// Validate our parser output matches expected.json (non-divergence fixtures only)
///
/// For non-divergence fixtures, expected.json should match both the canonical parser
/// AND our parser. Uses semantic (serde_json::Value) comparison to ignore field ordering
/// differences between our parser and the canonical parser.
pub(super) fn validate_parser_ours_matches_expected(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    paths: &fixtures::InputAstPaths,
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

    // Semantic (Value) comparison — ignores key-order differences between
    // our parser and the canonical parser
    if paths.ast_json == expected_json {
        result.add_success(ValidationSuccess::ParserOursMatchesExpected);
    } else {
        result.add_error(ValidationError::ParserOursDiffersFromExpected);
    }
}

/// Validate typed-walk parity on synthesized and extracted probes
///
/// The fixture's own content only exercises the typed offset-translation walk
/// when it's multibyte standalone TS — a handful of files. These probes give
/// every fixture's AST shapes typed-walk parity coverage: a synthesized
/// multibyte variant for `.ts`/`.svelte.ts` inputs, and the extracted
/// `<script>` contents (as-is when multibyte, plus a synthesized variant) for
/// `.svelte` inputs. See `fixtures::typed_walk_parity_probes`.
pub(super) fn validate_typed_walk_parity(
    result: &mut FixtureValidation,
    input: &str,
    parsed: &fixtures::ParsedInput,
) {
    let parity = fixtures::typed_walk_parity_probes(input, parsed);
    for (probe, failure) in parity.failures {
        match failure {
            fixtures::TypedWalkParityFailure::Diverged => {
                result.add_error(ValidationError::ParserTypedWalkParityDiverges(probe));
            }
            fixtures::TypedWalkParityFailure::Parse(e) => {
                result.add_error(ValidationError::ParserTypedWalkProbeUnparseable(probe, e));
            }
        }
    }
    if parity.checked > 0 {
        result.add_success(ValidationSuccess::ParserTypedWalkParityOk(parity.checked));
    }
}

/// F1: Validate input file formats to itself
pub(super) fn validate_formatter_idempotent(
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
pub(super) fn validate_normalization_ours(
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
pub(super) async fn validate_parser_external(
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
pub(super) async fn validate_formatter_prettier(
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
pub(super) async fn validate_normalization_prettier(
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
pub(super) async fn validate_invalid_syntax(
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
