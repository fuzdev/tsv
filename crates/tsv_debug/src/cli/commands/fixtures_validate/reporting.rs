/// Helper to report validation errors with consistent formatting
pub(super) fn report_validation_error(
    title: &str,
    description: &str,
    files: &[String],
    fix_lines: &[&str],
) {
    eprintln!("\n❌ {} ({}):", title, files.len());
    eprintln!();
    eprintln!("{}", description);
    eprintln!();
    eprintln!("Affected files:");
    for item in files {
        eprintln!("  ✗ {}", item);
    }
    eprintln!();
    eprintln!("How to fix:");
    for line in fix_lines {
        eprintln!("{}", line);
    }
}

/// Helper to report normalization errors (unformatted_* or unformatted_ours_*)
fn report_normalization_error(
    file_pattern: &str,  // "unformatted_*" or "unformatted_ours_*"
    failures: &[String],
    additional_context: Option<&str>,
) {
    eprintln!(
        "\n❌ {}.svelte files don't normalize to input.svelte ({}):",
        file_pattern,
        failures.len()
    );
    eprintln!();
    eprintln!(
        "⚠️  CRITICAL: {}* files MUST normalize to input.svelte with our formatter.",
        file_pattern
    );
    if let Some(context) = additional_context {
        eprintln!("{}", context);
    } else {
        eprintln!(
            "This violates the normalization invariant - the core purpose of these variants."
        );
    }
    eprintln!();
    eprintln!("Affected files:");
    for item in failures {
        eprintln!("  ✗ {}", item);
    }
    eprintln!();
    eprintln!("How to fix:");
    eprintln!("  # Check what our formatter produces");
    eprintln!("  cargo run -p tsv_cli format FIXTURE/{}.svelte", file_pattern);
    eprintln!();
    eprintln!("  # Compare to input.svelte");
    eprintln!(
        "  diff <(cargo run -p tsv_cli format FIXTURE/{}.svelte) FIXTURE/input.svelte",
        file_pattern
    );
    eprintln!();
    eprintln!("If the formatter output is wrong, fix the formatter first!");
    eprintln!("If the variant is invalid, delete it or update it to normalize correctly.");
    eprintln!("See docs/fixtures.md for {}* validation procedures.", file_pattern);
}

