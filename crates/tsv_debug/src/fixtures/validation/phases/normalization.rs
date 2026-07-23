//! N-phase normalization validation (N* rules: variant normalization, prettier intermediates).

use std::collections::HashMap;

use crate::deno::run_prettier;
use crate::diff;
use crate::fixtures::{self, Fixture, FixtureFiles, read_file};

use super::super::errors::{ValidationError, ValidationSuccess};
use super::super::{FixtureValidation, UndocumentedPrettierOutput};

/// Find the first `prettier_variant_*` file whose content equals `content`.
///
/// A `prettier_variant_*` already asserts ours → input (N2) AND pins prettier's
/// exact output (N1, prettier == self), so any `unformatted_*` / `unformatted_ours_*`
/// with identical content is redundant — it adds no coverage. Used by both the N4
/// and N5 redundancy checks.
fn matching_prettier_variant<'a>(
    content: &str,
    pv_contents: &'a HashMap<String, Vec<String>>,
) -> Option<&'a String> {
    pv_contents
        .iter()
        .find(|(pv_content, _)| pv_content.as_str() == content)
        .map(|(_, pv_files)| &pv_files[0])
}

/// Report a `Duplicate*WithinFixture` error for any content shared by more than one
/// variant file. `contents` maps file content → the names that produced it (each
/// variant loop builds one for its kind); `dup_error` is that kind's duplicate variant.
fn report_duplicate_variants(
    result: &mut FixtureValidation,
    contents: &HashMap<String, Vec<String>>,
    dup_error: fn(Vec<String>) -> ValidationError,
) {
    for names in contents.values() {
        if names.len() > 1 {
            result.add_error(dup_error(names.clone()));
        }
    }
}

