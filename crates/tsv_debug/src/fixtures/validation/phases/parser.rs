//! P-phase parser validation plus invalid-syntax checks (P* rules + input_invalid_*).

use crate::deno::parse_by_type_with_goal;
use crate::fixtures::{self, CanonicalParseError, Fixture, FixtureFiles, InputType, read_file};

use super::super::FixtureValidation;
use super::super::errors::{ValidationError, ValidationSuccess};
use super::super::parsed_input::{InputAstPaths, input_ast_paths, parse_input};

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

    match compare_wire(paths, &expected_str) {
        WireMatch::Exact => result.add_success(ValidationSuccess::ParserOursMatchesExpected),
        WireMatch::FieldOrderOnly => result.add_error(ValidationError::ParserOursFieldOrderDiffers),
        WireMatch::Differs => result.add_error(ValidationError::ParserOursDiffersFromExpected),
        WireMatch::Unreadable(e) => result.add_error(ValidationError::ParserError(format!(
            "Failed to parse expected.json: {e}"
        ))),
    }
}

/// How the writer's wire stands against an `expected*.json` the canonical parser produced —
/// the triage P2b and P4 share, so a field-order divergence reads the same at both.
enum WireMatch {
    /// The tabbed serialization is byte-identical to the file.
    Exact,
    /// Semantically equal as a key-order-insensitive `Value`; only the field order differs.
    FieldOrderOnly,
    /// A real AST difference.
    Differs,
    /// The expected file is not readable JSON; carries the reader's message.
    Unreadable(String),
}

fn compare_wire(paths: &InputAstPaths, expected: &str) -> WireMatch {
    if paths.ast_json_tabs == expected {
        return WireMatch::Exact;
    }
    match crate::json::from_str::<serde_json::Value>(expected) {
        Ok(expected_json) if paths.wire_value() == expected_json => WireMatch::FieldOrderOnly,
        Ok(_) => WireMatch::Differs,
        Err(e) => WireMatch::Unreadable(e.to_string()),
    }
}

/// P1, P3: Validate expected.json and expected_svelte.json against the canonical parser
///
/// One path for every input type: the canonical parser is Svelte's for `.svelte`,
/// acorn-typescript at the fixture's goal for `.ts` / `.svelte.ts`, and `parseCss` for
/// `.css`, serialized by [`fixtures::canonical_expected_json`] — the same bytes
/// `fixtures:update:parsed` writes. P1: the canonical parser must accept the input and
/// match `expected.json`. P3: `expected_svelte.json` must hold the canonical AST, or
/// [`fixtures::EXPECTED_SVELTE_ERROR_JSON`] exactly when the canonical parser rejects.
/// A sidecar fault is no verdict on the input, so it is reported once and grades neither.
pub(in crate::fixtures::validation) async fn validate_parser_external(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_type: InputType,
) {
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

    if expected_content.is_none() && expected_svelte_content.is_none() {
        return;
    }

    // Narrowed to the two answers that grade the files — the AST, or the rejection's message;
    // a sidecar fault or an unserializable AST is reported once and grades nothing
    let canonical: Result<String, String> =
        match fixtures::canonical_expected_json(input, input_type, fixture.goal()).await {
            Ok(json) => Ok(json),
            Err(CanonicalParseError::Rejected(message)) => Err(message),
            Err(CanonicalParseError::Sidecar(e)) => {
                result.add_error(ValidationError::CanonicalParserSidecarFailure(
                    fixtures::canonical_sidecar_failure(input_type, &e),
                ));
                return;
            }
            Err(CanonicalParseError::Unserializable(message)) => {
                result.add_error(ValidationError::ParserError(message));
                return;
            }
        };

    // P1: expected.json holds the canonical AST, so a rejection fails it outright
    if let Some(expected_str) = &expected_content {
        match &canonical {
            Ok(json) if expected_str == json => {
                result.add_success(ValidationSuccess::ParserExpectedJsonMatches);
            }
            Ok(_) => result.add_error(ValidationError::ParserExpectedJsonOutdated),
            Err(message) => {
                result.add_error(ValidationError::ParserError(format!(
                    "canonical parser ({}) rejected the input: {message}",
                    input_type.canonical_parser_name()
                )));
            }
        }
    }

    // P3: expected_svelte.json holds the canonical AST, or the error marker exactly
    // when the canonical parser rejects
    if let Some(expected_svelte_str) = &expected_svelte_content {
        let expects_rejection = expected_svelte_str == fixtures::EXPECTED_SVELTE_ERROR_JSON;
        let matches = match &canonical {
            Ok(json) => !expects_rejection && expected_svelte_str == json,
            Err(_) => expects_rejection,
        };
        if matches {
            result.add_success(ValidationSuccess::ParserExpectedSvelteMatches);
        } else {
            result.add_error(ValidationError::ParserExpectedSvelteOutdated);
        }
    }
}

