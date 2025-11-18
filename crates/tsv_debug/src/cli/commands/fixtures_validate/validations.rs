use crate::{deno, fixtures};

pub(super) struct ValidationResults {
    pub structure_errors: Vec<String>,
    pub unformatted_mismatch: Vec<String>,
    pub invalid_inputs: Vec<(String, std::path::PathBuf, String, String)>,
    pub unformatted_duplicates: Vec<(String, Vec<String>, String)>,
    pub prettier_quirk_duplicates: Vec<(String, Vec<String>, String)>,
    pub prettier_quirk_not_idempotent: Vec<String>,
    pub non_idempotent: Vec<String>,
    pub prettier_quirk_not_normalized: Vec<String>,
    pub unformatted_not_normalized: Vec<String>,
    pub unformatted_ours_not_normalized: Vec<String>,
    pub outdated_expected_ours: Vec<String>,
    pub outdated_expected: Vec<String>,
    pub outdated_expected_svelte: Vec<String>,
    pub outdated_output_prettier: Vec<String>,
    pub redundant_unformatted: Vec<(String, String, String)>, // (fixture, unformatted_file, prettier_quirk_file)
    pub duplicates: Vec<(String, Vec<String>)>,
    pub checked: usize,
    pub unformatted_checked: usize,
    pub unformatted_ours_checked: usize,
    pub prettier_quirk_checked: usize,
    pub failed_fixtures: std::collections::HashSet<String>,
}

