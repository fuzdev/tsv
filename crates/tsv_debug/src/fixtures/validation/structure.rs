//! Fixture structure validation: the S* rules for file layout,
//! expected-JSON patterns, and divergence-suffix naming.

use crate::fixtures::{
    AUDIT_SIGNATURE_FILENAME, Fixture, FixtureFiles, PRETTIER_NONCONVERGENT_FILENAME,
    PRETTIER_REJECTS_FILENAME, determine_required_suffix, has_prettier_divergence_suffix,
    has_svelte_divergence_suffix, read_file,
};

/// Validate fixture structure and conventions
///
/// Checks:
/// S1:  `input.svelte` exists for every fixture
/// S2:  `expected.json` OR (`expected_ours.json` + `expected_svelte.json`) exists
/// S3:  `expected.json` cannot coexist with `expected_*.json` files
/// S4:  `unformatted_*.svelte` variants differ from `input.svelte`
/// S5:  `prettier_variant_*` variants differ from input file
/// S6:  `output_prettier.svelte` differs from `input.svelte`
/// S7:  `unformatted_ours_*.svelte` variants differ from `input.svelte`
/// S8:  `_prettier_divergence` or `_svelte_prettier_divergence` suffix required when prettier divergence files exist
/// S9:  Prettier divergence dirs CANNOT have `unformatted_*.svelte` files
/// S10: `prettier_variant_*` files MUST be in prettier divergence dirs (enforced by S8)
/// S11: `unformatted_ours_*` files MUST be in prettier divergence dirs (enforced by S8)
/// S12: `_svelte_divergence` or `_svelte_prettier_divergence` suffix required when `expected_ours.json`/`expected_svelte.json` exist
/// S13: Svelte divergence dirs MUST have BOTH `expected_ours.json` AND `expected_svelte.json`
/// S14: `expected_ours.json` MUST be in svelte divergence dirs
/// S15: `expected_svelte.json` MUST be in svelte divergence dirs
/// S16: Svelte divergence dirs CANNOT have `expected.json`
/// S18: `prettier_nonconvergent.txt` requires a prettier divergence dir and CANNOT
///      coexist with prettier-claim files (`output_prettier.*`, `unformatted_*`,
///      `unformatted_prettier_*`, `prettier_variant_*`, `variant_*`, `divergent_variant_*`,
///      `prettier_intermediate_*`) — prettier has no fixed point, so no
///      prettier-anchored claim is expressible (`unformatted_ours_*` stays allowed:
///      it claims only OUR normalization)
/// S19: `prettier_rejects.txt` — same divergence-dir + claim-file rules as S18
///      (prettier throws, so no prettier-anchored claim is expressible), AND it is
///      mutually exclusive with `prettier_nonconvergent.txt`
/// D1:  README.md required for divergences
pub fn validate_fixture_structure(fixture: &Fixture, files: &FixtureFiles) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Get input type (validates that it's a known type)
    let input_type = fixture.input_type();
    let input_ext = input_type.extension();

    // Get directory name for suffix checks
    let dir_name = fixture_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_svelte_divergence_dir = has_svelte_divergence_suffix(dir_name);
    let is_prettier_divergence_dir = has_prettier_divergence_suffix(dir_name);

    // Check expected.json OR (expected_ours.json + expected_svelte.json) exists
    let expected_path = fixture.expected_path();
    let expected_ours_path = fixture.expected_ours_path();
    let expected_svelte_path = fixture.expected_svelte_path();

    let has_expected = expected_path.exists();
    let has_expected_ours = expected_ours_path.exists();
    let has_expected_svelte = expected_svelte_path.exists();

    // S12-S16: Svelte divergence suffix rules
    if has_expected_ours || has_expected_svelte {
        // S13: Both files must exist together
        if !has_expected_ours || !has_expected_svelte {
            return Err(
                "Found either expected_ours.json or expected_svelte.json but not both.\n\
                When using the expected_ours.json + expected_svelte.json pattern, both files must exist.\n\
                - expected_ours.json: Our parser's AST (source of truth for our tests)\n\
                - expected_svelte.json: Svelte's AST (documents the difference)\n\
                Run: deno task fixtures:update:parsed".to_string()
            );
        }

        // S14/S15: expected_ours.json and expected_svelte.json MUST be in svelte divergence dirs
        if !is_svelte_divergence_dir {
            // Determine correct suffix based on what files exist (must check prettier files too)
            let output_prettier_path = fixture.output_prettier_path();
            let suggested_suffix = determine_required_suffix(
                true, // has_expected_ours (we know this is true)
                true, // has_expected_svelte (we know this is true)
                output_prettier_path.exists(),
                !files.prettier_variant.is_empty(),
                !files.unformatted_ours.is_empty(),
                !files.variant.is_empty(),
                !files.divergent_variant.is_empty(),
            )
            .unwrap_or("_svelte_divergence");

            return Err(format!(
                "expected_ours.json and expected_svelte.json can only exist in directories with '{suggested_suffix}' suffix.\n\
                Found these files in directory '{dir_name}'.\n\
                Rename directory to '{dir_name}{suggested_suffix}'"
            ));
        }

        // S3/S16: expected.json cannot coexist with expected_*.json files
        if has_expected {
            return Err(
                "expected.json cannot coexist with expected_ours.json + expected_svelte.json.\n\
                Use either:\n\
                - expected.json (default: our parser matches Svelte)\n\
                - expected_ours.json + expected_svelte.json (our parser intentionally differs)\n\
                Remove expected.json"
                    .to_string(),
            );
        }

        // S17: expected_ours.json and expected_svelte.json must have different content
        // (if they're identical, there's no divergence and the pattern is pointless)
        let expected_ours_content = read_file(&expected_ours_path)?;
        let expected_svelte_content = read_file(&expected_svelte_path)?;
        if expected_ours_content == expected_svelte_content {
            return Err(
                "expected_ours.json and expected_svelte.json are identical.\n\
                The divergence pattern is only for when our parser differs from Svelte's.\n\
                If the ASTs match, use the standard expected.json pattern instead:\n\
                1. Remove _svelte_divergence suffix from directory name\n\
                2. Delete expected_ours.json and expected_svelte.json\n\
                3. Run: deno task fixtures:update:parsed"
                    .to_string(),
            );
        }
    } else if is_svelte_divergence_dir {
        // S12-rev: Svelte divergence dir MUST have expected_ours.json + expected_svelte.json
        return Err(format!(
            "Directory '{dir_name}' has '_svelte_divergence' suffix but lacks required files.\n\
            Svelte divergence directories MUST have both:\n\
            - expected_ours.json (our parser's AST)\n\
            - expected_svelte.json (Svelte parser's AST)\n\
            Either add these files or remove the '_svelte_divergence' suffix from the directory name."
        ));
    } else {
        // Standard pattern: expected.json (required)
        if !has_expected {
            return Err("Missing expected.json (required for parser tests).\n\
                Run: deno task fixtures:update:parsed"
                .to_string());
        }
    }

    let input_content = read_file(&fixture.input_path())?;

    // Check output_prettier file (if it exists)
    let output_prettier_path = fixture.output_prettier_path();
    let output_prettier_filename = fixture.output_prettier_filename();
    if output_prettier_path.exists() {
        let output_prettier_content = read_file(&output_prettier_path)?;

        // Must differ from input (no dead files)
        if output_prettier_content == input_content {
            return Err(format!(
                "{output_prettier_filename} is identical to {} (should be deleted if identical).\n\
                {output_prettier_filename} only exists when prettier formats {} differently.",
                fixture.input_file, fixture.input_file
            ));
        }

        // Note: output_prettier validation is handled by validation/phases.rs
        // (see validate_formatter_prettier function)
    }

    // audit_signature.txt is only meaningful alongside output_prettier.*
    // (it captures prettier's chain from there). Reject orphans.
    let audit_signature_path = fixture.audit_signature_path();
    if audit_signature_path.exists() && !output_prettier_path.exists() {
        return Err(format!(
            "{AUDIT_SIGNATURE_FILENAME} exists without {output_prettier_filename}.\n\
            The audit signature pins prettier's multi-pass chain anchored at output_prettier.*\n\
            and only applies when that file exists. Either:\n\
            - Generate {output_prettier_filename} (run: deno task fixtures:update:formatted), or\n\
            - Delete {AUDIT_SIGNATURE_FILENAME} if no longer relevant."
        ));
    }

    // S19: the two prettier no-oracle markers make incompatible claims and cannot
    // coexist — prettier either throws on the input or formats it forever without
    // a fixed point, never both.
    if files.prettier_nonconvergent && files.prettier_rejects {
        return Err(format!(
            "{PRETTIER_NONCONVERGENT_FILENAME} and {PRETTIER_REJECTS_FILENAME} cannot coexist.\n\
            They make incompatible claims: non-convergence means prettier formats the input\n\
            forever without a fixed point, while rejection means prettier throws on it.\n\
            Keep exactly one."
        ));
    }

    // S18/S19: a prettier no-oracle marker (prettier_nonconvergent.txt — no fixed
    // point; prettier_rejects.txt — prettier throws) contradicts every
    // prettier-anchored claim file. There is no canonical prettier output to record
    // (output_prettier.*), no stable form to preserve (prettier_variant_*/variant_*/
    // divergent_variant_*), and nothing prettier can normalize a variant to (unformatted_*/
    // unformatted_prettier_*/prettier_intermediate_*). unformatted_ours_* stays
    // allowed: it claims only OUR formatter's normalization.
    if files.prettier_nonconvergent || files.prettier_rejects {
        let marker = if files.prettier_rejects {
            PRETTIER_REJECTS_FILENAME
        } else {
            PRETTIER_NONCONVERGENT_FILENAME
        };
        let mut conflicts = Vec::new();
        if output_prettier_path.exists() {
            conflicts.push(output_prettier_filename.to_string());
        }
        for name in files
            .unformatted
            .iter()
            .chain(&files.unformatted_prettier)
            .chain(&files.prettier_variant)
            .chain(&files.variant)
            .chain(&files.divergent_variant)
            .chain(&files.prettier_intermediate)
            .chain(&files.prettier_intermediate_to_variant)
        {
            conflicts.push(name.clone());
        }
        if !conflicts.is_empty() {
            return Err(format!(
                "{marker} cannot coexist with prettier-claim files.\n\
                The marker asserts prettier cannot serve as an oracle on this input, so no\n\
                prettier-anchored claim (output_prettier.*, prettier_variant_*, variant_*,\n\
                divergent_variant_*, unformatted_*, unformatted_prettier_*, prettier_intermediate_*) is expressible.\n\
                Conflicting file(s): {}\n\
                Either delete the marker (if prettier can format input now) or remove the claim files.",
                conflicts.join(", ")
            ));
        }
    }

    // Check unformatted_* variants are not identical to input
    for variant_name in &files.unformatted {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        if variant_content == input_content {
            return Err(format!(
                "unformatted_*{input_ext} variant '{variant_name}' is identical to {} (should be different for testing normalization)",
                fixture.input_file
            ));
        }
    }

    // Check prettier_variant_* variants, collecting contents for the
    // variant_* cross-check below
    let mut prettier_variant_contents: Vec<(String, String)> = Vec::new();
    for variant_name in &files.prettier_variant {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Rule 3: Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "prettier_variant_*{input_ext} variant '{variant_name}' is identical to {} (should demonstrate a prettier variant)",
                fixture.input_file
            ));
        }
        prettier_variant_contents.push((variant_name.clone(), variant_content));
    }

    // Check variant_* variants, collecting contents for the divergent_variant_* cross-check
    let mut variant_contents: Vec<(String, String)> = Vec::new();
    for variant_name in &files.variant {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "variant_*{input_ext} variant '{variant_name}' is identical to {} (should be a distinct stable form)",
                fixture.input_file
            ));
        }

        // Must differ from all prettier_variant_* files
        for (pv_name, pv_content) in &prettier_variant_contents {
            if variant_content == *pv_content {
                return Err(format!(
                    "variant_*{input_ext} variant '{variant_name}' is identical to prettier_variant file '{pv_name}'.\n\
                    variant_* files must have distinct content from prettier_variant_* files.\n\
                    If our formatter normalizes this to input, use prettier_variant_* instead."
                ));
            }
        }
        variant_contents.push((variant_name.clone(), variant_content));
    }

    // Check divergent_variant_* forms (prettier keeps V, ours rewrites V to a third form)
    for tw_name in &files.divergent_variant {
        let tw_path = fixture_dir.join(tw_name);
        let tw_content = read_file(&tw_path)?;

        // Must differ from input (else ours would be collapsing to input — a
        // prettier_variant_*, not a divergent-variant form)
        if tw_content == input_content {
            return Err(format!(
                "divergent_variant_*{input_ext} form '{tw_name}' is identical to {} (a divergent_variant form must be a distinct stable form)",
                fixture.input_file
            ));
        }

        // Must differ from every prettier_variant_* and variant_* file — the three
        // stable forms (input, this V, ours(V)) are what make it divergent_variant; if V
        // coincides with a documented variant, one of the two is misclassified.
        for (pv_name, pv_content) in &prettier_variant_contents {
            if tw_content == *pv_content {
                return Err(format!(
                    "divergent_variant_*{input_ext} form '{tw_name}' is identical to prettier_variant file '{pv_name}'.\n\
                    A divergent_variant_* form must be distinct from prettier_variant_* files."
                ));
            }
        }
        for (v_name, v_content) in &variant_contents {
            if tw_content == *v_content {
                return Err(format!(
                    "divergent_variant_*{input_ext} form '{tw_name}' is identical to variant file '{v_name}'.\n\
                    A divergent_variant_* form must be distinct from variant_* files."
                ));
            }
        }
    }

    // S8: Check directory naming - prettier divergence suffix required when prettier validation should be skipped
    let has_prettier_variant_files = !files.prettier_variant.is_empty();
    let has_variant_files = !files.variant.is_empty();
    let has_divergent_variant_files = !files.divergent_variant.is_empty();
    let has_output_prettier = output_prettier_path.exists();

    // Divergence documentation: files that show what prettier produces
    // (unformatted_ours_* tests OUR formatter, doesn't document prettier's output).
    // divergent_variant_* pins a prettier-stable form our formatter rewrites, so it counts.
    let has_divergence_documentation = has_output_prettier
        || has_prettier_variant_files
        || has_variant_files
        || has_divergent_variant_files;

    let has_prettier_intermediate_files = !files.prettier_intermediate.is_empty();
    let has_prettier_intermediate_to_variant_files =
        !files.prettier_intermediate_to_variant.is_empty();

    // Prettier divergence suffix is required when ANY prettier divergence files exist
    let needs_prettier_divergence_suffix = has_output_prettier
        || has_prettier_variant_files
        || has_variant_files
        || has_divergent_variant_files
        || !files.unformatted_ours.is_empty()
        || !files.unformatted_prettier.is_empty()
        || has_prettier_intermediate_files
        || has_prettier_intermediate_to_variant_files
        || files.prettier_nonconvergent
        || files.prettier_rejects;

    if needs_prettier_divergence_suffix && !is_prettier_divergence_dir {
        let mut reasons = Vec::new();
        if has_output_prettier {
            reasons.push(output_prettier_filename.to_string());
        }
        if has_prettier_variant_files {
            reasons.push(format!(
                "{} prettier_variant_*{} file(s)",
                files.prettier_variant.len(),
                input_ext
            ));
        }
        if has_variant_files {
            reasons.push(format!(
                "{} variant_*{} file(s)",
                files.variant.len(),
                input_ext
            ));
        }
        if has_divergent_variant_files {
            reasons.push(format!(
                "{} divergent_variant_*{} file(s)",
                files.divergent_variant.len(),
                input_ext
            ));
        }
        if !files.unformatted_ours.is_empty() {
            reasons.push(format!(
                "{} unformatted_ours_*{} file(s)",
                files.unformatted_ours.len(),
                input_ext
            ));
        }
        if has_prettier_intermediate_files {
            reasons.push(format!(
                "{} prettier_intermediate_*{} file(s)",
                files.prettier_intermediate.len(),
                input_ext
            ));
        }
        if has_prettier_intermediate_to_variant_files {
            reasons.push(format!(
                "{} prettier_intermediate_to_variant_*{} file(s)",
                files.prettier_intermediate_to_variant.len(),
                input_ext
            ));
        }
        if !files.unformatted_prettier.is_empty() {
            reasons.push(format!(
                "{} unformatted_prettier_*{} file(s)",
                files.unformatted_prettier.len(),
                input_ext
            ));
        }
        if files.prettier_nonconvergent {
            reasons.push(PRETTIER_NONCONVERGENT_FILENAME.to_string());
        }
        if files.prettier_rejects {
            reasons.push(PRETTIER_REJECTS_FILENAME.to_string());
        }
        let reason = reasons.join(" and ");

        // Use determine_required_suffix to suggest the correct suffix
        let suggested_suffix = determine_required_suffix(
            has_expected_ours,
            has_expected_svelte,
            has_output_prettier,
            has_prettier_variant_files,
            !files.unformatted_ours.is_empty(),
            has_variant_files,
            has_divergent_variant_files,
        )
        .unwrap_or("_prettier_divergence");

        // Build specific suggestions based on what files are causing the issue
        let base_dir_name = dir_name
            .trim_end_matches("_svelte_divergence")
            .trim_end_matches("_prettier_divergence");

        let mut suggestions = vec![format!(
            "Rename directory to '{base_dir_name}{suggested_suffix}' (keeps prettier validation skipped)"
        )];

        // If the only issue is unformatted_ours_* files, offer the rename alternative
        if !files.unformatted_ours.is_empty() && !has_output_prettier && !has_prettier_variant_files
        {
            let file_renames: Vec<String> = files
                .unformatted_ours
                .iter()
                .map(|f| {
                    let new_name = f.replace("unformatted_ours_", "unformatted_");
                    format!("  {f} → {new_name}")
                })
                .collect();
            suggestions.push(format!(
                "Rename file(s) to enable prettier validation:\n{}",
                file_renames.join("\n")
            ));
        }

        return Err(format!(
            "Directory name must end with '{suggested_suffix}' when prettier validation should be skipped.\n\
            Found {reason} but directory '{dir_name}' lacks the suffix.\n\n\
            Options:\n\
            - {}\n\n\
            The 'unformatted_ours_*' naming skips prettier validation, which requires the divergence suffix.\n\
            Use 'unformatted_*' (without 'ours') if both formatters should validate the file.",
            suggestions.join("\n- ")
        ));
    }

    // S8-rev: Prettier divergence dir MUST document the divergence
    // Acceptable documentation:
    // - output_prettier.* (shows prettier formats input differently)
    // - prettier_variant_*.* (shows prettier's stable variants)
    // - variant_*.* (shows dual-stable forms)
    // - divergent_variant_*.* (shows a prettier-stable form our formatter rewrites to a third form)
    // - unformatted_ours_*.* + README.md (for normalization divergence where prettier(input)==input)
    let readme_path = fixture_dir.join("README.md");
    let has_readme = readme_path.exists();
    let has_unformatted_ours = !files.unformatted_ours.is_empty();

    // unformatted_ours_* + README is acceptable when prettier(input) == input
    // (divergence is about normalization behavior, not formatting the canonical input)
    let has_normalization_divergence_docs = has_unformatted_ours && has_readme;
    // A prettier no-oracle marker + README documents the divergence: either
    // "prettier has no fixed point" (F5) or "prettier rejects this input" (F6).
    // In both cases there is no prettier output file to record.
    let has_no_oracle_docs = (files.prettier_nonconvergent || files.prettier_rejects) && has_readme;
    let has_any_divergence_docs =
        has_divergence_documentation || has_normalization_divergence_docs || has_no_oracle_docs;

    if !has_any_divergence_docs && is_prettier_divergence_dir {
        // For pure _prettier_divergence dirs (not combined with _svelte)
        if !is_svelte_divergence_dir && dir_name.ends_with("_prettier_divergence") {
            return Err(format!(
                "Directory '{dir_name}' claims prettier divergence but lacks documentation.\n\
                The '_prettier_divergence' suffix means we differ from Prettier - that claim must be documented.\n\n\
                Required: Add one of these:\n\
                - {output_prettier_filename} (if prettier formats input differently)\n\
                - prettier_variant_*{input_ext} files (if prettier has stable variants our formatter normalizes)\n\
                - variant_*{input_ext} files (if both formatters keep the form stable)\n\
                - divergent_variant_*{input_ext} files (if prettier keeps the form but our formatter rewrites it to a third stable form)\n\
                - unformatted_ours_*{input_ext} files + README.md (if divergence is about normalization)\n\
                - {PRETTIER_NONCONVERGENT_FILENAME} + README.md (if prettier never reaches a fixed point)\n\
                - {PRETTIER_REJECTS_FILENAME} + README.md (if prettier throws on the input)"
            ));
        }
        // For combined _svelte_prettier_divergence dirs
        if is_svelte_divergence_dir && dir_name.ends_with("_svelte_prettier_divergence") {
            return Err(format!(
                "Directory '{dir_name}' claims both parser AND formatter divergence.\n\
                Parser divergence is documented (expected_ours.json + expected_svelte.json).\n\
                Formatter divergence is NOT documented.\n\n\
                Either:\n\
                - Add {output_prettier_filename} or prettier_variant_*{input_ext} or variant_*{input_ext} or divergent_variant_*{input_ext} to document formatter divergence, OR\n\
                - Add unformatted_ours_*{input_ext} + README.md for normalization divergence, OR\n\
                - Add {PRETTIER_NONCONVERGENT_FILENAME} + README.md if prettier never reaches a fixed point, OR\n\
                - Add {PRETTIER_REJECTS_FILENAME} + README.md if prettier throws on the input, OR\n\
                - Rename to '{}_svelte_divergence' if there's no formatter divergence",
                dir_name.trim_end_matches("_svelte_prettier_divergence")
            ));
        }
    }

    // S9: Prettier divergence directories with output_prettier.* CANNOT have unformatted_* files.
    // There prettier(input) != input, so prettier can never normalize a variant to input —
    // use unformatted_prettier_* (→ output_prettier) or unformatted_ours_* instead.
    // Without output_prettier.*, input is prettier-stable (F3), so unformatted_* is
    // meaningful and N3 validates it.
    if is_prettier_divergence_dir && has_output_prettier && !files.unformatted.is_empty() {
        let renamed_files = files
            .unformatted
            .iter()
            .map(|f| {
                format!(
                    "  {} → {}",
                    f,
                    f.replace("unformatted_", "unformatted_ours_")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        return Err(format!(
            "Directory with prettier divergence suffix and {output_prettier_filename} cannot have unformatted_*{input_ext} files.\n\
            Found {} unformatted_*{input_ext} file(s) in directory '{}'.\n\
            Prettier formats input differently here, so it can never normalize these variants to input.\n\
            \n\
            Choose one solution:\n\
            \n\
            Option 1: Rename files to unformatted_ours_*{input_ext} (if only our formatter normalizes them to input)\n\
            {}\n\
            \n\
            Option 2: Rename files to unformatted_prettier_*{input_ext} (if prettier normalizes them to {output_prettier_filename})",
            files.unformatted.len(),
            dir_name,
            renamed_files,
        ));
    }

    // Check unformatted_ours_* variants
    for variant_name in &files.unformatted_ours {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "unformatted_ours_*{input_ext} variant '{variant_name}' is identical to {} (should be different for testing normalization)",
                fixture.input_file
            ));
        }
    }

    // Check unformatted_prettier_* variants
    // These require output_prettier.* to exist (they normalize to prettier's output)
    if !files.unformatted_prettier.is_empty() && !has_output_prettier {
        return Err(format!(
            "unformatted_prettier_*{input_ext} files require {} to exist.\n\
            Found {} unformatted_prettier_*{input_ext} file(s) but no {}.\n\
            These files test that prettier normalizes to its canonical output.\n\
            Either:\n\
            - Add {} (run: deno task fixtures:update:formatted), OR\n\
            - Remove unformatted_prettier_*{input_ext} files",
            output_prettier_filename,
            files.unformatted_prettier.len(),
            output_prettier_filename,
            output_prettier_filename,
        ));
    }

    for variant_name in &files.unformatted_prettier {
        let variant_path = fixture_dir.join(variant_name);
        let variant_content = read_file(&variant_path)?;

        // Must differ from input
        if variant_content == input_content {
            return Err(format!(
                "unformatted_prettier_*{input_ext} variant '{variant_name}' is identical to {} (should be different for testing normalization)",
                fixture.input_file
            ));
        }
    }

    // Check if README.md should exist (D1 validation)
    let has_parser_divergence = has_expected_ours && has_expected_svelte;
    let has_formatter_divergence = output_prettier_path.exists();
    let has_prettier_variants = !files.prettier_variant.is_empty();
    let has_variants = !files.variant.is_empty();
    let has_divergent_variant = !files.divergent_variant.is_empty();
    let has_prettier_intermediate = !files.prettier_intermediate.is_empty();
    let has_prettier_intermediate_to_variant = !files.prettier_intermediate_to_variant.is_empty();

    let needs_readme = has_parser_divergence
        || has_formatter_divergence
        || has_prettier_variants
        || has_variants
        || has_divergent_variant
        || has_prettier_intermediate
        || has_prettier_intermediate_to_variant
        || files.prettier_nonconvergent
        || files.prettier_rejects;

    if needs_readme && !has_readme {
        let mut reasons = Vec::new();
        if has_parser_divergence {
            reasons.push(
                "- Parser divergence (expected_ours.json + expected_svelte.json)".to_string(),
            );
        }
        if has_formatter_divergence {
            reasons.push(format!(
                "- Formatter divergence ({output_prettier_filename})"
            ));
        }
        if has_prettier_variants {
            reasons.push(format!(
                "- Prettier variants (prettier_variant_*{input_ext})"
            ));
        }
        if has_variants {
            reasons.push(format!("- Prettier stable variants (variant_*{input_ext})"));
        }
        if has_divergent_variant {
            reasons.push(format!(
                "- Divergent-variant forms (divergent_variant_*{input_ext})"
            ));
        }
        if has_prettier_intermediate {
            reasons.push(format!(
                "- Prettier intermediate (prettier_intermediate_*{input_ext})"
            ));
        }
        if has_prettier_intermediate_to_variant {
            reasons.push(format!(
                "- Prettier intermediate to variant (prettier_intermediate_to_variant_*{input_ext})"
            ));
        }
        if files.prettier_nonconvergent {
            reasons.push(format!(
                "- Prettier non-convergence ({PRETTIER_NONCONVERGENT_FILENAME})"
            ));
        }
        if files.prettier_rejects {
            reasons.push(format!(
                "- Prettier rejection ({PRETTIER_REJECTS_FILENAME})"
            ));
        }

        return Err(format!(
            "README.md required when quirks/divergences exist.\n\
            This fixture has:\n\
            {}\n\
            README.md should document WHY we differ and provide context.\n\
            See docs/fixture_overview.md for README requirements.",
            reasons.join("\n")
        ));
    }

    Ok(())
}
