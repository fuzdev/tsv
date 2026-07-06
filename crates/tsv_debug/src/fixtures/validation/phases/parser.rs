//! P-phase parser validation plus invalid-syntax checks (P* rules + input_invalid_*).

use crate::deno::{parse_css, parse_svelte, parse_typescript_with_goal};
use crate::fixtures::{self, Fixture, FixtureFiles, InputType, read_file};
use tsv_cli::json_utils::to_json_with_tabs;

use super::super::FixtureValidation;
use super::super::errors::{ValidationError, ValidationSuccess};
use super::super::parsed_input::InputAstPaths;

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
/// AND our parser. Byte-strict: compares the tabbed serialization against the file
/// content exactly like P1/P2/P3, so wire *field-order* divergences fail too
/// (`preserve_order` keeps real key order on both sides, and both sides are
/// `to_json_with_tabs` output, so number/escape formatting is already normalized).
/// A mismatch that is semantically equal as `serde_json::Value` (key-order-insensitive)
/// is reported as a field-order divergence to make triage self-identifying.
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

    if paths.ast_json_tabs == expected_str {
        result.add_success(ValidationSuccess::ParserOursMatchesExpected);
        return;
    }

    let expected_json: serde_json::Value = match serde_json::from_str(&expected_str) {
        Ok(v) => v,
        Err(e) => {
            result.add_error(ValidationError::ParserError(format!(
                "Failed to parse expected.json: {e}"
            )));
            return;
        }
    };

    if paths.ast_json == expected_json {
        result.add_error(ValidationError::ParserOursFieldOrderDiffers);
    } else {
        result.add_error(ValidationError::ParserOursDiffersFromExpected);
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

        match parse_typescript_with_goal(input, fixture.goal()).await {
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

/// F7 (tsv side): live-verify a `tsv_rejects.txt` claim — tsv must REJECT the
/// input with an error message containing the marker's trimmed substring.
///
/// The marker asserts tsv over-rejects an input the canonical parser accepts, so
/// tsv produces no AST: the tsv-side parser/formatter phases (P2/P2b, F1, the
/// ours-side normalization) are inexpressible and replaced by this check.
/// Catches the over-rejection being fixed (tsv accepts now →
/// `TsvRejectsMarkerButTsvAccepts`) and the rejection moving to a different
/// message (→ `TsvRejectsMarkerWrongMessage`), each with a remediation hint.
/// Pure Rust — the tsv parser runs in-process, no sidecar.
pub(in crate::fixtures::validation) fn validate_tsv_rejects(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_type: InputType,
) {
    let expected = match read_file(&fixture.tsv_rejects_path()) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            result.add_error(ValidationError::ParserError(format!(
                "reading tsv_rejects.txt for {}: {e}",
                fixture.input_file
            )));
            return;
        }
    };
    if expected.is_empty() {
        result.add_error(ValidationError::TsvRejectsMarkerEmpty(
            fixture.input_file.clone(),
        ));
        return;
    }

    let arena = bumpalo::Bump::new();
    let parse_result: Result<(), String> = match input_type {
        InputType::Svelte => tsv_svelte::parse(input, &arena)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        InputType::SvelteTs | InputType::TypeScript => {
            tsv_ts::parse_with_goal(input, fixture.goal(), &arena)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        InputType::Css => tsv_css::parse(input, &arena)
            .map(|_| ())
            .map_err(|e| e.to_string()),
    };

    match parse_result {
        Ok(()) => {
            result.add_error(ValidationError::TsvRejectsMarkerButTsvAccepts(
                fixture.input_file.clone(),
            ));
        }
        Err(actual) => {
            if actual.contains(&expected) {
                result.add_success(ValidationSuccess::TsvRejectionVerified);
            } else {
                result.add_error(ValidationError::TsvRejectsMarkerWrongMessage {
                    input: fixture.input_file.clone(),
                    expected,
                    actual,
                });
            }
        }
    }
}

/// F7 (canonical side): the canonical parser must still ACCEPT a `tsv_rejects.txt`
/// input, and its serialized AST must equal `expected_svelte.json`.
///
/// This is the self-heal that the retired Rust-test pins lacked: if the canonical
/// parser starts rejecting too, the divergence is dead (both parsers agree now)
/// and this fails with `TsvRejectsCanonicalRejects` — convert the fixture to
/// `input_invalid_*`. Otherwise the canonical AST is pinned byte-strict against
/// `expected_svelte.json` (refreshed by `fixtures:update:parsed`), so a canonical
/// parser bump that changes the shape surfaces too. Dispatches on input type
/// (`.svelte` → Svelte, `.ts`/`.svelte.ts` → acorn-typescript, `.css` →
/// parseCss), always comparing to `expected_svelte.json` (the canonical AST).
pub(in crate::fixtures::validation) async fn validate_tsv_rejects_canonical(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_type: InputType,
) {
    let expected = match read_file(&fixture.expected_svelte_path()) {
        Ok(c) => c,
        Err(e) => {
            result.add_error(ValidationError::FileReadError(e));
            return;
        }
    };

    // Parse with the canonical parser for this input type, then serialize with the
    // same tabbed format expected_svelte.json stores (matches fixtures_update_parsed).
    let canonical: Result<String, String> = match input_type {
        InputType::Svelte => parse_svelte(input).await.map_err(|e| e.to_string()),
        InputType::SvelteTs | InputType::TypeScript => {
            parse_typescript_with_goal(input, fixture.goal())
                .await
                .map_err(|e| e.to_string())
        }
        InputType::Css => parse_css(input).await.map_err(|e| e.to_string()),
    }
    .and_then(|ast| {
        to_json_with_tabs(&ast)
            .map(|json| format!("{json}\n"))
            .map_err(|e| format!("Failed to serialize canonical AST: {e}"))
    });

    match canonical {
        Ok(actual) => {
            if actual == expected {
                result.add_success(ValidationSuccess::ParserExpectedSvelteMatches);
            } else {
                result.add_error(ValidationError::ParserExpectedSvelteOutdated);
            }
        }
        // A serialization failure is impossible in practice (the canonical AST is
        // already JSON), so a hard error here means the canonical parser rejected —
        // the divergence is dead.
        Err(_) => {
            result.add_error(ValidationError::TsvRejectsCanonicalRejects(
                fixture.input_file.clone(),
            ));
        }
    }
}

/// Validate input_invalid_* files: must fail to parse with both our parser and canonical parser
///
/// For Svelte files: both our parser and Svelte's parser must fail
/// For TypeScript and SvelteTs files: both our parser and acorn-typescript must fail
/// For CSS files: both our parser and Svelte's parseCss must fail
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
                tsv_ts::parse_with_goal(&variant_content, fixture.goal(), &arena).is_err()
            }
            InputType::Css => tsv_css::parse(&variant_content, &arena).is_err(),
        };

        // Check canonical parser
        let canonical_failed = match input_type {
            InputType::Svelte => parse_svelte(&variant_content).await.is_err(),
            InputType::SvelteTs | InputType::TypeScript => {
                parse_typescript_with_goal(&variant_content, fixture.goal())
                    .await
                    .is_err()
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