/// One variant parse pin, read off disk.
pub(in crate::fixtures::validation) struct VariantPin {
    /// The pin file (`expected_<stem>.json`).
    pin_name: String,
    /// The variant file the pin names (`<stem><ext>`).
    variant_name: String,
    /// That variant's source.
    variant: String,
    /// The pin's own bytes — the canonical AST, as `fixtures:update:parsed` wrote it.
    expected: String,
}

/// Read every pin and its variant, once for both halves of P4 — the two phases grade the
/// same two files, so the reading and its error path are stated here alone. A pin whose
/// file or variant cannot be read records the error and is left out; it grades nothing.
pub(in crate::fixtures::validation) fn read_variant_pins(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    files: &FixtureFiles,
) -> Vec<VariantPin> {
    let mut read = |name: &str| match read_file(&fixture.path.join(name)) {
        Ok(content) => Some(content),
        Err(e) => {
            result.add_error(ValidationError::FileReadError(e));
            None
        }
    };
    files
        .expected_variant
        .iter()
        .filter_map(|entry| {
            let variant = read(&entry.variant)?;
            let expected = read(&entry.pin)?;
            Some(VariantPin {
                pin_name: entry.pin.clone(),
                variant_name: entry.variant.clone(),
                variant,
                expected,
            })
        })
        .collect()
}

/// P4 (tsv side): our parse of a pinned variant reproduces its `expected_<stem>.json`
/// byte-strict — the P2b claim, made about the variant instead of the input.
///
/// The pin exists for a parse fact no `input.*` can carry: a leading BOM with nothing
/// load-bearing behind it, which the format side strips, so such an input is never its own
/// fixed point (F1). Pure Rust, like P2b;
/// a semantically-equal mismatch is reported as the field-order kind, like P2b's.
pub(in crate::fixtures::validation) fn validate_variant_parse_pins_ours(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input_type: InputType,
    pins: &[VariantPin],
) {
    let mut matched = 0;
    for pin in pins {
        let arena = bumpalo::Bump::new();
        let parsed = match parse_input(&pin.variant, input_type, fixture.goal(), &arena) {
            Ok(parsed) => parsed,
            Err(message) => {
                result.add_error(ValidationError::ParserVariantError {
                    variant: pin.variant_name.clone(),
                    message,
                });
                continue;
            }
        };
        let paths = input_ast_paths(&parsed, &pin.variant);
        match compare_wire(&paths, &pin.expected) {
            WireMatch::Exact => matched += 1,
            WireMatch::FieldOrderOnly => {
                result.add_error(ValidationError::ParserVariantOursFieldOrderDiffers {
                    variant: pin.variant_name.clone(),
                    pin: pin.pin_name.clone(),
                });
            }
            WireMatch::Differs => {
                result.add_error(ValidationError::ParserVariantOursDiffers {
                    variant: pin.variant_name.clone(),
                    pin: pin.pin_name.clone(),
                });
            }
            WireMatch::Unreadable(e) => {
                result.add_error(ValidationError::ParserError(format!(
                    "Failed to parse {}: {e}",
                    pin.pin_name
                )));
            }
        }
    }
    if matched > 0 {
        result.add_success(ValidationSuccess::VariantParsePinsOurs(matched));
    }
}