pub(super) fn report_all_errors(results: &super::validations::ValidationResults) {
    if !results.structure_errors.is_empty() {
        eprintln!(
            "\n❌ Fixture structure errors ({}):",
            results.structure_errors.len()
        );
        eprintln!();
        eprintln!("These fixtures have invalid file structures that prevent testing.");
        eprintln!("Common issues: missing files, duplicate content, incorrect file patterns.");
        eprintln!();
        eprintln!("Affected fixtures:");
        for item in &results.structure_errors {
            eprintln!("  {}", item);
        }
        eprintln!();
        eprintln!("See docs/fixtures.md for fixture structure requirements.");
    }

    if !results.non_idempotent.is_empty() {
        report_validation_error(
            "input.svelte files not idempotent",
            "⚠️  CRITICAL: These input.svelte files don't format to themselves with our formatter.\n\
            This violates the CORE INVARIANT: input.svelte MUST be canonical and idempotent.\n\
            All fixtures MUST have input.svelte as the source of truth for correct formatting.",
            &results.non_idempotent,
            &[
                "  # Step 1: Check what our formatter produces",
                "  cargo run -p tsv_cli format FIXTURE/input.svelte",
                "",
                "  # Step 2: Compare the difference",
                "  cargo run -p tsv_debug compare FIXTURE/input.svelte",
                "",
                "  # Step 3: Fix by formatting input.svelte",
                "  cargo run -p tsv_cli format FIXTURE/input.svelte > /tmp/formatted.svelte",
                "  mv /tmp/formatted.svelte FIXTURE/input.svelte",
                "  deno task fixtures_update_parsed FIXTURE",
                "",
                "If the formatter output is wrong, fix the formatter first!",
                "See docs/fixtures.md for the idempotency invariant.",
            ],
        );
    }

    if !results.prettier_quirk_not_normalized.is_empty() {
        report_validation_error(
            "prettier_quirk_*.svelte files don't normalize to input.svelte",
            "⚠️  CRITICAL: prettier_quirk_* files should normalize to input.svelte with our formatter.\n\
            This is Rule 2: our formatter MUST normalize prettier quirks to canonical output.",
            &results.prettier_quirk_not_normalized,
            &[
                "  # Check what our formatter produces",
                "  cargo run -p tsv_cli format FIXTURE/prettier_quirk_*.svelte",
                "",
                "  # Compare to input.svelte",
                "  diff <(cargo run -p tsv_cli format FIXTURE/prettier_quirk_*.svelte) FIXTURE/input.svelte",
                "",
                "This is a formatter bug - fix the formatter to normalize quirks correctly!",
                "See docs/fixtures.md (Prettier Quirk System, Rule 2).",
            ],
        );
    }

    if !results.outdated_expected_ours.is_empty() {
        report_validation_error(
            "expected_ours.json files are outdated",
            "These expected_ours.json files don't match our parser's current output.",
            &results.outdated_expected_ours,
            &["deno task fixtures_update_parsed <fixture_name>"],
        );
    }

    if !results.outdated_expected.is_empty() {
        report_validation_error(
            "expected.json files are outdated",
            "These expected.json files don't match Svelte parser's current output.",
            &results.outdated_expected,
            &["deno task fixtures_update_parsed <fixture_name>"],
        );
    }

    if !results.outdated_expected_svelte.is_empty() {
        report_validation_error(
            "expected_svelte.json files are outdated",
            "These expected_svelte.json files don't match Svelte parser's current output.",
            &results.outdated_expected_svelte,
            &["deno task fixtures_update_parsed <fixture_name>"],
        );
    }

    if !results.outdated_output_prettier.is_empty() {
        report_validation_error(
            "output_prettier.svelte files are outdated",
            "These output_prettier.svelte files don't match prettier's current output.",
            &results.outdated_output_prettier,
            &["deno task fixtures_update_formatted"],
        );
    }

    if !results.unformatted_not_normalized.is_empty() {
        report_normalization_error(
            "unformatted_*",
            &results.unformatted_not_normalized,
            None, // Use default context message
        );
    }

    if !results.unformatted_ours_not_normalized.is_empty() {
        report_normalization_error(
            "unformatted_ours_*",
            &results.unformatted_ours_not_normalized,
            Some("These files exist in _prettier_divergence directories and are validated ONLY by our formatter."),
        );
    }

    if !results.prettier_quirk_not_idempotent.is_empty() {
        report_validation_error(
            "prettier_quirk_*.svelte files not preserved by prettier",
            "prettier_quirk_* files should be idempotent (prettier preserves them).\n\
            If prettier normalizes the file, it's not a quirk - use unformatted_* instead.",
            &results.prettier_quirk_not_idempotent,
            &[
                "  # Check if prettier preserves the file",
                "  cargo run -p tsv_debug format_prettier FIXTURE/prettier_quirk_*.svelte",
                "",
                "  # If prettier normalizes it:",
                "  mv FIXTURE/prettier_quirk_*.svelte FIXTURE/unformatted_*.svelte",
                "",
                "See docs/fixtures.md (Prettier Quirk System) for details.",
            ],
        );
    }

    if !results.invalid_inputs.is_empty() {
        eprintln!(
            "\n❌ input.svelte differs from prettier ({}):",
            results.invalid_inputs.len()
        );
        eprintln!();
        eprintln!("Running: deno task fixtures_validate");
        eprintln!("input.svelte must match prettier output (it's the baseline for tests).");
        eprintln!();
        eprintln!("Affected fixtures:");
        for (relative_path, _, _, _) in &results.invalid_inputs {
            eprintln!("  ✗ {}/input.svelte", relative_path);
        }

        eprintln!("\nPossible causes:");
        eprintln!("  1. Input file has formatting issues (extra spaces, incorrect indentation)");
        eprintln!("  2. Input was manually edited without running prettier");
        eprintln!("  3. Prettier config changed and fixtures need updating");

        eprintln!("\nHow to fix:");
        eprintln!("  # Step 1: Check what prettier does to your input");
        eprintln!("  cargo run -p tsv_debug compare FIXTURE/input.svelte");
        eprintln!();
        eprintln!("  # Step 2: Fix by either:");
        eprintln!("  a) Preserve original as unformatted variant:");
        eprintln!("     mv FIXTURE/input.svelte FIXTURE/unformatted_DESCRIPTIVE_NAME.svelte");
        eprintln!(
            "     cargo run -p tsv_debug format_prettier FIXTURE/unformatted_DESCRIPTIVE_NAME.svelte > FIXTURE/input.svelte"
        );
        eprintln!("     deno task fixtures_update_parsed CATEGORY");
        eprintln!();
        eprintln!("  b) Format input in place:");
        eprintln!(
            "     cargo run -p tsv_debug format_prettier FIXTURE/input.svelte > /tmp/formatted.svelte"
        );
        eprintln!("     mv /tmp/formatted.svelte FIXTURE/input.svelte");
        eprintln!("     deno task fixtures_update_parsed CATEGORY");
        eprintln!();
        eprintln!("See docs/fixtures.md for detailed troubleshooting procedures.");
    }

    if !results.unformatted_mismatch.is_empty() {
        eprintln!(
            "\n❌ unformatted_*.svelte variants don't normalize to input.svelte ({}):",
            results.unformatted_mismatch.len()
        );
        eprintln!();
        eprintln!("Running: deno task fixtures_validate");
        eprintln!(
            "When formatted with prettier, variants should match input.svelte (the canonical baseline)."
        );
        eprintln!();
        eprintln!("Affected fixtures:");
        for item in &results.unformatted_mismatch {
            eprintln!("  ✗ {}", item);
        }

        eprintln!("\nPossible causes:");
        eprintln!("  1. Content differs (e.g., text/comments don't match baseline)");
        eprintln!("  2. Variant tests something prettier doesn't normalize (invalid test)");
        eprintln!("  3. Baseline was updated but variant wasn't");

        eprintln!("\nHow to fix:");
        eprintln!("  # Step 1: Compare to find differences");
        eprintln!("  cat FIXTURE/input.svelte");
        eprintln!("  cat FIXTURE/unformatted_*.svelte");
        eprintln!();
        eprintln!("  # Step 2: Check what prettier does");
        eprintln!("  cargo run -p tsv_debug compare FIXTURE/unformatted_*.svelte");
        eprintln!();
        eprintln!("  # Step 3: Fix by either:");
        eprintln!(
            "  a) Update variant content to match baseline (keep same text/comments, only formatting differs)"
        );
        eprintln!("  b) Delete variant if it's no longer a valid normalization test");
        eprintln!();
        eprintln!("See docs/fixtures.md for unformatted_* validation procedures.");
    }

    if !results.unformatted_duplicates.is_empty() {
        eprintln!(
            "\n❌ Duplicate unformatted_*.svelte files detected ({} fixtures):",
            results.unformatted_duplicates.len()
        );
        eprintln!("The following fixtures have unformatted_*.svelte files with identical content:");
        eprintln!("Remove duplicate files - each unformatted_*.svelte variant should be unique.\n");

        for (fixture_path, variants, content) in &results.unformatted_duplicates {
            eprintln!("──────────────────────────────────────────────");
            eprintln!("❌ {}", fixture_path);
            eprintln!("  Duplicate files ({}):", variants.len());
            for variant in variants {
                eprintln!("    - {}", variant);
            }

            // Show first 3 lines of content
            let lines: Vec<&str> = content.lines().collect();
            let show_lines = 3.min(lines.len());
            if show_lines > 0 {
                eprintln!("\n  Content (first {} lines):", show_lines);
                for line in lines.iter().take(show_lines) {
                    eprintln!("    {}", line);
                }
                if lines.len() > show_lines {
                    eprintln!("    ... ({} more lines)", lines.len() - show_lines);
                }
            }

            eprintln!("\n  Suggestion: Remove all but one file, or differentiate their content");
            eprintln!();
        }
    }

    if !results.prettier_quirk_duplicates.is_empty() {
        eprintln!(
            "\n❌ Duplicate prettier_quirk_*.svelte files detected ({} fixtures):",
            results.prettier_quirk_duplicates.len()
        );
        eprintln!();
        eprintln!(
            "prettier_quirk_* files should test different prettier quirks (different whitespace/formatting)."
        );
        eprintln!("Each variant should produce different output when formatted with prettier.");
        eprintln!();
        eprintln!("Affected fixtures:");
        for (fixture_path, variants, _) in &results.prettier_quirk_duplicates {
            eprintln!("  ✗ {}", fixture_path);
            eprintln!("    Duplicate files ({}):", variants.len());
            for variant in variants {
                eprintln!("      - {}", variant);
            }
        }

        eprintln!("\nPossible causes:");
        eprintln!("  1. Files were accidentally copied without changing content");
        eprintln!("  2. Different quirk names but testing the same thing");
        eprintln!("  3. Files were meant to differ but don't actually differ");

        eprintln!("\nHow to fix:");
        eprintln!("  # Step 1: Compare the duplicate files");
        eprintln!("  diff FIXTURE/prettier_quirk_file1.svelte FIXTURE/prettier_quirk_file2.svelte");
        eprintln!();
        eprintln!("  # Step 2: Check what each tests");
        eprintln!("  cat FIXTURE/prettier_quirk_file1.svelte");
        eprintln!("  cat FIXTURE/prettier_quirk_file2.svelte");
        eprintln!();
        eprintln!("  # Step 3: Fix by either:");
        eprintln!(
            "  a) Differentiate them: edit one to test a different quirk (e.g., different spacing)"
        );
        eprintln!("  b) Remove the redundant one: rm FIXTURE/prettier_quirk_duplicate.svelte");
        eprintln!();
        eprintln!("See docs/fixtures.md (Prettier Quirk System) for details.");
    }

    if !results.redundant_unformatted.is_empty() {
        eprintln!(
            "\n❌ Redundant unformatted_*.svelte files detected ({} files):",
            results.redundant_unformatted.len()
        );
        eprintln!();
        eprintln!(
            "These unformatted_*.svelte files are identical to prettier_quirk_*.svelte files in the same fixture."
        );
        eprintln!(
            "prettier_quirk files already test both prettier quirks AND our formatter normalization,"
        );
        eprintln!("making identical unformatted files redundant.");
        eprintln!();
        eprintln!("Affected files:");
        for (fixture_path, unformatted_file, quirk_file) in &results.redundant_unformatted {
            eprintln!("  ✗ {}/{}", fixture_path, unformatted_file);
            eprintln!("    Identical to: {}", quirk_file);
        }

        eprintln!("\nRationale:");
        eprintln!("  - prettier_quirk_*.svelte files test:");
        eprintln!("    • Rule 1: prettier(file) == file (prettier preserves quirk)");
        eprintln!("    • Rule 2: tsv_format(file) == input.svelte (our formatter normalizes)");
        eprintln!(
            "  - Identical unformatted_*.svelte files test the exact same Rule 2 normalization"
        );
        eprintln!("  - Keeping both is duplicate test coverage with no added value");

        eprintln!("\nHow to fix:");
        eprintln!("  # Simply delete the redundant unformatted file:");
        eprintln!("  rm FIXTURE/unformatted_NAME.svelte");
        eprintln!();
        eprintln!("The prettier_quirk file already provides complete test coverage.");
    }

    if !results.duplicates.is_empty() {
        eprintln!(
            "\n❌ Duplicate input.svelte files detected ({} groups):",
            results.duplicates.len()
        );
        eprintln!("The following fixtures have identical input.svelte content:");
        eprintln!("Consider removing duplicates or differentiating them.\n");

        for (idx, (content, paths)) in results.duplicates.iter().enumerate() {
            eprintln!("──────────────────────────────────────────────");
            eprintln!("Duplicate group {} ({} fixtures):", idx + 1, paths.len());
            for path in paths {
                eprintln!("  - {}", path);
            }

            // Show first 3 lines of content
            let lines: Vec<&str> = content.lines().collect();
            let show_lines = 3.min(lines.len());
            if show_lines > 0 {
                eprintln!("\n  Content (first {} lines):", show_lines);
                for line in lines.iter().take(show_lines) {
                    eprintln!("    {}", line);
                }
                if lines.len() > show_lines {
                    eprintln!("    ... ({} more lines)", lines.len() - show_lines);
                }
            }
            eprintln!();
        }

        eprintln!("Suggestion: Remove duplicate fixtures or differentiate their inputs");
    }
}
