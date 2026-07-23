use crate::cli::CliError;
use crate::fixtures::{self, AUDIT_SIGNATURE_FILENAME, AuditSignature, FixtureFiles};
use argh::FromArgs;
use futures_util::StreamExt;

/// Regenerate output_prettier.*, prettier_intermediate_*, and audit_signature.txt.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fixtures_update_formatted")]
pub struct FixturesUpdateFormattedCommand {
    /// fixture filter patterns (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

impl FixturesUpdateFormattedCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let rt = super::create_runtime();
        rt.block_on(run(&self.filters))
    }
}

async fn run(filters: &[String]) -> Result<(), CliError> {
    let (fixture_list, total_count) = super::walk_and_filter(filters)?;

    let mut created = 0;
    let mut updated = 0;
    let mut removed = 0;
    let mut unchanged = 0;
    let mut failed = 0;

    // Separate counters for prettier_intermediate_*
    let mut intermediate_created = 0;
    let mut intermediate_updated = 0;
    let mut intermediate_removed = 0;
    let mut intermediate_unchanged = 0;

    // Separate counters for audit_signature.txt
    let mut signature_created = 0;
    let mut signature_updated = 0;
    let mut signature_removed = 0;
    let mut signature_unchanged = 0;

    let matched_count = fixture_list.len();

    // Input order (`ResultOrder::Input`) so progress lines print deterministically;
    // tasks never print — all output happens here in the driver, per fixture.
    let mut results = super::spawn_work_stream(
        fixture_list,
        super::ResultOrder::Input,
        |fixture| async move {
            let outcome = process_fixture(&fixture).await;
            (fixture, outcome)
        },
    );

    while let Some(joined) = results.next().await {
        let (fixture, outcome) = super::task_result(joined, "update")?;

        let FixtureOutcome::Processed {
            formatted,
            signature,
            intermediates,
        } = outcome
        else {
            // A no-output-claim marker is present: prettier_nonconvergent.txt (no fixed
            // point), prettier_rejects.txt (prettier throws), or tsv_rejects.txt (tsv
            // rejects the input, so the fixture makes no formatting claim). None has an
            // output_prettier.* to regenerate; the validator live-verifies each (F5/F6/F7).
            let (marker, reason) = if fixture.tsv_rejects_path().exists() {
                (fixtures::TSV_REJECTS_FILENAME, "tsv rejects input")
            } else if fixture.prettier_rejects_path().exists() {
                (
                    fixtures::PRETTIER_REJECTS_FILENAME,
                    "prettier rejects input",
                )
            } else {
                (
                    fixtures::PRETTIER_NONCONVERGENT_FILENAME,
                    "prettier has no fixed point",
                )
            };
            println!("- {} skipped ({marker}: {reason})", fixture.relative_path);
            continue;
        };

        // Update output_prettier.*
        let output_filename = fixture.output_prettier_filename();
        match formatted {
            FormattedResult::Created => {
                println!("✓ Created {}/{}", fixture.relative_path, output_filename);
                created += 1;
            }
            FormattedResult::Updated => {
                println!("✓ Updated {}/{}", fixture.relative_path, output_filename);
                updated += 1;
            }
            FormattedResult::Removed => {
                println!(
                    "✓ Removed {}/{} (identical to input)",
                    fixture.relative_path, output_filename
                );
                removed += 1;
            }
            FormattedResult::Unchanged => {
                println!(
                    "- {}/{} is up to date",
                    fixture.relative_path, output_filename
                );
                unchanged += 1;
            }
            FormattedResult::NotNeeded => {
                println!(
                    "- {}/{} not needed (input already formatted)",
                    fixture.relative_path, output_filename
                );
                unchanged += 1;
            }
            FormattedResult::Failed(err) => {
                eprintln!("✗ Failed to process {}: {}", fixture.relative_path, err);
                failed += 1;
            }
        }

        // Update audit_signature.txt (pins prettier's multi-pass chain from output_prettier.*).
        // Only meaningful in _prettier_divergence dirs with output_prettier.* present.
        if let Some(signature) = signature {
            match signature {
                FormattedResult::Created => {
                    println!(
                        "✓ Created {}/{}",
                        fixture.relative_path, AUDIT_SIGNATURE_FILENAME
                    );
                    signature_created += 1;
                }
                FormattedResult::Updated => {
                    println!(
                        "✓ Updated {}/{}",
                        fixture.relative_path, AUDIT_SIGNATURE_FILENAME
                    );
                    signature_updated += 1;
                }
                FormattedResult::Removed => {
                    println!(
                        "✓ Removed {}/{} (prettier is idempotent on output_prettier)",
                        fixture.relative_path, AUDIT_SIGNATURE_FILENAME
                    );
                    signature_removed += 1;
                }
                FormattedResult::Unchanged => {
                    signature_unchanged += 1;
                }
                FormattedResult::NotNeeded => {
                    signature_unchanged += 1;
                }
                FormattedResult::Failed(err) => {
                    eprintln!(
                        "✗ Failed to update {}/{}: {}",
                        fixture.relative_path, AUDIT_SIGNATURE_FILENAME, err
                    );
                    failed += 1;
                }
            }
        }

        for item in intermediates {
            let (filename, result) = match item {
                IntermediateOutput::Note(msg) => {
                    println!("{msg}");
                    continue;
                }
                IntermediateOutput::File(filename, result) => (filename, result),
            };
            match result {
                FormattedResult::Created => {
                    println!("✓ Created {}/{}", fixture.relative_path, filename);
                    intermediate_created += 1;
                }
                FormattedResult::Updated => {
                    println!("✓ Updated {}/{}", fixture.relative_path, filename);
                    intermediate_updated += 1;
                }
                FormattedResult::Removed => {
                    println!(
                        "✓ Removed {}/{} (no intermediate needed)",
                        fixture.relative_path, filename
                    );
                    intermediate_removed += 1;
                }
                FormattedResult::Unchanged => {
                    println!("- {}/{} is up to date", fixture.relative_path, filename);
                    intermediate_unchanged += 1;
                }
                FormattedResult::NotNeeded => {
                    intermediate_unchanged += 1;
                }
                FormattedResult::Failed(err) => {
                    eprintln!(
                        "✗ Failed to process {}/{}: {}",
                        fixture.relative_path, filename, err
                    );
                    failed += 1;
                }
            }
        }
    }

    let total_created = created + intermediate_created + signature_created;
    let total_updated = updated + intermediate_updated + signature_updated;
    let total_removed = removed + intermediate_removed + signature_removed;
    let total_unchanged = unchanged + intermediate_unchanged + signature_unchanged;

    if filters.is_empty() {
        println!(
            "\nSummary: {total_created} created, {total_updated} updated, {total_removed} removed, {total_unchanged} unchanged, {failed} failed ({matched_count} fixtures)"
        );
    } else {
        println!(
            "\nSummary: {total_created} created, {total_updated} updated, {total_removed} removed, {total_unchanged} unchanged, {failed} failed (matched {matched_count} of {total_count} fixtures)"
        );
    }

    if created > 0 || updated > 0 || removed > 0 {
        println!("⚠️  Updated source of truth files (output_prettier.*)");
    }
    if intermediate_created > 0 || intermediate_updated > 0 || intermediate_removed > 0 {
        println!(
            "⚠️  Updated prettier_intermediate_* / prettier_intermediate_to_variant_* / prettier_intermediate_to_divergent_variant_* files"
        );
    }
    if signature_created > 0 || signature_updated > 0 || signature_removed > 0 {
        println!("⚠️  Updated audit_signature.txt files");
    }

    if failed > 0 {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}

enum FormattedResult {
    Created,
    Updated,
    Removed,
    Unchanged,
    NotNeeded,
    Failed(String),
}

/// Per-fixture results computed in a spawned task and printed by the driver in
/// fixture order — tasks never print, so concurrent fixtures can't interleave output.
enum FixtureOutcome {
    /// A no-output-claim marker is present — `prettier_nonconvergent.txt` (F5) or
    /// `prettier_rejects.txt` (F6), where prettier cannot format this input, or
    /// `tsv_rejects.txt` (F7), where tsv rejects the input so the fixture makes no
    /// formatting claim. Either way there is nothing to regenerate.
    Skipped,
    Processed {
        /// Result for `output_prettier.*`.
        formatted: FormattedResult,
        /// Result for `audit_signature.txt`; `Some` iff the fixture is a
        /// `_prettier_divergence` dir.
        signature: Option<FormattedResult>,
        /// Per-variant intermediate-file output, in print order; empty for
        /// non-divergence fixtures.
        intermediates: Vec<IntermediateOutput>,
    },
}

/// One unit of output from `update_intermediate_files`, in print order.
enum IntermediateOutput {
    /// Informational line, printed verbatim — no counter impact.
    Note(String),
    /// A result for the named intermediate file.
    File(String, FormattedResult),
}

/// Run all per-fixture update work (output_prettier, audit signature, intermediates).
/// Pure compute + fixture-dir-local file IO — safe to run concurrently across fixtures.
async fn process_fixture(fixture: &fixtures::Fixture) -> FixtureOutcome {
    // Skip the no-output-claim fixtures. prettier_nonconvergent.txt / prettier_rejects.txt:
    // prettier can't format the input. tsv_rejects.txt: the fixture makes NO formatting
    // claim (tsv rejects the input), so regenerating output_prettier.* would fabricate a
    // spurious claim from prettier's own acceptance of an input tsv rejects.
    if fixture.prettier_nonconvergent_path().exists()
        || fixture.prettier_rejects_path().exists()
        || fixture.tsv_rejects_path().exists()
    {
        return FixtureOutcome::Skipped;
    }

    let formatted = update_formatted_file(fixture).await;

    let (signature, intermediates) = if fixture.is_prettier_divergence() {
        let signature = update_audit_signature(fixture).await;
        let input_ext = fixture.input_type().extension();
        let intermediates = update_intermediate_files(fixture, input_ext).await;
        (Some(signature), intermediates)
    } else {
        (None, Vec::new())
    };

    FixtureOutcome::Processed {
        formatted,
        signature,
        intermediates,
    }
}

/// Update `audit_signature.txt` for a fixture.
///
/// The signature pins prettier's multi-pass chain starting from `output_prettier.*`. It's
/// created/updated when `prettier(output_prettier) != output_prettier` (prettier non-idempotent),
/// and removed when prettier is idempotent on output_prettier (chain depth zero — nothing to pin).
///
/// Skips silently if output_prettier doesn't exist (idempotent case — no chain to record).
async fn update_audit_signature(fixture: &fixtures::Fixture) -> FormattedResult {
    let signature_path = fixture.audit_signature_path();
    let output_prettier_path = fixture.output_prettier_path();

    // No output_prettier → no chain anchor → remove any stale signature
    if !output_prettier_path.exists() {
        if signature_path.exists() {
            return match fixtures::delete_file_if_exists(&signature_path) {
                Ok(()) => FormattedResult::Removed,
                Err(e) => FormattedResult::Failed(e),
            };
        }
        return FormattedResult::NotNeeded;
    }

    let output_prettier_content = match fixtures::read_file(&output_prettier_path) {
        Ok(s) => s,
        Err(e) => return FormattedResult::Failed(e),
    };

    let parser = fixture.input_type().prettier_parser();
    let chain = match AuditSignature::walk(&output_prettier_content, parser).await {
        Ok(c) => c,
        Err(e) => return FormattedResult::Failed(e),
    };

    match chain {
        None => {
            // Prettier idempotent — no signature needed. Remove stale file if any.
            if signature_path.exists() {
                match fixtures::delete_file_if_exists(&signature_path) {
                    Ok(()) => FormattedResult::Removed,
                    Err(e) => FormattedResult::Failed(e),
                }
            } else {
                FormattedResult::NotNeeded
            }
        }
        Some(sig) => {
            let serialized = sig.serialize();
            let existing = fixtures::read_file(&signature_path).ok();
            if existing.as_deref() == Some(serialized.as_str()) {
                FormattedResult::Unchanged
            } else if existing.is_none() {
                match fixtures::write_file(&signature_path, &serialized) {
                    Ok(()) => FormattedResult::Created,
                    Err(e) => FormattedResult::Failed(e),
                }
            } else {
                match fixtures::write_file(&signature_path, &serialized) {
                    Ok(()) => FormattedResult::Updated,
                    Err(e) => FormattedResult::Failed(e),
                }
            }
        }
    }
}

async fn update_formatted_file(fixture: &fixtures::Fixture) -> FormattedResult {
    // Read input file
    let input = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => return FormattedResult::Failed(e),
    };

    // Run prettier
    let formatted =
        match crate::deno::run_prettier(&input, fixture.input_type().prettier_parser()).await {
            Ok(f) => f,
            Err(e) => return FormattedResult::Failed(format!("Prettier error: {e}")),
        };

    let output_prettier_path = fixture.output_prettier_path();

    // If formatted output is identical to input, remove output_prettier file
    if formatted == input {
        if output_prettier_path.exists() {
            match fixtures::delete_file_if_exists(&output_prettier_path) {
                Ok(()) => FormattedResult::Removed,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            FormattedResult::NotNeeded
        }
    } else {
        // Formatted output differs from input, write/update output_prettier file
        let existing = fixtures::read_file(&output_prettier_path).ok();

        if Some(&formatted) == existing.as_ref() {
            FormattedResult::Unchanged
        } else if existing.is_none() {
            match fixtures::write_file(&output_prettier_path, &formatted) {
                Ok(()) => FormattedResult::Created,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            match fixtures::write_file(&output_prettier_path, &formatted) {
                Ok(()) => FormattedResult::Updated,
                Err(e) => FormattedResult::Failed(e),
            }
        }
    }
}

/// Shape of prettier's chain from an `unformatted_ours_*` variant.
///
/// Distinguishes the three "no intermediate file" cases (which all clean up any stale files)
/// from the two "write an intermediate file" cases (which differ only in target filename).
enum ChainShape {
    /// `prettier(variant) == input` — first pass normalizes directly; no intermediate needed.
    NormalizesToInput,
    /// `prettier(variant) != input` but `prettier(prettier(variant)) == prettier(variant)` —
    /// first pass is already a fixed point; no intermediate needed.
    StableFirstPass,
    /// First pass unstable, second pass equals `input` — write `prettier_intermediate_*`.
    UnstableConvergesToInput,
    /// First pass unstable, second pass equals a sibling `variant_*` / `prettier_variant_*` —
    /// write `prettier_intermediate_to_variant_*`.
    UnstableConvergesToVariant,
    /// First pass unstable, second pass equals a sibling `divergent_variant_*` (a
    /// prettier-stable form ours rewrites to a third form) — write
    /// `prettier_intermediate_to_divergent_variant_*` (N7c).
    UnstableConvergesToDivergentVariant,
    /// First pass unstable, second pass is neither `input` nor any documented variant —
    /// the chain is anchored further downstream and captured by `audit_signature.txt`
    /// alongside `output_prettier.*`; no intermediate file is appropriate.
    UnstableNotConverging,
    /// First-pass output is syntactically invalid (prettier bug) — second pass fails to parse.
    /// No chain classification is possible. Treat like `UnstableNotConverging` for file
    /// management (clean up stale intermediates) but surface the prettier bug in the message.
    FirstPassUnparseable(String),
}

/// Update prettier_intermediate_*{,_to_variant_*} files for a fixture.
///
/// For each `unformatted_ours_*` file, classifies what prettier does over one or two passes
/// (see `ChainShape`), then either removes any stale intermediate files or writes the
/// correct one. The `UnstableNotConverging` case is the interaction point with
/// `audit_signature.txt` — those fixtures pin their chain there instead.
async fn update_intermediate_files(
    fixture: &fixtures::Fixture,
    input_ext: &str,
) -> Vec<IntermediateOutput> {
    let mut results = Vec::new();

    let input = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => {
            results.push(IntermediateOutput::File(
                "prettier_intermediate_*".to_string(),
                FormattedResult::Failed(e),
            ));
            return results;
        }
    };

    let files = FixtureFiles::scan(fixture);

    // Pre-load convergence-target contents to distinguish "converges to input" from
    // "converges to a documented variant" / "…to a documented divergent_variant" on the
    // second pass.
    let mut variant_target_contents: Vec<String> = Vec::new();
    for pv_name in files.prettier_variant.iter().chain(&files.variant) {
        if let Ok(content) = fixtures::read_file(&fixture.path.join(pv_name)) {
            variant_target_contents.push(content);
        }
    }
    let mut divergent_variant_target_contents: Vec<String> = Vec::new();
    for dv_name in &files.divergent_variant {
        if let Ok(content) = fixtures::read_file(&fixture.path.join(dv_name)) {
            divergent_variant_target_contents.push(content);
        }
    }

    for variant_name in &files.unformatted_ours {
        // Extract suffix: unformatted_ours_X.svelte -> X
        let suffix = variant_name
            .strip_prefix("unformatted_ours_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("");

        let plain_filename = format!("prettier_intermediate_{suffix}{input_ext}");
        let to_variant_filename = format!("prettier_intermediate_to_variant_{suffix}{input_ext}");
        let to_divergent_variant_filename =
            format!("prettier_intermediate_to_divergent_variant_{suffix}{input_ext}");
        let plain_path = fixture.path.join(&plain_filename);
        let to_variant_path = fixture.path.join(&to_variant_filename);
        let to_divergent_variant_path = fixture.path.join(&to_divergent_variant_filename);

        let (shape, formatted) = match classify_variant_chain(
            fixture,
            variant_name,
            &input,
            &variant_target_contents,
            &divergent_variant_target_contents,
        )
        .await
        {
            Ok(pair) => pair,
            Err(e) => {
                results.push(IntermediateOutput::File(
                    plain_filename,
                    FormattedResult::Failed(e),
                ));
                continue;
            }
        };

        match shape {
            ChainShape::NormalizesToInput | ChainShape::StableFirstPass => {
                remove_stale_intermediates(
                    &[
                        (&plain_path, &plain_filename),
                        (&to_variant_path, &to_variant_filename),
                        (&to_divergent_variant_path, &to_divergent_variant_filename),
                    ],
                    &mut results,
                );
            }
            ChainShape::UnstableNotConverging => {
                // Make the skip visible — silently doing nothing here masks the (intentional)
                // interaction between prettier_intermediate_* and audit_signature.txt.
                results.push(IntermediateOutput::Note(format!(
                    "- {}/{}: chain doesn't converge to input or any variant — captured by audit_signature.txt instead",
                    fixture.relative_path, variant_name
                )));
                remove_stale_intermediates(
                    &[
                        (&plain_path, &plain_filename),
                        (&to_variant_path, &to_variant_filename),
                        (&to_divergent_variant_path, &to_divergent_variant_filename),
                    ],
                    &mut results,
                );
            }
            ChainShape::FirstPassUnparseable(prettier_err) => {
                // Prettier produced syntactically invalid output on the first pass — a known
                // prettier bug (e.g., `{@const x = expr) /* c */}`). Document the bug in the
                // fixture's README and clean up any stale intermediate files. Not a failure:
                // there's no chain to record, and `fixtures:validate` is the authoritative
                // green-light for the fixture as a whole.
                results.push(IntermediateOutput::Note(format!(
                    "- {}/{}: prettier produced invalid syntax on first pass (prettier bug, see README): {prettier_err}",
                    fixture.relative_path, variant_name
                )));
                remove_stale_intermediates(
                    &[
                        (&plain_path, &plain_filename),
                        (&to_variant_path, &to_variant_filename),
                        (&to_divergent_variant_path, &to_divergent_variant_filename),
                    ],
                    &mut results,
                );
            }
            ChainShape::UnstableConvergesToInput => {
                write_intermediate_target(
                    &plain_path,
                    plain_filename,
                    &[
                        (&to_variant_path, &to_variant_filename),
                        (&to_divergent_variant_path, &to_divergent_variant_filename),
                    ],
                    &formatted,
                    &mut results,
                );
            }
            ChainShape::UnstableConvergesToVariant => {
                write_intermediate_target(
                    &to_variant_path,
                    to_variant_filename,
                    &[
                        (&plain_path, &plain_filename),
                        (&to_divergent_variant_path, &to_divergent_variant_filename),
                    ],
                    &formatted,
                    &mut results,
                );
            }
            ChainShape::UnstableConvergesToDivergentVariant => {
                write_intermediate_target(
                    &to_divergent_variant_path,
                    to_divergent_variant_filename,
                    &[
                        (&plain_path, &plain_filename),
                        (&to_variant_path, &to_variant_filename),
                    ],
                    &formatted,
                    &mut results,
                );
            }
        }
    }

    results
}

/// Classify prettier's chain from a single `unformatted_ours_*` variant.
///
/// Returns the chain shape and prettier's first-pass output (the bytes that go into a
/// `prettier_intermediate_*` file when the shape calls for one). Errors are surfaced as
/// `FormattedResult::Failed` by the caller, keyed to the `prettier_intermediate_*` filename.
async fn classify_variant_chain(
    fixture: &fixtures::Fixture,
    variant_name: &str,
    input: &str,
    variant_target_contents: &[String],
    divergent_variant_target_contents: &[String],
) -> Result<(ChainShape, String), String> {
    let variant_content = fixtures::read_file(&fixture.path.join(variant_name))?;
    let parser = fixture.input_type().prettier_parser();

    let formatted = crate::deno::run_prettier(&variant_content, parser)
        .await
        .map_err(|e| format!("Prettier error: {e}"))?;

    if formatted == *input {
        return Ok((ChainShape::NormalizesToInput, formatted));
    }

    // Second pass can fail when prettier's first-pass output is syntactically invalid
    // (a known prettier bug — e.g., `{@const x = expr) /* c */}` has an unmatched paren).
    // Don't propagate as a hard error; classify it so the caller can clean up and report
    // the prettier bug specifically.
    let second_pass = match crate::deno::run_prettier(&formatted, parser).await {
        Ok(s) => s,
        Err(e) => return Ok((ChainShape::FirstPassUnparseable(e.to_string()), formatted)),
    };

    let shape = if second_pass == formatted {
        ChainShape::StableFirstPass
    } else if second_pass == *input {
        ChainShape::UnstableConvergesToInput
    } else if variant_target_contents.contains(&second_pass) {
        ChainShape::UnstableConvergesToVariant
    } else if divergent_variant_target_contents.contains(&second_pass) {
        ChainShape::UnstableConvergesToDivergentVariant
    } else {
        ChainShape::UnstableNotConverging
    };
    Ok((shape, formatted))
}

/// Remove any intermediate files that exist; append a `Removed`/`Failed` result for each.
fn remove_stale_intermediates(
    paths: &[(&std::path::Path, &String)],
    results: &mut Vec<IntermediateOutput>,
) {
    for (path, name) in paths {
        if path.exists() {
            let result = match fixtures::delete_file_if_exists(path) {
                Ok(()) => FormattedResult::Removed,
                Err(e) => FormattedResult::Failed(e),
            };
            results.push(IntermediateOutput::File((*name).clone(), result));
        }
    }
}

/// Write `formatted` into `target_path` (Created/Updated/Unchanged) and clean up any stale
/// `opposite_paths` from a previous run with a different chain shape (there are now three
/// mutually exclusive intermediate kinds — plain / to_variant / to_divergent_variant).
fn write_intermediate_target(
    target_path: &std::path::Path,
    target_filename: String,
    opposite_paths: &[(&std::path::Path, &String)],
    formatted: &str,
    results: &mut Vec<IntermediateOutput>,
) {
    for (opposite_path, opposite_filename) in opposite_paths {
        if opposite_path.exists()
            && let Err(e) = fixtures::delete_file_if_exists(opposite_path)
        {
            results.push(IntermediateOutput::File(
                (*opposite_filename).clone(),
                FormattedResult::Failed(e),
            ));
            // Continue to write the correct target even on cleanup failure.
        }
    }

    let existing = fixtures::read_file(target_path).ok();
    let result = if existing.as_deref() == Some(formatted) {
        FormattedResult::Unchanged
    } else if existing.is_none() {
        match fixtures::write_file(target_path, formatted) {
            Ok(()) => FormattedResult::Created,
            Err(e) => FormattedResult::Failed(e),
        }
    } else {
        match fixtures::write_file(target_path, formatted) {
            Ok(()) => FormattedResult::Updated,
            Err(e) => FormattedResult::Failed(e),
        }
    };
    results.push(IntermediateOutput::File(target_filename, result));
}