/// P4 (canonical side): each `expected_<stem>.json` still holds what the canonical
/// parser emits for its variant — the P1 freshness claim, made about the variant. A
/// rejection fails the pin outright (it holds an AST); a sidecar fault is reported once
/// and grades nothing, as everywhere.
pub(in crate::fixtures::validation) async fn validate_variant_parse_pins_canonical(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input_type: InputType,
    pins: &[VariantPin],
) {
    let mut matched = 0;
    for pin in pins {
        match fixtures::canonical_expected_json(&pin.variant, input_type, fixture.goal()).await {
            Ok(json) if json == pin.expected => matched += 1,
            Ok(_) => result.add_error(ValidationError::ParserVariantPinOutdated(
                pin.pin_name.clone(),
            )),
            Err(CanonicalParseError::Rejected(message)) => {
                result.add_error(ValidationError::ParserVariantError {
                    variant: pin.variant_name.clone(),
                    message: format!(
                        "canonical parser ({}) rejected the pinned variant: {message}",
                        input_type.canonical_parser_name()
                    ),
                });
            }
            Err(CanonicalParseError::Sidecar(e)) => {
                result.add_error(ValidationError::CanonicalParserSidecarFailure(format!(
                    "{}: {}",
                    pin.variant_name,
                    fixtures::canonical_sidecar_failure(input_type, &e)
                )));
            }
            Err(CanonicalParseError::Unserializable(message)) => {
                result.add_error(ValidationError::ParserError(message));
            }
        }
    }
    if matched > 0 {
        result.add_success(ValidationSuccess::VariantParsePinsCanonical(matched));
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
/// parser bump that changes the shape surfaces too. Separate from P3 because a
/// rejection here is never the error marker: the canonical AST comes from
/// [`fixtures::canonical_expected_json`], the derivation P3 shares. A sidecar fault
/// is no rejection, so it is reported as a canonical-parser sidecar failure instead.
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

    match fixtures::canonical_expected_json(input, input_type, fixture.goal()).await {
        Ok(actual) if actual == expected => {
            result.add_success(ValidationSuccess::ParserExpectedSvelteMatches);
        }
        Ok(_) => result.add_error(ValidationError::ParserExpectedSvelteOutdated),
        Err(CanonicalParseError::Rejected(_)) => {
            result.add_error(ValidationError::TsvRejectsCanonicalRejects(
                fixture.input_file.clone(),
            ));
        }
        Err(CanonicalParseError::Sidecar(e)) => {
            result.add_error(ValidationError::CanonicalParserSidecarFailure(
                fixtures::canonical_sidecar_failure(input_type, &e),
            ));
        }
        Err(CanonicalParseError::Unserializable(message)) => {
            result.add_error(ValidationError::ParserError(message));
        }
    }
}

/// Validate input_invalid_* files: must fail to parse with both our parser and canonical parser
///
/// For Svelte files: both our parser and Svelte's parser must fail
/// For TypeScript and SvelteTs files: both our parser and acorn-typescript must fail
/// For CSS files: both our parser and Svelte's parseCss must fail
///
/// Only the canonical parser's own rejection counts as failing; a sidecar fault is reported
/// against the file and grades nothing.
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
        let ours_failed =
            parse_input(&variant_content, input_type, fixture.goal(), &arena).is_err();

        // Check canonical parser
        let canonical_failed = match parse_by_type_with_goal(
            &variant_content,
            input_type.parser_type(),
            fixture.goal(),
        )
        .await
        {
            Ok(_) => false,
            Err(e) if fixtures::is_canonical_rejection(&e) => true,
            Err(e) => {
                result.add_error(ValidationError::CanonicalParserSidecarFailure(format!(
                    "{variant_name}: {}",
                    fixtures::canonical_sidecar_failure(input_type, &e)
                )));
                continue;
            }
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