/// N2, N4, N5, N9b, N9c, N11b–d: Validate our formatter's variant handling
/// (normalization to input, `variant_*` dual-stability, and `divergent_variant_*`
/// rewrite-to-a-third-form), with duplicate and redundancy checks across the
/// variant kinds
pub(in crate::fixtures::validation) fn validate_normalization_ours(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    files: &FixtureFiles,
) {
    let fixture_dir = &fixture.path;
    let mut total_variants = 0;

    // N2: prettier_variant_* → input file (our formatter)
    let mut pv_contents: HashMap<String, Vec<String>> = HashMap::new();

    for pv_name in &files.prettier_variant {
        let pv_path = fixture_dir.join(pv_name);
        let pv_content = match read_file(&pv_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        // Track for duplicate detection
        pv_contents
            .entry(pv_content.clone())
            .or_default()
            .push(pv_name.clone());

        match fixtures::format_with_our_formatter_with_goal(&pv_content, pv_name, fixture.goal()) {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationPrettierVariantNotNormalized(
                        pv_name.clone(),
                    ));
                    result.add_diff(
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
    report_duplicate_variants(
        result,
        &pv_contents,
        ValidationError::DuplicatePrettierVariantWithinFixture,
    );

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
    let mut unformatted_contents: HashMap<String, Vec<String>> = HashMap::new();

    for variant_name in &files.unformatted {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        // Track for duplicate detection
        unformatted_contents
            .entry(variant_content.clone())
            .or_default()
            .push(variant_name.clone());

        match fixtures::format_with_our_formatter_with_goal(
            &variant_content,
            &fixture.input_file,
            fixture.goal(),
        ) {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedNotNormalized(
                        variant_name.clone(),
                    ));
                    result.add_diff(
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
    report_duplicate_variants(
        result,
        &unformatted_contents,
        ValidationError::DuplicateUnformattedWithinFixture,
    );

    // Check for redundant unformatted files (identical to prettier_variant)
    for (unformatted_content, unformatted_files) in &unformatted_contents {
        if let Some(pv_file) = matching_prettier_variant(unformatted_content, &pv_contents) {
            for unformatted_file in unformatted_files {
                result.add_error(ValidationError::RedundantUnformattedMatchesPrettierVariant(
                    unformatted_file.clone(),
                    pv_file.clone(),
                ));
            }
        }
    }

    // N5: unformatted_ours_* → input file (our formatter only)
    let mut unformatted_ours_contents: HashMap<String, Vec<String>> = HashMap::new();

    for variant_name in &files.unformatted_ours {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        // Track for duplicate detection
        unformatted_ours_contents
            .entry(variant_content.clone())
            .or_default()
            .push(variant_name.clone());

        match fixtures::format_with_our_formatter_with_goal(
            &variant_content,
            &fixture.input_file,
            fixture.goal(),
        ) {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedOursNotNormalized(
                        variant_name.clone(),
                    ));
                    result.add_diff(
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

    // Check for duplicate unformatted_ours files (mirrors the unformatted_* guard above)
    report_duplicate_variants(
        result,
        &unformatted_ours_contents,
        ValidationError::DuplicateUnformattedWithinFixture,
    );

    // Check for redundant unformatted_ours files (identical to prettier_variant): a
    // prettier_variant_* already covers ours → input, so a matching unformatted_ours_*
    // adds nothing.
    for (variant_content, variant_files) in &unformatted_ours_contents {
        if let Some(pv_file) = matching_prettier_variant(variant_content, &pv_contents) {
            for variant_file in variant_files {
                result.add_error(ValidationError::RedundantUnformattedMatchesPrettierVariant(
                    variant_file.clone(),
                    pv_file.clone(),
                ));
            }
        }
    }

    // N9b, N9c: variant_* validation (our formatter)
    // N9b: ours(ours(file)) == ours(file) — our output is idempotent
    // N9c: ours(file) != input — must NOT normalize to input (else should be prettier_variant_*)
    let mut variant_contents: HashMap<String, Vec<String>> = HashMap::new();
    let mut variant_ok = 0;

    for stable_name in &files.variant {
        let stable_path = fixture_dir.join(stable_name);
        let stable_content = match read_file(&stable_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        // Track for duplicate detection
        variant_contents
            .entry(stable_content.clone())
            .or_default()
            .push(stable_name.clone());

        match fixtures::format_with_our_formatter_with_goal(
            &stable_content,
            &fixture.input_file,
            fixture.goal(),
        ) {
            Ok(formatted) => {
                // N9c: Must NOT normalize to input
                if formatted == *input {
                    result.add_error(ValidationError::NormalizationVariantNormalizesToInput(
                        stable_name.clone(),
                    ));
                    continue;
                }

                // N9b: our formatter must KEEP V verbatim — `ours(V) == V`. A
                // variant_* is dual-stable: both formatters leave it as-is. The
                // looser "reaches *a* fixed point" check let through the case
                // where prettier keeps V but ours rewrites it to a *third* stable
                // form — that is a divergent_variant_* form, not a variant_*.
                if formatted != stable_content {
                    result.add_error(ValidationError::NormalizationVariantOursNotStable(
                        stable_name.clone(),
                    ));
                    result.add_diff(
                        &format!(
                            "variant not stable: {}/{}",
                            fixture.relative_path, stable_name
                        ),
                        &stable_content,
                        &formatted,
                        &diff::DiffOptions::idempotency(),
                    );
                } else {
                    variant_ok += 1;
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
    report_duplicate_variants(
        result,
        &variant_contents,
        ValidationError::DuplicateVariantWithinFixture,
    );

    if variant_ok > 0 {
        result.add_success(ValidationSuccess::VariantVariantsOk(variant_ok));
    }

    // N11b, N11c, N11d: divergent_variant_* validation (our formatter)
    // A divergent_variant_* form V is prettier-stable (N11a, prettier phase) but our
    // formatter rewrites it to a *third* stable form:
    //   N11b: ours(V) != input — else it collapses to input (use prettier_variant_*)
    //   N11c: ours(V) != V     — else both formatters keep it (use variant_*)
    //   N11d: ours(ours(V)) == ours(V) — the rewritten third form is itself stable
    let mut divergent_variant_contents: HashMap<String, Vec<String>> = HashMap::new();
    let mut divergent_variant_ok = 0;

    for tw_name in &files.divergent_variant {
        let tw_path = fixture_dir.join(tw_name);
        let tw_content = match read_file(&tw_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        divergent_variant_contents
            .entry(tw_content.clone())
            .or_default()
            .push(tw_name.clone());

        match fixtures::format_with_our_formatter_with_goal(
            &tw_content,
            &fixture.input_file,
            fixture.goal(),
        ) {
            Ok(formatted) => {
                // N11b: ours must NOT normalize to input
                if formatted == *input {
                    result.add_error(
                        ValidationError::NormalizationDivergentVariantOursNormalizesToInput(
                            tw_name.clone(),
                        ),
                    );
                    continue;
                }

                // N11c: ours must NOT keep V verbatim (that would be a variant_*)
                if formatted == tw_content {
                    result.add_error(
                        ValidationError::NormalizationDivergentVariantOursDualStable(
                            tw_name.clone(),
                        ),
                    );
                    continue;
                }

                // N11d: the rewritten third form must itself be a fixed point
                match fixtures::format_with_our_formatter_with_goal(
                    &formatted,
                    &fixture.input_file,
                    fixture.goal(),
                ) {
                    Ok(second_pass) => {
                        if second_pass != formatted {
                            result.add_error(
                                ValidationError::NormalizationDivergentVariantOursNotStable(
                                    tw_name.clone(),
                                ),
                            );
                            result.add_diff(
                                &format!(
                                    "divergent_variant third-form not stable: {}/{}",
                                    fixture.relative_path, tw_name
                                ),
                                &formatted,
                                &second_pass,
                                &diff::DiffOptions::idempotency(),
                            );
                        } else {
                            divergent_variant_ok += 1;
                        }
                    }
                    Err(e) => {
                        result.add_error(ValidationError::FormatterError(format!(
                            "{tw_name} (second pass): {e}"
                        )));
                    }
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!("{tw_name}: {e}")));
            }
        }
    }

    // Check for duplicate divergent_variant files
    report_duplicate_variants(
        result,
        &divergent_variant_contents,
        ValidationError::DuplicateDivergentVariantWithinFixture,
    );

    if divergent_variant_ok > 0 {
        result.add_success(ValidationSuccess::DivergentVariantOursOk(
            divergent_variant_ok,
        ));
    }

    if total_variants > 0 {
        result.add_success(ValidationSuccess::NormalizationVariantsOk(total_variants));
    }
}

/// N1, N3, N6, N7, N7b, N8, N9a, N10, N11a: Validate prettier normalization behavior
///
/// Orchestrates the per-rule helpers below. Each rule lives in its own function
/// so a skip or early return inside one rule can't silently disable the rules
/// after it (the bug class that once hid N6/N7/N7b/N8/N10 behind an N3 skip).
pub(in crate::fixtures::validation) async fn validate_normalization_prettier(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    files: &FixtureFiles,
) {
    validate_n1_prettier_variants_preserved(result, fixture, files).await;
    validate_n9a_variants_preserved(result, fixture, files).await;
    validate_n11a_divergent_variant_preserved(result, fixture, files).await;
    validate_n3_unformatted_normalizes(result, fixture, input, files).await;
    let unformatted_ours_outputs =
        validate_n6_unformatted_ours(result, fixture, input, input_ext, files).await;
    validate_n7_prettier_intermediates(
        result,
        fixture,
        input,
        input_ext,
        files,
        &unformatted_ours_outputs,
    )
    .await;
    validate_n7b_intermediates_to_variant(
        result,
        fixture,
        input,
        input_ext,
        files,
        &unformatted_ours_outputs,
    )
    .await;
    validate_n7c_intermediates_to_divergent_variant(
        result,
        fixture,
        input,
        input_ext,
        files,
        &unformatted_ours_outputs,
    )
    .await;
    validate_n8_unformatted_prettier(result, fixture, files).await;
    validate_n10_cross_path_discovery(
        result,
        fixture,
        input,
        input_ext,
        files,
        &unformatted_ours_outputs,
    );
}

/// Shared body for the "prettier preserves this stable-form file verbatim" checks
/// (N1 `prettier_variant_*`, N9a `variant_*`, N11a `divergent_variant_*`): assert
/// `prettier(file) == file` for each named file, reporting `not_preserved` on a
/// mismatch and counting the stable ones into `stable_success`. The three kinds
/// differ only in their file list, error/success variants, and diff label.
async fn validate_prettier_preserves(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    file_names: &[String],
    label: &str,
    not_preserved: fn(String) -> ValidationError,
    stable_success: fn(usize) -> ValidationSuccess,
) {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    let mut stable = 0;

    for name in file_names {
        let path = fixture_dir.join(name);
        let content = match read_file(&path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        match run_prettier(&content, prettier_parser).await {
            Ok(formatted) => {
                if formatted != content {
                    result.add_error(not_preserved(name.clone()));
                    result.add_diff(
                        &format!("{label} not preserved: {}/{}", fixture.relative_path, name),
                        &content,
                        &formatted,
                        &diff::DiffOptions::prettier_behavior(),
                    );
                } else {
                    stable += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "Prettier on {name}: {e}"
                )));
            }
        }
    }

    if stable > 0 {
        result.add_success(stable_success(stable));
    }
}

/// N1: prettier(prettier_variant_*) == prettier_variant_* (prettier preserves its stable variants)
async fn validate_n1_prettier_variants_preserved(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    files: &FixtureFiles,
) {
    validate_prettier_preserves(
        result,
        fixture,
        &files.prettier_variant,
        "prettier_variant",
        ValidationError::NormalizationPrettierVariantNotPreserved,
        ValidationSuccess::PrettierVariantsStable,
    )
    .await;
}

/// N9a: prettier(variant_*) == variant_* (prettier preserves these too)
async fn validate_n9a_variants_preserved(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    files: &FixtureFiles,
) {
    validate_prettier_preserves(
        result,
        fixture,
        &files.variant,
        "variant",
        ValidationError::NormalizationVariantNotPreserved,
        ValidationSuccess::VariantsStable,
    )
    .await;
}

/// N11a: prettier(divergent_variant_*) == divergent_variant_* (prettier preserves these too)
///
/// A divergent_variant_* form is prettier-stable by definition; the ours-side checks
/// (N11b–d, in `validate_normalization_ours`) verify that our formatter rewrites
/// it to a distinct third stable form.
async fn validate_n11a_divergent_variant_preserved(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    files: &FixtureFiles,
) {
    validate_prettier_preserves(
        result,
        fixture,
        &files.divergent_variant,
        "divergent_variant",
        ValidationError::NormalizationDivergentVariantNotPreserved,
        ValidationSuccess::DivergentVariantStable,
    )
    .await;
}

/// N3: prettier(unformatted_*) == input
///
/// Runs in every directory that has unformatted_* files: S9 only allows them where
/// input is prettier-stable (plain dirs, and divergence dirs without output_prettier),
/// so prettier normalizing them to input is always the claim to validate.
async fn validate_n3_unformatted_normalizes(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    files: &FixtureFiles,
) {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    let mut normalized = 0;

    for variant_name in &files.unformatted {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        match run_prettier(&variant_content, prettier_parser).await {
            Ok(formatted) => {
                if formatted != *input {
                    result.add_error(ValidationError::NormalizationUnformattedPrettierMismatch(
                        variant_name.clone(),
                    ));
                    result.add_diff(
                        &format!(
                            "prettier normalization: {}/{}",
                            fixture.relative_path, variant_name
                        ),
                        input,
                        &formatted,
                        &diff::DiffOptions::prettier_behavior(),
                    );
                } else {
                    normalized += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "Prettier on {variant_name}: {e}"
                )));
            }
        }
    }

    if normalized > 0 {
        result.add_success(ValidationSuccess::UnformattedPrettierNormalized(normalized));
    }
}

/// N6: prettier(unformatted_ours_*) != input
///
/// unformatted_ours_* files claim that only our formatter normalizes them to input,
/// so prettier should NOT normalize them to input (otherwise they should be unformatted_*).
///
/// Returns prettier's output per unformatted_ours_* suffix, consumed by the
/// N7/N7b/N10 helpers (entries exist only where prettier's output differs from input).
async fn validate_n6_unformatted_ours(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    files: &FixtureFiles,
) -> HashMap<String, String> {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    let mut unformatted_ours_prettier_outputs: HashMap<String, String> = HashMap::new();

    for variant_name in &files.unformatted_ours {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = match read_file(&variant_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        match run_prettier(&variant_content, prettier_parser).await {
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
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "Prettier on {variant_name}: {e}"
                )));
            }
        }
    }

    if !unformatted_ours_prettier_outputs.is_empty() {
        result.add_success(ValidationSuccess::UnformattedOursDivergent(
            unformatted_ours_prettier_outputs.len(),
        ));
    }

    unformatted_ours_prettier_outputs
}

/// N7: prettier_intermediate_* validation
///
/// These files capture prettier's unstable first-pass output from unformatted_ours_* files.
async fn validate_n7_prettier_intermediates(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    files: &FixtureFiles,
    unformatted_ours_prettier_outputs: &HashMap<String, String>,
) {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    let mut converged = 0;

    for intermediate_name in &files.prettier_intermediate {
        let intermediate_path = fixture_dir.join(intermediate_name);
        let intermediate_content = match read_file(&intermediate_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
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
            result.add_diff(
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
                    result.add_diff(
                        &format!(
                            "prettier_intermediate not converging: {}/{}",
                            fixture.relative_path, intermediate_name
                        ),
                        &second_pass,
                        input,
                        &diff::DiffOptions::prettier_behavior(),
                    );
                } else {
                    converged += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "Prettier on {intermediate_name}: {e}"
                )));
            }
        }
    }

    if converged > 0 {
        result.add_success(ValidationSuccess::PrettierIntermediatesConverge(converged));
    }
}

/// N7b: prettier_intermediate_to_variant_* validation
///
/// Like N7, but the second pass must converge to a documented variant_*/prettier_variant_*
/// file (not input).
async fn validate_n7b_intermediates_to_variant(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    files: &FixtureFiles,
    unformatted_ours_prettier_outputs: &HashMap<String, String>,
) {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    // Pre-read variant_*/prettier_variant_* contents — these are the allowed convergence
    // targets. Read failures are tolerated without an error: N1/N9a own these files and
    // report unreadable ones loudly, so a silent skip here can't hide a gap.
    let mut variant_target_contents: Vec<String> = Vec::new();
    for pv_name in &files.prettier_variant {
        if let Ok(content) = read_file(&fixture_dir.join(pv_name)) {
            variant_target_contents.push(content);
        }
    }
    for v_name in &files.variant {
        if let Ok(content) = read_file(&fixture_dir.join(v_name)) {
            variant_target_contents.push(content);
        }
    }

    let mut converged = 0;

    for intermediate_name in &files.prettier_intermediate_to_variant {
        let intermediate_path = fixture_dir.join(intermediate_name);
        let intermediate_content = match read_file(&intermediate_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
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
            result.add_diff(
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
                        result.add_diff(
                            &format!(
                                "prettier_intermediate_to_variant not converging: {}/{}",
                                fixture.relative_path, intermediate_name
                            ),
                            &second_pass,
                            first_target,
                            &diff::DiffOptions::prettier_behavior(),
                        );
                    }
                } else {
                    converged += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "Prettier on {intermediate_name}: {e}"
                )));
            }
        }
    }

    if converged > 0 {
        result.add_success(ValidationSuccess::PrettierIntermediatesToVariantConverge(
            converged,
        ));
    }
}

