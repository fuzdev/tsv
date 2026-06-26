//! P-phase parser validation plus invalid-syntax checks (P* rules + input_invalid_*).

use crate::deno::{parse_css, parse_svelte, parse_typescript};
use crate::fixtures::{self, Fixture, FixtureFiles, InputType, read_file};
use tsv_cli::json_utils::to_json_with_tabs;

use super::super::FixtureValidation;
use super::super::errors::{ValidationError, ValidationSuccess};
use super::super::parsed_input::{InputAstPaths, ParsedInput, TypedWalkParityFailure};

/// P2: Validate expected_ours.json matches our parser output
pub(in crate::fixtures::validation) fn validate_parser_ours(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    paths: &InputAstPaths,
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
pub(in crate::fixtures::validation) fn validate_parser_ours_matches_expected(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    paths: &InputAstPaths,
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
/// `.svelte` inputs. See `typed_walk_parity_probes` in parsed_input.rs.
pub(in crate::fixtures::validation) fn validate_typed_walk_parity(
    result: &mut FixtureValidation,
    input: &str,
    parsed: &ParsedInput<'_>,
) {
    let parity = super::super::parsed_input::typed_walk_parity_probes(input, parsed);
    for (probe, failure) in parity.failures {
        match failure {
            TypedWalkParityFailure::Diverged => {
                result.add_error(ValidationError::ParserTypedWalkParityDiverges(probe));
            }
            TypedWalkParityFailure::Parse(e) => {
                result.add_error(ValidationError::ParserTypedWalkProbeUnparseable(probe, e));
            }
        }
    }
    if parity.checked > 0 {
        result.add_success(ValidationSuccess::ParserTypedWalkParityOk(parity.checked));
    }
}

/// P1, P3: Validate expected.json and expected_svelte.json match external parser
///
/// For Svelte fixtures: uses Svelte's parser
/// For TypeScript and SvelteTs fixtures: uses acorn+typescript parser
pub(in crate::fixtures::validation) async fn validate_parser_external(
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
        let expected_content = match read_file(&expected_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                return;
            }
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

    // Treat an existing-but-unreadable expected file as a loud error, not as
    // absent — `None` here means "nothing to validate", which would silently
    // skip the parser-freshness checks.
    let expected_content = if expected_path.exists() {
        match read_file(&expected_path) {
            Ok(c) => Some(c),
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                return;
            }
        }
    } else {
        None
    };

    let expected_svelte_content = if expected_svelte_path.exists() {
        match read_file(&expected_svelte_path) {
            Ok(c) => Some(c),
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                return;
            }
        }
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

/// Validate input_invalid_* files: must fail to parse with both our parser and canonical parser
///
/// For Svelte files: both our parser and Svelte's parser must fail
/// For TypeScript and SvelteTs files: both our parser and acorn-typescript must fail
/// For CSS files: our parser must fail (no canonical source)
pub(in crate::fixtures::validation) async fn validate_invalid_syntax(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input_type: InputType,
    files: &FixtureFiles,
) {
    let fixture_dir = &fixture.path;

    if files.input_invalid.is_empty() {
        return;
    }

    let mut valid_count = 0;

    for variant_name in &files.input_invalid {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        // Check our parser
        let arena = bumpalo::Bump::new();
        let ours_failed = match input_type {
            InputType::Svelte => tsv_svelte::parse(&variant_content, &arena).is_err(),
            InputType::SvelteTs | InputType::TypeScript => {
                tsv_ts::parse(&variant_content, &arena).is_err()
            }
            InputType::Css => tsv_css::parse(&variant_content, &arena).is_err(),
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
