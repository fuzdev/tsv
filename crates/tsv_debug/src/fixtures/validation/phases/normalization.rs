//! N-phase normalization validation (N* rules: variant normalization, prettier intermediates).

use std::collections::{HashMap, HashSet};

use crate::deno::run_prettier;
use crate::diff;
use crate::fixtures::{
    self, AuditSignature, ChainAnchor, ChainWalk, Fixture, FixtureFiles, SingleFormPins,
    audit_signature_variant_suffix, classify_stable_form, read_file, unformatted_ours_filename,
    unformatted_ours_suffix,
};

use super::super::errors::{
    AuditSignatureStaleness, UnpinnedChainReason, ValidationError, ValidationSuccess,
};
use super::super::{FixtureValidation, UndocumentedPrettierOutput};

/// What already pins an `unformatted_ours_*` suffix's prettier chain, so
/// [`validate_n10_cross_path_discovery`] does not report it as undocumented.
///
/// Two spellings, because a chain can be pinned by NAME or by CONTENT: N12 pins a whole
/// chain under its own suffix, while an intermediate pins one unstable form — and a suffix
/// whose first pass lands on a form a sibling already recorded is covered by that one file
/// (S22 forbids the byte-copy that would give it a file of its own).
struct ChainPins {
    /// Suffixes whose chain `audit_signature_<suffix>.txt` pins (N12).
    signature_suffixes: HashSet<String>,
    /// Contents of the `prettier_intermediate*_*` files that verified (N7/N7b/N7c).
    intermediate_forms: Vec<String>,
}