/// N7c: prettier_intermediate_to_divergent_variant_* validation
///
/// The `divergent_variant`-targeted sibling of N7b: prettier's unstable first-pass
/// output from an `unformatted_ours_*` shell whose second pass converges to a
/// documented `divergent_variant_*` (a prettier-stable form our formatter rewrites
/// to a third form), rather than input (N7) or a `variant_*`/`prettier_variant_*`
/// (N7b). This is the convergence target no other intermediate marker accepts —
/// it arises when the intersection first-member redundant-paren shell's prettier
/// path settles on a glued form ours un-glues. Checks mirror N7b exactly, only the
/// convergence target set differs.
async fn validate_n7c_intermediates_to_divergent_variant(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    files: &FixtureFiles,
    unformatted_ours_prettier_outputs: &HashMap<String, String>,
) {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    // Pre-read divergent_variant_* contents — the allowed convergence targets.
    // Read failures are tolerated without an error: N11a owns these files and
    // reports unreadable ones loudly, so a silent skip here can't hide a gap.
    let mut divergent_variant_target_contents: Vec<String> = Vec::new();
    for dv_name in &files.divergent_variant {
        if let Ok(content) = read_file(&fixture_dir.join(dv_name)) {
            divergent_variant_target_contents.push(content);
        }
    }

    let mut converged = 0;

    for intermediate_name in &files.prettier_intermediate_to_divergent_variant {
        let intermediate_path = fixture_dir.join(intermediate_name);
        let intermediate_content = match read_file(&intermediate_path) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        // Extract suffix: prettier_intermediate_to_divergent_variant_X.svelte -> X
        let suffix = intermediate_name
            .strip_prefix("prettier_intermediate_to_divergent_variant_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("");

        // Check 1: Must have corresponding unformatted_ours_* file
        let Some(expected_content) = unformatted_ours_prettier_outputs.get(suffix) else {
            result.add_error(
                ValidationError::NormalizationPrettierIntermediateToDivergentVariantMissingSource(
                    intermediate_name.clone(),
                ),
            );
            continue;
        };

        // Check 2: must have at least one divergent_variant_* file as convergence target
        if divergent_variant_target_contents.is_empty() {
            result.add_error(
                ValidationError::NormalizationPrettierIntermediateToDivergentVariantNoVariantTarget(
                    intermediate_name.clone(),
                ),
            );
            continue;
        }

        // Check 3: prettier(unformatted_ours_X) == prettier_intermediate_to_divergent_variant_X
        if *expected_content != intermediate_content {
            result.add_error(
                ValidationError::NormalizationPrettierIntermediateToDivergentVariantMismatch(
                    intermediate_name.clone(),
                ),
            );
            result.add_diff(
                &format!(
                    "prettier_intermediate_to_divergent_variant mismatch: {}/{}",
                    fixture.relative_path, intermediate_name
                ),
                &intermediate_content,
                expected_content,
                &diff::DiffOptions::freshness(),
            );
            continue;
        }

        // Check 4: prettier(intermediate) != intermediate (unstable)
        match run_prettier(&intermediate_content, prettier_parser).await {
            Ok(second_pass) => {
                if second_pass == intermediate_content {
                    result.add_error(
                        ValidationError::NormalizationPrettierIntermediateToDivergentVariantIsStable(
                            intermediate_name.clone(),
                        ),
                    );
                    continue;
                }

                // Check 5: second pass must NOT equal input (else use prettier_intermediate_* instead)
                if second_pass == *input {
                    result.add_error(
                        ValidationError::NormalizationPrettierIntermediateToDivergentVariantConvergesToInput(
                            intermediate_name.clone(),
                        ),
                    );
                    continue;
                }

                // Check 6: second pass must match some divergent_variant_* content
                let hits_divergent_variant =
                    divergent_variant_target_contents.contains(&second_pass);
                if !hits_divergent_variant {
                    result.add_error(
                        ValidationError::NormalizationPrettierIntermediateToDivergentVariantNotConverging(
                            intermediate_name.clone(),
                        ),
                    );
                    if let Some(first_target) = divergent_variant_target_contents.first() {
                        result.add_diff(
                            &format!(
                                "prettier_intermediate_to_divergent_variant not converging: {}/{}",
                                fixture.relative_path, intermediate_name
                            ),
                            &second_pass,
                            first_target,
                            &diff::DiffOptions::prettier_behavior(),
                        );
                    }
                } else {
                    converged += 1;
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "Prettier on {intermediate_name}: {e}"
                )));
            }
        }
    }

    if converged > 0 {
        result.add_success(
            ValidationSuccess::PrettierIntermediatesToDivergentVariantConverge(converged),
        );
    }
}