pub(super) fn run_validations(
    fixtures: &[fixtures::Fixture],
    filters: &[String],
) -> ValidationResults {
    let mut structure_errors = Vec::new();
    let mut unformatted_mismatch = Vec::new();
    let mut invalid_inputs = Vec::new(); // Track inputs that don't match prettier
    let mut input_contents: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new(); // Track duplicate inputs
    let mut unformatted_duplicates = Vec::new(); // Track duplicate unformatted_*.svelte files
    let mut prettier_quirk_duplicates = Vec::new(); // Track duplicate prettier_quirk_*.svelte files
    let mut prettier_quirk_not_idempotent = Vec::new(); // Track prettier_quirk_* files that prettier doesn't preserve
    let mut non_idempotent = Vec::new(); // Track input.svelte files that don't format to themselves (F1)
    let mut prettier_quirk_not_normalized = Vec::new(); // Track prettier_quirk_* files that don't normalize to input.svelte (N2)
    let mut unformatted_not_normalized = Vec::new(); // Track unformatted_* files that don't normalize to input.svelte (N4)
    let mut unformatted_ours_not_normalized = Vec::new(); // Track unformatted_ours_* files that don't normalize to input.svelte (N4)
    let mut outdated_expected_ours = Vec::new(); // Track expected_ours.json files that don't match our parser output (P2)
    let mut outdated_expected = Vec::new(); // Track expected.json files that don't match Svelte parser output (P1)
    let mut outdated_expected_svelte = Vec::new(); // Track expected_svelte.json files that don't match Svelte parser output (P3)
    let mut outdated_output_prettier = Vec::new(); // Track output_prettier.svelte files that don't match prettier output (F2)
    let mut redundant_unformatted = Vec::new(); // Track unformatted_* files identical to prettier_quirk_* files
    let mut checked = 0;
    let mut unformatted_checked = 0;
    let mut unformatted_ours_checked = 0;
    let mut prettier_quirk_checked = 0;
    let mut failed_fixtures: std::collections::HashSet<String> = std::collections::HashSet::new(); // Track fixtures with any failure

    for fixture in fixtures {
        checked += 1;

        // Validate fixture structure
        if let Err(e) = fixtures::validate_fixture_structure(fixture) {
            structure_errors.push(format!("{}: {}", fixture.relative_path, e));
            failed_fixtures.insert(fixture.relative_path.clone());
            continue; // Skip further checks if structure is invalid
        }

        // Read input file
        let input = match fixtures::read_file(&fixture.input_path()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("✗ Failed to read {}: {}", fixture.relative_path, e);
                continue;
            }
        };

        // Track input content for duplicate detection
        input_contents
            .entry(input.clone())
            .or_default()
            .push(fixture.relative_path.clone());

        // F1: Check that input.svelte formats to itself with our formatter (idempotency)
        // This is the MOST CRITICAL invariant!
        let filepath = "temp.svelte";
        match fixtures::format_with_our_formatter(&input, filepath) {
            Ok(formatted) => {
                if formatted != input {
                    non_idempotent.push(format!("{}/input.svelte", fixture.relative_path));
                    failed_fixtures.insert(fixture.relative_path.clone());
                }
            }
            Err(e) => {
                eprintln!("✗ Format error for {}: {}", fixture.relative_path, e);
                // Don't mark as non-idempotent if formatter crashes - that's a different issue
                continue;
            }
        }

        // P2: Check expected_ours.json matches our parser output
        let expected_ours_path = fixture.expected_ours_path();
        if expected_ours_path.exists() {
            match fixtures::read_json_file(&expected_ours_path) {
                Ok(expected_ast) => match fixtures::parse_with_our_parser(&input, filepath) {
                    Ok(actual_ast) => {
                        if expected_ast != actual_ast {
                            outdated_expected_ours
                                .push(format!("{}/expected_ours.json", fixture.relative_path));
                            failed_fixtures.insert(fixture.relative_path.clone());
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Parser error for {}: {}", fixture.relative_path, e);
                        continue;
                    }
                },
                Err(e) => {
                    eprintln!(
                        "✗ Failed to read expected_ours.json for {}: {}",
                        fixture.relative_path, e
                    );
                    continue;
                }
            }
        }

        // P1 & P3: Parse input with Svelte's parser for expected.json and expected_svelte.json validation
        // Only parse once and use for both checks (optimization)
        let expected_path = fixture.expected_path();
        let expected_svelte_path = fixture.expected_svelte_path();

        // Check if expected_svelte.json signals expected parse failure
        let expected_svelte_failure = if expected_svelte_path.exists() {
            match fixtures::read_json_file(&expected_svelte_path) {
                Ok(json) => {
                    // {"error": "..."} signals expected parse failure
                    if let Some(obj) = json.as_object() {
                        if let Some(error_value) = obj.get("error") {
                            // Validate canonical error format
                            if let Some(error_str) = error_value.as_str() {
                                if error_str != fixtures::EXPECTED_SVELTE_ERROR_MARKER {
                                    outdated_expected_svelte.push(format!(
                                            "{}/expected_svelte.json (non-standard error: expected {{\"error\": \"{}\"}})",
                                            fixture.relative_path,
                                            fixtures::EXPECTED_SVELTE_ERROR_MARKER
                                        ));
                                    failed_fixtures.insert(fixture.relative_path.clone());
                                }
                            } else {
                                outdated_expected_svelte.push(format!(
                                    "{}/expected_svelte.json (error field must be a string)",
                                    fixture.relative_path
                                ));
                                failed_fixtures.insert(fixture.relative_path.clone());
                            }
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                Err(_) => false,
            }
        } else {
            false
        };

        if expected_path.exists() || expected_svelte_path.exists() {
            let svelte_ast_result = deno::parse_svelte(&input);

            match svelte_ast_result {
                Ok(svelte_ast_str) => {
                    // If we expected Svelte to fail but it succeeded, that's an error
                    if expected_svelte_failure {
                        outdated_expected_svelte.push(format!(
                            "{}/expected_svelte.json (expected parse failure, got success)",
                            fixture.relative_path
                        ));
                        failed_fixtures.insert(fixture.relative_path.clone());
                    }

                    // Parse the JSON string to serde_json::Value for comparison
                    let svelte_ast: serde_json::Value = match serde_json::from_str(&svelte_ast_str)
                    {
                        Ok(ast) => ast,
                        Err(e) => {
                            eprintln!(
                                "✗ Failed to parse Svelte AST JSON for {}: {}",
                                fixture.relative_path, e
                            );
                            continue;
                        }
                    };

                    // P1: Check expected.json
                    if expected_path.exists() {
                        match fixtures::read_json_file(&expected_path) {
                            Ok(expected_ast) => {
                                if expected_ast != svelte_ast {
                                    outdated_expected
                                        .push(format!("{}/expected.json", fixture.relative_path));
                                    failed_fixtures.insert(fixture.relative_path.clone());
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "✗ Failed to read expected.json for {}: {}",
                                    fixture.relative_path, e
                                );
                                continue;
                            }
                        }
                    }

                    // P3: Check expected_svelte.json (only if not empty/failure marker)
                    if expected_svelte_path.exists() && !expected_svelte_failure {
                        match fixtures::read_json_file(&expected_svelte_path) {
                            Ok(expected_svelte_ast) => {
                                if expected_svelte_ast != svelte_ast {
                                    outdated_expected_svelte.push(format!(
                                        "{}/expected_svelte.json",
                                        fixture.relative_path
                                    ));
                                    failed_fixtures.insert(fixture.relative_path.clone());
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "✗ Failed to read expected_svelte.json for {}: {}",
                                    fixture.relative_path, e
                                );
                                continue;
                            }
                        }
                    }
                }
                Err(_e) => {
                    // Svelte parse failed
                    if expected_svelte_failure {
                        // This is expected! (empty expected_svelte.json signals expected failure)
                        // Don't print error, don't mark as failed
                    } else if expected_path.exists()
                        || (expected_svelte_path.exists() && !expected_svelte_failure)
                    {
                        // We expected parse to succeed, but it failed - that's an error
                        eprintln!(
                            "✗ Svelte parser error for {} (unexpected failure): {}",
                            fixture.relative_path, _e
                        );
                        failed_fixtures.insert(fixture.relative_path.clone());
                        continue;
                    }
                    // else: no expected files, so parse failure is fine (we're not checking Svelte)
                }
            }
        }

        // Determine filepath for prettier (always temp.svelte)
        let filepath = "temp.svelte";

        // Run prettier on input
        let formatted = match deno::run_prettier(&input, filepath) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("✗ Prettier error for {}: {}", fixture.relative_path, e);
                continue;
            }
        };

        let output_prettier_path = fixture.output_prettier_path();

        // F2: Check output_prettier.svelte matches prettier output
        if output_prettier_path.exists() {
            match fixtures::read_file(&output_prettier_path) {
                Ok(expected_prettier) => {
                    if expected_prettier != formatted {
                        outdated_output_prettier
                            .push(format!("{}/output_prettier.svelte", fixture.relative_path));
                        failed_fixtures.insert(fixture.relative_path.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "✗ Failed to read output_prettier.svelte for {}: {}",
                        fixture.relative_path, e
                    );
                }
            }
            // We intentionally differ from prettier, skip prettier baseline check
            // Continue to validate unformatted_* and prettier_quirk_* files
        } else if formatted != input {
            // No output_prettier.svelte, but input differs from prettier
            // This violates the invariant: input.svelte must be canonical (match prettier)
            invalid_inputs.push((
                fixture.relative_path.clone(),
                fixture.input_path(),
                input.clone(),
                formatted.clone(),
            ));
            failed_fixtures.insert(fixture.relative_path.clone());
            // Skip further validation for this fixture
            continue;
        }

        // Baseline for unformatted_*.svelte comparison is always input.svelte
        // (since input.svelte is always canonical)
        let baseline = input.clone();

        // Validate prettier_quirk_* variants (Rules 1 & 2 from docs/fixtures.md)
        // Rule 3 (quirk_file != input.svelte) is validated in validate_fixture_structure()
        let prettier_quirk_variants = fixtures::discover_prettier_quirk_variants(&fixture.path);
        let has_prettier_quirks = !prettier_quirk_variants.is_empty();
        let mut quirk_contents: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for quirk_name in &prettier_quirk_variants {
            prettier_quirk_checked += 1;
            let quirk_path = fixture.path.join(quirk_name);

            // Read quirk variant
            let quirk_content = match fixtures::read_file(&quirk_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "✗ Failed to read {}/{}: {}",
                        fixture.relative_path, quirk_name, e
                    );
                    continue;
                }
            };

            // Track content for duplicate detection
            quirk_contents
                .entry(quirk_content.clone())
                .or_default()
                .push(quirk_name.clone());

            // Rule 1: prettier(quirk_file) == quirk_file (prettier preserves quirk)
            let quirk_formatted = match deno::run_prettier(&quirk_content, filepath) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!(
                        "✗ Prettier error for {}/{}: {}",
                        fixture.relative_path, quirk_name, e
                    );
                    continue;
                }
            };

            if quirk_formatted != quirk_content {
                prettier_quirk_not_idempotent
                    .push(format!("{}/{}", fixture.relative_path, quirk_name));
                failed_fixtures.insert(fixture.relative_path.clone());
            }

            // Rule 2: tsv_format(quirk_file) == input.svelte (our formatter normalizes)
            match fixtures::format_with_our_formatter(&quirk_content, filepath) {
                Ok(formatted) => {
                    if formatted != input {
                        prettier_quirk_not_normalized
                            .push(format!("{}/{}", fixture.relative_path, quirk_name));
                        failed_fixtures.insert(fixture.relative_path.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "✗ Format error for {}/{}: {}",
                        fixture.relative_path, quirk_name, e
                    );
                    // Don't mark as not_normalized if formatter crashes
                    continue;
                }
            }
        }

        // Check for duplicate prettier_quirk_*.svelte files within this fixture
        for (content, variants) in &quirk_contents {
            if variants.len() > 1 {
                prettier_quirk_duplicates.push((
                    fixture.relative_path.clone(),
                    variants.clone(),
                    content.clone(),
                ));
                failed_fixtures.insert(fixture.relative_path.clone());
            }
        }

        // Check unformatted_*.svelte variants
        let unformatted_variants = fixtures::discover_unformatted_variants(&fixture.path);
        let mut variant_contents: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for variant_name in unformatted_variants {
            unformatted_checked += 1;
            let variant_path = fixture.path.join(&variant_name);

            // Read unformatted variant
            let variant_content = match fixtures::read_file(&variant_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "✗ Failed to read {}/{}: {}",
                        fixture.relative_path, variant_name, e
                    );
                    continue;
                }
            };

            // Track content for duplicate detection
            variant_contents
                .entry(variant_content.clone())
                .or_default()
                .push(variant_name.clone());

            // N4: Validate that unformatted_* normalizes to input.svelte with our formatter
            match fixtures::format_with_our_formatter(&variant_content, filepath) {
                Ok(formatted) => {
                    if formatted != input {
                        unformatted_not_normalized
                            .push(format!("{}/{}", fixture.relative_path, variant_name));
                        failed_fixtures.insert(fixture.relative_path.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "✗ Format error for {}/{}: {}",
                        fixture.relative_path, variant_name, e
                    );
                    // Don't skip prettier validation if our formatter crashes
                    // continue to prettier check below
                }
            }

            // Skip prettier validation if prettier_quirk_* files exist
            // (prettier won't normalize these due to quirks)
            if has_prettier_quirks {
                continue;
            }

            // Run prettier on variant
            let variant_formatted = match deno::run_prettier(&variant_content, filepath) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!(
                        "✗ Prettier error for {}/{}: {}",
                        fixture.relative_path, variant_name, e
                    );
                    continue;
                }
            };

            // Compare to baseline
            if variant_formatted != baseline {
                unformatted_mismatch.push(format!("{}/{}", fixture.relative_path, variant_name));
                failed_fixtures.insert(fixture.relative_path.clone());
            }
        }

        // Check for duplicate unformatted_*.svelte files within this fixture
        for (content, variants) in &variant_contents {
            if variants.len() > 1 {
                unformatted_duplicates.push((
                    fixture.relative_path.clone(),
                    variants.clone(),
                    content.clone(),
                ));
                failed_fixtures.insert(fixture.relative_path.clone());
            }
        }

        // Check for redundant unformatted_*.svelte files (identical to prettier_quirk_*.svelte)
        // This is only relevant when prettier_quirk files exist
        if !quirk_contents.is_empty() {
            for (unformatted_content, unformatted_files) in &variant_contents {
                // Check if this content matches any prettier_quirk file
                for (quirk_content, quirk_files) in &quirk_contents {
                    if unformatted_content == quirk_content {
                        // Found redundancy - unformatted file is identical to prettier_quirk file
                        for unformatted_file in unformatted_files {
                            // Report the first quirk file as the one that makes this redundant
                            let quirk_file = quirk_files[0].clone();
                            redundant_unformatted.push((
                                fixture.relative_path.clone(),
                                unformatted_file.clone(),
                                quirk_file,
                            ));
                            failed_fixtures.insert(fixture.relative_path.clone());
                        }
                    }
                }
            }
        }

        // Check unformatted_ours_*.svelte variants (only in _prettier_divergence directories)
        let unformatted_ours_variants = fixtures::discover_unformatted_ours_variants(&fixture.path);
        for variant_name in unformatted_ours_variants {
            unformatted_ours_checked += 1;
            let variant_path = fixture.path.join(&variant_name);

            // Read unformatted_ours variant
            let variant_content = match fixtures::read_file(&variant_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "✗ Failed to read {}/{}: {}",
                        fixture.relative_path, variant_name, e
                    );
                    continue;
                }
            };

            // N4: Validate that unformatted_ours_* normalizes to input.svelte with our formatter
            // NOTE: No prettier validation - these files are ONLY validated by our formatter
            match fixtures::format_with_our_formatter(&variant_content, filepath) {
                Ok(formatted) => {
                    if formatted != input {
                        unformatted_ours_not_normalized
                            .push(format!("{}/{}", fixture.relative_path, variant_name));
                        failed_fixtures.insert(fixture.relative_path.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "✗ Format error for {}/{}: {}",
                        fixture.relative_path, variant_name, e
                    );
                }
            }
        }
    }

    // Check for duplicate inputs (only when validating all fixtures)
    let duplicates: Vec<(String, Vec<String>)> = if filters.is_empty() {
        let mut dups: Vec<(String, Vec<String>)> = input_contents
            .iter()
            .filter(|(_, paths)| paths.len() > 1)
            .map(|(content, paths)| (content.clone(), paths.clone()))
            .collect();
        dups.sort_by(|a, b| a.1.len().cmp(&b.1.len()).reverse());
        dups
    } else {
        Vec::new()
    };

    ValidationResults {
        structure_errors,
        unformatted_mismatch,
        invalid_inputs,
        unformatted_duplicates,
        prettier_quirk_duplicates,
        prettier_quirk_not_idempotent,
        non_idempotent,
        prettier_quirk_not_normalized,
        unformatted_not_normalized,
        unformatted_ours_not_normalized,
        outdated_expected_ours,
        outdated_expected,
        outdated_expected_svelte,
        outdated_output_prettier,
        redundant_unformatted,
        duplicates,
        checked,
        unformatted_checked,
        unformatted_ours_checked,
        prettier_quirk_checked,
        failed_fixtures,
    }
}