impl ChainPins {
    /// Is this suffix's prettier output already pinned — by its own chain signature, or by
    /// a sibling intermediate holding the same form?
    fn covers(&self, suffix: &str, prettier_output: &str) -> bool {
        self.signature_suffixes.contains(suffix)
            || self
                .intermediate_forms
                .iter()
                .any(|form| form == prettier_output)
    }
}

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

        match fixtures::format_with_our_formatter(&pv_content, pv_name, fixture.goal()) {
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

        match fixtures::format_with_our_formatter(
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

        match fixtures::format_with_our_formatter(
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

        match fixtures::format_with_our_formatter(
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

        match fixtures::format_with_our_formatter(&tw_content, &fixture.input_file, fixture.goal())
        {
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
                match fixtures::format_with_our_formatter(
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

/// N1, N3, N6, N7, N7b, N7c, N8, N9a, N10, N11a, N12: Validate prettier normalization behavior
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
    // The three intermediate families return the forms they verified. A suffix whose own
    // prettier output IS one of them is pinned by that file — S22 forbids a byte-copy under
    // a second name, so the sharing is how two sources whose chains coincide are both
    // covered by the one file that records the form.
    let input_target = [input.to_string()];
    let variant_targets =
        read_contents(fixture, files.prettier_variant.iter().chain(&files.variant));
    let divergent_variant_targets = read_contents(fixture, &files.divergent_variant);
    let mut intermediate_forms = Vec::new();
    for (family, names, targets) in [
        (
            &N7_INTERMEDIATE,
            &files.prettier_intermediate,
            &input_target[..],
        ),
        (
            &N7B_INTERMEDIATE_TO_VARIANT,
            &files.prettier_intermediate_to_variant,
            &variant_targets[..],
        ),
        (
            &N7C_INTERMEDIATE_TO_DIVERGENT_VARIANT,
            &files.prettier_intermediate_to_divergent_variant,
            &divergent_variant_targets[..],
        ),
    ] {
        intermediate_forms.extend(
            validate_intermediate_family(
                result,
                fixture,
                input,
                family,
                names,
                targets,
                &unformatted_ours_outputs,
            )
            .await,
        );
    }
    validate_n8_unformatted_prettier(result, fixture, files).await;
    // N12 before N10: the chain signatures it verifies are exactly what N10 must not
    // report as undocumented.
    let pins = SingleFormPins::collect(fixture, input_ext, files);
    let chain_pinned = validate_n12_variant_chain_signatures(
        result,
        fixture,
        input,
        input_ext,
        files,
        &unformatted_ours_outputs,
        &pins,
    )
    .await;
    let chain_pins = ChainPins {
        signature_suffixes: chain_pinned,
        intermediate_forms,
    };
    validate_n10_cross_path_discovery(
        result,
        fixture,
        input,
        input_ext,
        &unformatted_ours_outputs,
        &pins,
        &chain_pins,
    )
    .await;
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
                    // Store for prettier_intermediate_* / N12 validation, keyed by suffix.
                    let suffix = unformatted_ours_suffix(variant_name, input_ext).unwrap_or("");
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

/// One `prettier_intermediate*_*` family — N7 / N7b / N7c differ only in what this names:
/// the filename prefix, which checks apply, and the error or success each check reports.
struct IntermediateFamily {
    /// Filename prefix, e.g. `prettier_intermediate_`; without its trailing `_` it labels the
    /// family's diffs ([`Self::label`]).
    prefix: &'static str,
    missing_source: fn(String) -> ValidationError,
    mismatch: fn(String) -> ValidationError,
    is_stable: fn(String) -> ValidationError,
    /// The checks a family whose convergence targets are documented variant files adds
    /// (N7b / N7c); `None` for N7, whose target is the input itself.
    variant_target: Option<VariantTargetChecks>,
    not_converging: fn(String) -> ValidationError,
    converged: fn(usize) -> ValidationSuccess,
}

/// The two checks only a variant-targeted intermediate family runs — one field, so a family
/// cannot carry one without the other.
struct VariantTargetChecks {
    /// The documented variant targets must exist.
    no_target: fn(String) -> ValidationError,
    /// A second pass that lands on the input belongs to plain N7 instead.
    converges_to_input: fn(String) -> ValidationError,
}

impl IntermediateFamily {
    /// The label this family's diffs carry: its prefix without the trailing `_`.
    fn label(&self) -> &'static str {
        self.prefix.trim_end_matches('_')
    }
}

/// N7: `prettier_intermediate_*` — prettier's unstable first pass over an `unformatted_ours_*`
/// file, whose second pass converges to the input.
const N7_INTERMEDIATE: IntermediateFamily = IntermediateFamily {
    prefix: "prettier_intermediate_",
    missing_source: ValidationError::NormalizationPrettierIntermediateMissingSource,
    mismatch: ValidationError::NormalizationPrettierIntermediateMismatch,
    is_stable: ValidationError::NormalizationPrettierIntermediateIsStable,
    variant_target: None,
    not_converging: ValidationError::NormalizationPrettierIntermediateNotConverging,
    converged: ValidationSuccess::PrettierIntermediatesConverge,
};

/// N7b: `prettier_intermediate_to_variant_*` — like N7, but the second pass converges to a
/// documented `variant_*` / `prettier_variant_*` file.
const N7B_INTERMEDIATE_TO_VARIANT: IntermediateFamily = IntermediateFamily {
    prefix: "prettier_intermediate_to_variant_",
    missing_source: ValidationError::NormalizationPrettierIntermediateToVariantMissingSource,
    mismatch: ValidationError::NormalizationPrettierIntermediateToVariantMismatch,
    is_stable: ValidationError::NormalizationPrettierIntermediateToVariantIsStable,
    variant_target: Some(VariantTargetChecks {
        no_target: ValidationError::NormalizationPrettierIntermediateToVariantNoVariantTarget,
        converges_to_input:
            ValidationError::NormalizationPrettierIntermediateToVariantConvergesToInput,
    }),
    not_converging: ValidationError::NormalizationPrettierIntermediateToVariantNotConverging,
    converged: ValidationSuccess::PrettierIntermediatesToVariantConverge,
};

/// N7c: `prettier_intermediate_to_divergent_variant_*` — the second pass converges to a
/// documented `divergent_variant_*` (a prettier-stable form our formatter rewrites to a third
/// form), the target no other intermediate marker accepts. It arises when the intersection
/// first-member redundant-paren shell's prettier path settles on a glued form ours un-glues.
const N7C_INTERMEDIATE_TO_DIVERGENT_VARIANT: IntermediateFamily = IntermediateFamily {
    prefix: "prettier_intermediate_to_divergent_variant_",
    missing_source:
        ValidationError::NormalizationPrettierIntermediateToDivergentVariantMissingSource,
    mismatch: ValidationError::NormalizationPrettierIntermediateToDivergentVariantMismatch,
    is_stable: ValidationError::NormalizationPrettierIntermediateToDivergentVariantIsStable,
    variant_target: Some(VariantTargetChecks {
        no_target:
            ValidationError::NormalizationPrettierIntermediateToDivergentVariantNoVariantTarget,
        converges_to_input:
            ValidationError::NormalizationPrettierIntermediateToDivergentVariantConvergesToInput,
    }),
    not_converging:
        ValidationError::NormalizationPrettierIntermediateToDivergentVariantNotConverging,
    converged: ValidationSuccess::PrettierIntermediatesToDivergentVariantConverge,
};

/// The readable contents of `names` in the fixture directory — a variant-targeted family's
/// convergence targets. Read failures are tolerated without an error: N1 / N9a / N11a own these
/// files and report an unreadable one loudly, so a silent skip here can't hide a gap.
fn read_contents<'a>(
    fixture: &Fixture,
    names: impl IntoIterator<Item = &'a String>,
) -> Vec<String> {
    names
        .into_iter()
        .filter_map(|name| read_file(&fixture.path.join(name)).ok())
        .collect()
}

/// N7 / N7b / N7c: validate one intermediate family's files. Each `<prefix><suffix>` file must
/// be prettier's first pass over `unformatted_ours_<suffix>`, must be unstable under a second
/// pass, and that second pass must land on one of `targets` — the input for N7, the documented
/// variant files for N7b / N7c.
///
/// Returns the contents of the intermediates that verified — the forms a sibling suffix
/// may share (see [`validate_n10_cross_path_discovery`]).
async fn validate_intermediate_family(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    family: &IntermediateFamily,
    names: &[String],
    targets: &[String],
    unformatted_ours_prettier_outputs: &HashMap<String, String>,
) -> Vec<String> {
    let input_ext = fixture.input_type().extension();
    let prettier_parser = fixture.input_type().prettier_parser();

    let mut verified_contents = Vec::new();
    let mut converged = 0;

    for name in names {
        let content = match read_file(&fixture.path.join(name)) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        // Extract suffix: <prefix>X.svelte -> X
        let suffix = name
            .strip_prefix(family.prefix)
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("");

        // Must have a corresponding unformatted_ours_* file
        let Some(expected_content) = unformatted_ours_prettier_outputs.get(suffix) else {
            result.add_error((family.missing_source)(name.clone()));
            continue;
        };

        // A variant-targeted family needs at least one documented target to converge to
        if let Some(checks) = &family.variant_target
            && targets.is_empty()
        {
            result.add_error((checks.no_target)(name.clone()));
            continue;
        }

        // prettier(unformatted_ours_X) == <prefix>X
        if *expected_content != content {
            result.add_error((family.mismatch)(name.clone()));
            result.add_diff(
                &format!(
                    "{} mismatch: {}/{}",
                    family.label(),
                    fixture.relative_path,
                    name
                ),
                &content,
                expected_content,
                &diff::DiffOptions::freshness(),
            );
            continue;
        }

        // prettier(<prefix>X) != <prefix>X — an intermediate is unstable by definition (a
        // stable one belongs in prettier_variant_* instead)
        match run_prettier(&content, prettier_parser).await {
            Ok(second_pass) => {
                if second_pass == content {
                    result.add_error((family.is_stable)(name.clone()));
                    continue;
                }

                // A second pass that lands on the input belongs to plain N7
                if let Some(checks) = &family.variant_target
                    && second_pass == *input
                {
                    result.add_error((checks.converges_to_input)(name.clone()));
                    continue;
                }

                if targets.contains(&second_pass) {
                    converged += 1;
                    verified_contents.push(content);
                } else {
                    result.add_error((family.not_converging)(name.clone()));
                    if let Some(first_target) = targets.first() {
                        result.add_diff(
                            &format!(
                                "{} not converging: {}/{}",
                                family.label(),
                                fixture.relative_path,
                                name
                            ),
                            &second_pass,
                            first_target,
                            &diff::DiffOptions::prettier_behavior(),
                        );
                    }
                }
            }
            Err(e) => {
                result.add_error(ValidationError::FormatterError(format!(
                    "Prettier on {name}: {e}"
                )));
            }
        }
    }

    if converged > 0 {
        result.add_success((family.converged)(converged));
    }

    verified_contents
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
/// The last word on every `unformatted_ours_*` prettier output: whatever N7/N7b/N7c did not
/// claim by suffix, N12 did not pin as a chain, and no documented stable form holds
/// (`SingleFormPins`) is reported here. Blocking when the fixture documents any stable form
/// at all — an unmatched output then means prettier drifted or the target is undocumented.
/// In a README-only fixture the report splits by what pin the output REQUIRES (the mirror of
/// the updater's `needs_chain_pin`): a multi-pass chain or a stable form tsv cannot format
/// has an auto-generated pin as its only expression, so its absence BLOCKS
/// (`UnpinnedPrettierChain` — a deleted pin must not degrade the claim to a note); a stable
/// form a single-form marker could express, a tsv non-idempotency, or a truncated chain
/// stays informational.
async fn validate_n10_cross_path_discovery(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    unformatted_ours_prettier_outputs: &HashMap<String, String>,
    pins: &SingleFormPins,
    chain_pins: &ChainPins,
) {
    // Also claim suffixes where prettier(unformatted_ours_*) == input (those got N6 errors, not novel)
    // These are already not in unformatted_ours_prettier_outputs (they were flagged as errors)

    // Check unclaimed outputs
    let mut pinned = 0;
    for (suffix, prettier_output) in unformatted_ours_prettier_outputs {
        if pins.claims_suffix(suffix) {
            continue;
        }

        // Already pinned: N12 recorded this suffix's whole chain, or a sibling's
        // `prettier_intermediate*_*` records this exact first-pass form and N7/N7b/N7c
        // verified the chain out of it. Either way it is not undocumented.
        if chain_pins.covers(suffix, prettier_output) {
            continue;
        }

        // Check against input
        if *prettier_output == *input {
            continue; // Already flagged by N6
        }

        // Check against known contents
        if !pins.matches_stable_form(prettier_output) {
            let source_file = unformatted_ours_filename(suffix, input_ext);
            // When the fixture documents prettier's stable forms (it has
            // output_prettier / prettier_variant_* / variant_* / divergent_variant_*
            // files), every unformatted_ours_* prettier output must match one — an
            // unmatched output means prettier drifted or the target is
            // undocumented, so block. Fixtures that document the divergence by
            // README alone (no stable-form files) keep this informational.
            // `fixtures_update_formatted`'s `update_intermediate_files` fails a declined
            // stable first pass on this same condition.
            if pins.has_documented_forms() {
                result.add_error(ValidationError::UndocumentedPrettierOutput(source_file));
            } else {
                // The fixture documents no stable form (a README-only divergence). Split by
                // what prettier's own next pass says — the same classification the updater
                // uses to decide the required pin (`needs_chain_pin`), asked here from the
                // ABSENCE side: the two shapes whose only pin is auto-generated
                // (a multi-pass chain; a stable form tsv cannot format) BLOCK when that pin
                // is missing, since a deleted `prettier_intermediate*_*` /
                // `audit_signature_<suffix>.txt` otherwise degrades the claim to a note.
                // One extra prettier pass, and only for an output that reached this arm,
                // which by construction is rare.
                match run_prettier(prettier_output, fixture.input_type().prettier_parser()).await {
                    Ok(second) if second == *prettier_output => {
                        // A one-pass-stable output; which pin (if any) can hold it is the
                        // shared `ours(V)` test.
                        let marker = classify_stable_form(
                            prettier_output,
                            input,
                            &fixture.input_file,
                            fixture.goal(),
                        );
                        match marker.file_prefix() {
                            Some(prefix) => {
                                // A single-form marker can express it — deliberately left
                                // unpinned (N12 declines these so the audit keeps suggesting
                                // the more informative marker). Name the file to add.
                                result.undocumented_prettier_outputs.push(
                                    UndocumentedPrettierOutput {
                                        source_file,
                                        suggested_pin: Some(format!("{prefix}{suffix}{input_ext}")),
                                    },
                                );
                            }
                            None if marker.requires_chain_pin() => {
                                result.add_error(ValidationError::UnpinnedPrettierChain(
                                    source_file,
                                    UnpinnedChainReason::StableFormOursRejects,
                                ));
                            }
                            None => {
                                // `OursNotIdempotent`: a tsv idempotency bug, not a pin
                                // choice — pinning the chain would paper over it. Report only.
                                result.undocumented_prettier_outputs.push(
                                    UndocumentedPrettierOutput {
                                        source_file,
                                        suggested_pin: None,
                                    },
                                );
                            }
                        }
                    }
                    Ok(_) => {
                        result.add_error(ValidationError::UnpinnedPrettierChain(
                            source_file,
                            UnpinnedChainReason::MultiPass,
                        ));
                    }
                    Err(_) => {
                        // Prettier cannot re-parse its own first pass — a prettier bug the
                        // fixture's README documents. No pin can represent a truncated
                        // chain, so none is required. Report only.
                        result
                            .undocumented_prettier_outputs
                            .push(UndocumentedPrettierOutput {
                                source_file,
                                suggested_pin: None,
                            });
                    }
                }
            }
        } else {
            pinned += 1;
        }
    }

    if pinned > 0 {
        result.add_success(ValidationSuccess::PrettierOutputsPinned(pinned));
    }
}

/// N12: `audit_signature_<suffix>.txt` byte-matches prettier's live chain from
/// `unformatted_ours_<suffix>`.
///
/// The pin for the outputs no single-form marker reaches: a chain with two or more distinct
/// intermediates (`prettier_intermediate*_*` captures exactly one unstable pass), or a
/// one-pass fixed point that is not a documented stable form — including one tsv cannot
/// ingest, where `prettier_variant_*`'s N2 could never hold. Every pass is compared
/// byte-exact, so prettier-version drift anywhere along the chain fails here.
///
/// Returns the suffixes whose signature verified, which N10 then treats as documented.
async fn validate_n12_variant_chain_signatures(
    result: &mut FixtureValidation,
    fixture: &Fixture,
    input: &str,
    input_ext: &str,
    files: &FixtureFiles,
    unformatted_ours_prettier_outputs: &HashMap<String, String>,
    pins: &SingleFormPins,
) -> HashSet<String> {
    let mut verified: HashSet<String> = HashSet::new();
    let mut matched = 0;
    let parser = fixture.input_type().prettier_parser();

    for signature_name in &files.audit_signature_variant {
        let Some(suffix) = audit_signature_variant_suffix(signature_name) else {
            continue;
        };
        // S21 already rejected a signature with no source. A source that exists but is
        // missing from the map failed under N6 (unreadable, prettier errored, or prettier
        // normalized it to input) — that rule owns the report; adding one here would
        // double-report the same defect.
        let Some(prettier_output) = unformatted_ours_prettier_outputs.get(suffix) else {
            continue;
        };

        if pins.covers(suffix, prettier_output, input) {
            result.add_error(ValidationError::VariantChainSignatureSuperseded(
                signature_name.clone(),
            ));
            continue;
        }

        let recorded_raw = match read_file(&fixture.audit_signature_variant_path(suffix)) {
            Ok(s) => s,
            Err(e) => {
                result.add_error(ValidationError::VariantChainSignatureMalformed(
                    signature_name.clone(),
                    e,
                ));
                continue;
            }
        };
        let recorded = match AuditSignature::parse(&recorded_raw) {
            Ok(s) => s,
            Err(e) => {
                result.add_error(ValidationError::VariantChainSignatureMalformed(
                    signature_name.clone(),
                    e,
                ));
                continue;
            }
        };

        let source_name = unformatted_ours_filename(suffix, input_ext);
        let source_content = match read_file(&fixture.path.join(&source_name)) {
            Ok(s) => s,
            Err(e) => {
                result.add_error(ValidationError::FileReadError(e));
                continue;
            }
        };

        let live = match AuditSignature::walk(&source_content, parser).await {
            Ok(ChainWalk::Pinned(s)) => s,
            Ok(ChainWalk::Collapsed) => {
                // Prettier holds the source itself stable, so there is no chain — the
                // source is a prettier fixed point and wants a single-form pin instead.
                result.add_error(ValidationError::VariantChainSignatureOutdated(
                    signature_name.clone(),
                    AuditSignatureStaleness::Collapsed,
                ));
                continue;
            }
            Ok(ChainWalk::Truncated { completed, error }) => {
                // The chain still exists but prettier now errors partway along it — NOT a
                // collapse, and no regenerate can repair it (the updater refuses a
                // truncated chain rather than deleting the pin). Investigate.
                result.add_error(ValidationError::VariantChainSignatureWalkFailed(
                    signature_name.clone(),
                    ChainAnchor::UnformattedOurs.truncated_message(completed, &error),
                ));
                continue;
            }
            Err(e) => {
                // Distinct from `Malformed`: the signature parsed fine, but the live chain
                // does not converge within the depth bound. Investigate rather than
                // regenerate.
                result.add_error(ValidationError::VariantChainSignatureWalkFailed(
                    signature_name.clone(),
                    e,
                ));
                continue;
            }
        };

        if live.passes == recorded.passes {
            verified.insert(suffix.to_string());
            matched += 1;
            continue;
        }

        result.add_error(ValidationError::VariantChainSignatureOutdated(
            signature_name.clone(),
            AuditSignatureStaleness::Drift,
        ));
        // Diff the first differing pass for actionable output.
        let max_len = recorded.passes.len().max(live.passes.len());
        for i in 0..max_len {
            let recorded_step = recorded.passes.get(i).map_or("", String::as_str);
            let live_step = live.passes.get(i).map_or("", String::as_str);
            if recorded_step != live_step {
                let pass_num = i + ChainAnchor::UnformattedOurs.first_pass_number();
                result.add_diff(
                    &format!(
                        "{signature_name} drift (pass={pass_num}): {}/{signature_name}",
                        fixture.relative_path
                    ),
                    recorded_step,
                    live_step,
                    &diff::DiffOptions::freshness(),
                );
                break;
            }
        }
    }

    if matched > 0 {
        result.add_success(ValidationSuccess::VariantChainSignaturesMatch(matched));
    }
    verified
}