/// N8: unformatted_prettier_* validation
///
/// These files test that prettier normalizes certain inputs to output_prettier.*.
async fn validate_n8_unformatted_prettier(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    files: &FixtureFiles,
) {
    let fixture_dir = &fixture.path;
    let prettier_parser = fixture.input_type().prettier_parser();

    if !files.unformatted_prettier.is_empty() {
        // Must have output_prettier.* to validate against
        let output_prettier_path = fixture.output_prettier_path();
        let output_prettier_content = if output_prettier_path.exists() {
            match read_file(&output_prettier_path) {
                Ok(c) => Some(c),
                Err(e) => {
                    // Unreadable is distinct from missing: report the read failure
                    // instead of a misleading per-variant MissingTarget error.
                    result.add_error(ValidationError::FileReadError(e));
                    return;
                }
            }
        } else {
            None
        };

        let mut normalized = 0;

        for variant_name in &files.unformatted_prettier {
            let variant_path = fixture_dir.join(variant_name);
            let variant_content = match read_file(&variant_path) {
                Ok(c) => c,
                Err(e) => {
                    result.add_error(ValidationError::FileReadError(e));
                    continue;
                }
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
            match run_prettier(&variant_content, prettier_parser).await {
                Ok(formatted) => {
                    if formatted != *expected_output {
                        result.add_error(
                            ValidationError::NormalizationUnformattedPrettierNotNormalized(
                                variant_name.clone(),
                            ),
                        );
                        result.add_diff(
                            &format!(
                                "prettier normalization to output_prettier: {}/{}",
                                fixture.relative_path, variant_name
                            ),
                            expected_output,
                            &formatted,
                            &diff::DiffOptions::prettier_behavior(),
                        );
                    } else {
                        normalized += 1;
                    }
                }
                Err(e) => {
                    result.add_error(ValidationError::FormatterError(format!(
                        "Prettier on {variant_name}: {e}"
                    )));
                }
            }
        }

        if normalized > 0 {
            result.add_success(ValidationSuccess::UnformattedPrettierToOutput(normalized));
        }
    }
}

/// N10: Cross-path discovery — find undocumented Prettier outputs
///
/// After N7, check which unformatted_ours_* prettier outputs weren't consumed by
/// prettier_intermediate_*, then check if those outputs match any known file content
/// (output_prettier, prettier_variant_*, variant_*, divergent_variant_*).
fn validate_n10_cross_path_discovery(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    files: &FixtureFiles,
    unformatted_ours_prettier_outputs: &HashMap<String, String>,
) {
    let fixture_dir = &fixture.path;

    // Build set of suffixes claimed by prettier_intermediate_* and prettier_intermediate_to_variant_*
    let mut claimed_suffixes: std::collections::HashSet<String> = std::collections::HashSet::new();
    for intermediate_name in &files.prettier_intermediate {
        let suffix = intermediate_name
            .strip_prefix("prettier_intermediate_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("")
            .to_string();
        claimed_suffixes.insert(suffix);
    }
    for intermediate_name in &files.prettier_intermediate_to_variant {
        let suffix = intermediate_name
            .strip_prefix("prettier_intermediate_to_variant_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("")
            .to_string();
        claimed_suffixes.insert(suffix);
    }
    for intermediate_name in &files.prettier_intermediate_to_divergent_variant {
        let suffix = intermediate_name
            .strip_prefix("prettier_intermediate_to_divergent_variant_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("")
            .to_string();
        claimed_suffixes.insert(suffix);
    }

    // Also claim suffixes where prettier(unformatted_ours_*) == input (those got N6 errors, not novel)
    // These are already not in unformatted_ours_prettier_outputs (they were flagged as errors)

    // Build known content set from output_prettier, prettier_variant_*, variant_*,
    // divergent_variant_*. Read failures are tolerated here without an error: F2/N1/N9a/N11a
    // own these files and report unreadable ones loudly, so a silent skip here can't
    // hide a gap.
    let mut known_contents: Vec<String> = Vec::new();

    // output_prettier content
    let output_prettier_path = fixture.output_prettier_path();
    if output_prettier_path.exists()
        && let Ok(content) = read_file(&output_prettier_path)
    {
        known_contents.push(content);
    }

    // prettier_variant_* contents
    for pv_name in &files.prettier_variant {
        let pv_path = fixture_dir.join(pv_name);
        if let Ok(content) = read_file(&pv_path) {
            known_contents.push(content);
        }
    }

    // variant_* contents
    for stable_name in &files.variant {
        let stable_path = fixture_dir.join(stable_name);
        if let Ok(content) = read_file(&stable_path) {
            known_contents.push(content);
        }
    }

    // divergent_variant_* contents — a prettier-stable form our formatter rewrites; it is
    // a documented target for an unformatted_ours_* whose prettier output lands on
    // it (e.g. the heritage own-line form), so include it in the known set.
    for tw_name in &files.divergent_variant {
        let tw_path = fixture_dir.join(tw_name);
        if let Ok(content) = read_file(&tw_path) {
            known_contents.push(content);
        }
    }

    // Check unclaimed outputs
    let mut pinned = 0;
    for (suffix, prettier_output) in unformatted_ours_prettier_outputs {
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
            // output_prettier / prettier_variant_* / variant_* / divergent_variant_*
            // files), every unformatted_ours_* prettier output must match one — an
            // unmatched output means prettier drifted or the target is
            // undocumented, so block. Fixtures that document the divergence by
            // README alone (no stable-form files) keep this informational.
            if known_contents.is_empty() {
                result
                    .undocumented_prettier_outputs
                    .push(UndocumentedPrettierOutput { source_file });
            } else {
                result.add_error(ValidationError::UndocumentedPrettierOutput(source_file));
            }
        } else {
            pinned += 1;
        }
    }

    if pinned > 0 {
        result.add_success(ValidationSuccess::PrettierOutputsPinned(pinned));
    }
}
