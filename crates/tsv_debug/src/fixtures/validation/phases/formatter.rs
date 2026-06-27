//! F-phase formatter validation (F* rules: idempotency, prettier freshness, audit signatures).

use crate::deno::run_prettier;
use crate::diff;
use crate::fixtures::{self, AuditSignature, Fixture, read_file};

use super::super::FixtureValidation;
use super::super::errors::{AuditSignatureStaleness, ValidationError, ValidationSuccess};

/// F1: Validate input file formats to itself
pub(in crate::fixtures::validation) fn validate_formatter_idempotent(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
) -> bool {
    match fixtures::format_with_our_formatter_with_goal(input, &fixture.input_file, fixture.goal())
    {
        Ok(formatted) => {
            if formatted != *input {
                result.add_error(ValidationError::FormatterInputNotIdempotent(
                    fixture.input_file.clone(),
                ));
                result.add_diff(
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

/// F5: Live-verify a `prettier_nonconvergent.txt` claim.
///
/// The marker asserts prettier has NO fixed point on this input — each pass keeps
/// changing the output forever — so F2/F3/F4 and the prettier-side N rules are
/// inexpressible (there is no canonical prettier output to record or pin). Instead
/// of trusting the marker, verify the claim still holds:
/// - `prettier(input) != input` (otherwise prettier is idempotent — divergence gone)
/// - `prettier^2(input) != prettier(input)` (otherwise a fixed point exists one pass
///   in — document it normally via `output_prettier.*`)
///
/// Two passes are a proxy for "never converges": a true proof is impossible, but
/// every convergent prettier chain observed in this repo bottoms out within
/// `MAX_CHAIN_DEPTH` passes, and the known non-convergence bugs grow output on
/// every single pass. If prettier's behavior shifts, this fails loudly with a
/// remediation hint.
pub(in crate::fixtures::validation) async fn validate_prettier_nonconvergent(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
) {
    let parser = fixture.input_type().prettier_parser();

    let pass1 = match run_prettier(input, parser).await {
        Ok(f) => f,
        Err(e) => {
            result.add_error(ValidationError::FormatterError(format!(
                "Prettier on {} (F5 pass 1): {e}",
                fixture.input_file
            )));
            return;
        }
    };
    if pass1 == *input {
        result.add_error(ValidationError::NonconvergentMarkerButPrettierIdempotent(
            fixture.input_file.clone(),
        ));
        return;
    }

    let pass2 = match run_prettier(&pass1, parser).await {
        Ok(f) => f,
        Err(e) => {
            result.add_error(ValidationError::FormatterError(format!(
                "Prettier on {} (F5 pass 2): {e}",
                fixture.input_file
            )));
            return;
        }
    };
    if pass2 == pass1 {
        result.add_error(ValidationError::NonconvergentMarkerButPrettierConverges(
            fixture.input_file.clone(),
        ));
        return;
    }

    result.add_success(ValidationSuccess::PrettierNonconvergenceVerified);
}

/// F6: Live-verify a `prettier_rejects.txt` claim.
///
/// The marker asserts prettier THROWS on this input (a parse rejection or a
/// printer crash), so there is no prettier output to record or pin — F2/F3/F4
/// and the prettier-side N rules are inexpressible. Instead of trusting the
/// marker, verify the claim still holds: `prettier(input)` must return an error
/// whose message contains the marker's recorded substring (the position-stripped
/// error text). This catches both the bug being fixed upstream (prettier accepts
/// → `RejectsMarkerButPrettierAccepts`) and the error morphing into a different
/// message (→ `RejectsMarkerWrongMessage`), each with a remediation hint.
pub(in crate::fixtures::validation) async fn validate_prettier_rejects(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
) {
    let expected = match read_file(&fixture.prettier_rejects_path()) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            result.add_error(ValidationError::FormatterError(format!(
                "reading prettier_rejects.txt for {}: {e}",
                fixture.input_file
            )));
            return;
        }
    };
    if expected.is_empty() {
        result.add_error(ValidationError::RejectsMarkerEmpty(
            fixture.input_file.clone(),
        ));
        return;
    }

    let parser = fixture.input_type().prettier_parser();
    match run_prettier(input, parser).await {
        Ok(_) => {
            result.add_error(ValidationError::RejectsMarkerButPrettierAccepts(
                fixture.input_file.clone(),
            ));
        }
        Err(e) => {
            let actual = e.to_string();
            if actual.contains(&expected) {
                result.add_success(ValidationSuccess::PrettierRejectionVerified);
            } else {
                result.add_error(ValidationError::RejectsMarkerWrongMessage {
                    input: fixture.input_file.clone(),
                    expected,
                    actual,
                });
            }
        }
    }
}

/// F2, F3: Validate formatter output matches prettier
pub(in crate::fixtures::validation) async fn validate_formatter_prettier(
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
        match read_file(&output_prettier_path) {
            Ok(expected_prettier) => {
                if expected_prettier != formatted {
                    result.add_error(ValidationError::FormatterOutputPrettierOutdated);
                    result.add_diff(
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
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
            }
        }
    } else {
        // F3: No output_prettier file - prettier(input) must equal input
        // This applies to ALL directories (including _prettier_divergence)
        if formatted != *input {
            result.add_error(ValidationError::FormatterInputDiffersFromPrettier(
                fixture.input_file.clone(),
            ));
            result.add_diff(
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
                result.add_diff(
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
