use crate::{deno, fixtures};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-validate command - validate all fixture files (input.svelte, formatted.svelte, unformatted_*.svelte)
pub struct FixturesValidateCommand;

impl Command for FixturesValidateCommand {
    fn name(&self) -> &str {
        "fixtures_validate"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let list_only = args.flag("list");

        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesValidateExecutable { list_only, filters }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_validate                           Validate all fixture files (CI)"
                .to_string(),
            "fixtures_validate --list                    List all fixtures".to_string(),
            "fixtures_validate <filter>...               Validate matching fixtures".to_string(),
        ]
    }
}

struct FixturesValidateExecutable {
    list_only: bool,
    filters: Vec<String>,
}

impl Executable for FixturesValidateExecutable {
    fn execute(&self) {
        let fixtures_dir = std::path::Path::new("tests/fixtures");

        if !fixtures_dir.exists() {
            eprintln!("Error: fixtures directory not found: tests/fixtures");
            std::process::exit(1);
        }

        let all_fixtures = match fixtures::walk_fixtures(fixtures_dir) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error walking fixtures: {}", e);
                std::process::exit(1);
            }
        };

        let total_count = all_fixtures.len();

        // Apply filters
        let fixtures: Vec<_> = all_fixtures
            .into_iter()
            .filter(|f| f.matches_filters(&self.filters))
            .collect();

        if fixtures.is_empty() {
            if self.filters.is_empty() {
                eprintln!("No fixtures found");
            } else {
                eprintln!("No fixtures found matching: {}", self.filters.join(" "));
            }
            std::process::exit(1);
        }

        if self.list_only {
            println!("Found fixtures:");
            for fixture in &fixtures {
                println!("  {} ({})", fixture.relative_path, fixture.input_file);
            }
            if self.filters.is_empty() {
                println!("\nTotal: {}", fixtures.len());
            } else {
                println!("\nMatched: {} of {} fixtures", fixtures.len(), total_count);
            }
            return;
        }

        let mut outdated = Vec::new();
        let mut incorrect = Vec::new();
        let mut structure_errors = Vec::new();
        let mut unformatted_mismatch = Vec::new();
        let mut invalid_inputs = Vec::new(); // Track inputs that don't match prettier
        let mut input_contents: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new(); // Track duplicate inputs
        let mut unformatted_duplicates = Vec::new(); // Track duplicate unformatted_*.svelte files
        let mut prettier_quirk_duplicates = Vec::new(); // Track duplicate prettier_quirk_*.svelte files
        let mut unformatted_formatted = Vec::new(); // Track formatted.* files that aren't prettier output
        let mut prettier_quirk_not_idempotent = Vec::new(); // Track prettier_quirk_* files that prettier doesn't preserve
        let mut checked = 0;
        let mut unformatted_checked = 0;
        let mut prettier_quirk_checked = 0;
        let mut failed_fixtures: std::collections::HashSet<String> =
            std::collections::HashSet::new(); // Track fixtures with any failure

        for fixture in &fixtures {
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

            let formatted_path = fixture.formatted_path();
            let output_prettier_path = fixture.output_prettier_path();

            // Skip formatted.svelte validation for fixtures where we intentionally differ from prettier
            // (implicit detection via output_prettier.svelte existence)
            if output_prettier_path.exists() {
                // For these fixtures, formatted.svelte (if exists) should contain OUR output
                // (not prettier's), so we skip prettier comparison
                continue;
            }

            // Check if formatted.svelte should exist
            if formatted == input {
                // Input is already formatted, formatted.svelte should NOT exist
                if formatted_path.exists() {
                    incorrect.push(format!(
                        "{}/formatted.svelte should not exist (input is already formatted)",
                        fixture.relative_path
                    ));
                    failed_fixtures.insert(fixture.relative_path.clone());
                }
            } else {
                // Input differs from prettier output, formatted.svelte SHOULD exist
                if !formatted_path.exists() {
                    // Store detailed info for better error message
                    invalid_inputs.push((
                        fixture.relative_path.clone(),
                        fixture.input_path(),
                        input.clone(),
                        formatted.clone(),
                    ));
                    failed_fixtures.insert(fixture.relative_path.clone());
                } else {
                    // Check if content matches
                    let existing = match fixtures::read_file(&formatted_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!(
                                "✗ Failed to read formatted file for {}: {}",
                                fixture.relative_path, e
                            );
                            continue;
                        }
                    };

                    if existing != formatted {
                        outdated.push(format!(
                            "{}/formatted.svelte is outdated",
                            fixture.relative_path
                        ));
                        failed_fixtures.insert(fixture.relative_path.clone());
                    } else {
                        // Verify formatted.svelte file itself is prettier output (idempotent check)
                        let formatted_reformatted = match deno::run_prettier(&existing, filepath) {
                            Ok(f) => f,
                            Err(e) => {
                                eprintln!(
                                    "✗ Prettier error for {}/formatted.svelte: {}",
                                    fixture.relative_path, e
                                );
                                continue;
                            }
                        };

                        if existing != formatted_reformatted {
                            unformatted_formatted.push(format!(
                                "{}/formatted.svelte is not prettier output (needs to be formatted)",
                                fixture.relative_path
                            ));
                            failed_fixtures.insert(fixture.relative_path.clone());
                        }
                    }
                }
            }

            // Determine baseline for unformatted_*.svelte comparison
            let baseline = if formatted_path.exists() {
                match fixtures::read_file(&formatted_path) {
                    Ok(s) => s,
                    Err(_) => formatted.clone(), // Fallback to prettier output
                }
            } else {
                input.clone()
            };

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
                // TODO: Implement when formatter is ready - requires calling tsv_cli format
                // For now, we just validate Rule 1 (prettier idempotence)
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

                // Skip prettier validation if prettier_quirk_* files exist
                // (prettier won't normalize these due to quirks)
                if has_prettier_quirks {
                    // TODO: Validate with our formatter instead (Rule 2)
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
                    unformatted_mismatch
                        .push(format!("{}/{}", fixture.relative_path, variant_name));
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
        }

        // Check for duplicate inputs (only when validating all fixtures)
        let duplicates: Vec<(String, Vec<String>)> = if self.filters.is_empty() {
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

        // Report results
        let has_errors = !outdated.is_empty()
            || !invalid_inputs.is_empty()
            || !incorrect.is_empty()
            || !structure_errors.is_empty()
            || !unformatted_mismatch.is_empty()
            || !duplicates.is_empty()
            || !unformatted_duplicates.is_empty()
            || !prettier_quirk_duplicates.is_empty()
            || !unformatted_formatted.is_empty()
            || !prettier_quirk_not_idempotent.is_empty();

        if !has_errors {
            if self.filters.is_empty() {
                println!(
                    "✓ All {} fixtures validated ({} unformatted_*.svelte, {} prettier_quirk_*.svelte)",
                    checked, unformatted_checked, prettier_quirk_checked
                );
            } else {
                println!(
                    "✓ Matched {} of {} fixtures pass validation ({} unformatted_*.svelte, {} prettier_quirk_*.svelte)",
                    checked, total_count, unformatted_checked, prettier_quirk_checked
                );
                println!(
                    "⚠️  Skipping cross-fixture duplicate detection (run without filters for full validation)"
                );
            }
            std::process::exit(0);
        }

        // TODO: Reorder error messages by severity:
        //   1. structure_errors (blocking - can't test without valid structure)
        //   2. invalid_inputs (input must be valid first)
        //   3. outdated/incorrect (formatted.svelte issues)
        //   4. unformatted_mismatch/duplicates (unformatted variant issues)
        //   5. prettier_quirk_not_idempotent/duplicates (quirk issues)
        //   6. duplicates (cross-fixture, lowest priority)

        if !structure_errors.is_empty() {
            eprintln!(
                "\n❌ Fixture structure errors ({}):",
                structure_errors.len()
            );
            eprintln!();
            eprintln!("These fixtures have invalid file structures that prevent testing.");
            eprintln!("Common issues: missing files, duplicate content, incorrect file patterns.");
            eprintln!();
            eprintln!("Affected fixtures:");
            for item in &structure_errors {
                eprintln!("  {}", item);
            }
            eprintln!();
            eprintln!("See docs/fixtures.md for fixture structure requirements.");
        }

        if !prettier_quirk_not_idempotent.is_empty() {
            eprintln!(
                "\n❌ prettier_quirk_*.svelte files not preserved by prettier ({}):",
                prettier_quirk_not_idempotent.len()
            );
            eprintln!();
            eprintln!("prettier_quirk_* files should be idempotent (prettier preserves them).");
            eprintln!(
                "If prettier normalizes the file, it's not a quirk - use unformatted_* instead."
            );
            eprintln!();
            eprintln!("Affected files:");
            for item in &prettier_quirk_not_idempotent {
                eprintln!("  ✗ {}", item);
            }
            eprintln!();
            eprintln!("How to fix:");
            eprintln!("  # Check if prettier preserves the file");
            eprintln!("  cargo run -p tsv_debug format_prettier FIXTURE/prettier_quirk_*.svelte");
            eprintln!();
            eprintln!("  # If prettier normalizes it:");
            eprintln!("  mv FIXTURE/prettier_quirk_*.svelte FIXTURE/unformatted_*.svelte");
            eprintln!();
            eprintln!("See docs/fixtures.md (Prettier Quirk System) for details.");
        }

        if !outdated.is_empty() {
            eprintln!("\n❌ Outdated formatted.svelte files ({}):", outdated.len());
            eprintln!();
            eprintln!(
                "These formatted.svelte files don't match what prettier would produce from input.svelte."
            );
            eprintln!(
                "This happens when input.svelte or prettier config changes after formatted.svelte was created."
            );
            eprintln!();
            eprintln!("Affected fixtures:");
            for item in &outdated {
                eprintln!("  ✗ {}", item);
            }
            eprintln!();
            eprintln!("How to fix:");
            eprintln!("  # Regenerate all formatted.svelte files");
            eprintln!("  deno task fixtures_update_formatted");
            eprintln!();
            eprintln!("See docs/fixtures.md for formatted.svelte usage guidelines.");
        }

        if !invalid_inputs.is_empty() {
            eprintln!(
                "\n❌ input.svelte differs from prettier ({}):",
                invalid_inputs.len()
            );
            eprintln!();
            eprintln!("Running: deno task fixtures_validate");
            eprintln!("input.svelte must match prettier output (it's the baseline for tests).");
            eprintln!();
            eprintln!("Affected fixtures:");
            for (relative_path, _, _, _) in &invalid_inputs {
                eprintln!("  ✗ {}/input.svelte", relative_path);
            }

            eprintln!("\nPossible causes:");
            eprintln!(
                "  1. Input file has formatting issues (extra spaces, incorrect indentation)"
            );
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
            eprintln!("     deno task fixtures_update_expected CATEGORY");
            eprintln!();
            eprintln!("  b) Format input in place:");
            eprintln!(
                "     cargo run -p tsv_debug format_prettier FIXTURE/input.svelte > /tmp/formatted.svelte"
            );
            eprintln!("     mv /tmp/formatted.svelte FIXTURE/input.svelte");
            eprintln!("     deno task fixtures_update_expected CATEGORY");
            eprintln!();
            eprintln!(
                "  c) Create formatted.svelte if input MUST be malformed (RARE - requires allowlist):"
            );
            eprintln!(
                "     cargo run -p tsv_debug format_prettier FIXTURE/input.svelte > FIXTURE/formatted.svelte"
            );
            eprintln!();
            eprintln!("See docs/fixtures.md for detailed troubleshooting procedures.");
        }

        if !incorrect.is_empty() {
            eprintln!(
                "\n❌ Unnecessary formatted.svelte files ({}):",
                incorrect.len()
            );
            eprintln!();
            eprintln!("These formatted.svelte files are identical to input.svelte.");
            eprintln!(
                "formatted.svelte should only exist when input.svelte is intentionally malformed."
            );
            eprintln!();
            eprintln!("Affected fixtures:");
            for item in &incorrect {
                eprintln!("  ✗ {}", item);
            }
            eprintln!();
            eprintln!("How to fix:");
            eprintln!("  # Remove the unnecessary formatted.svelte file");
            eprintln!("  rm FIXTURE/formatted.svelte");
            eprintln!();
            eprintln!("Or if input.svelte should differ from formatted.svelte:");
            eprintln!("  # Check if you need the formatted.svelte pattern");
            eprintln!("  # Most cases should use unformatted_*.svelte instead");
            eprintln!();
            eprintln!(
                "See docs/fixtures.md for formatted.svelte vs unformatted_*.svelte guidelines."
            );
        }

        if !unformatted_formatted.is_empty() {
            eprintln!(
                "\n❌ formatted.svelte files that don't match prettier output ({}):",
                unformatted_formatted.len()
            );
            eprintln!();
            eprintln!(
                "These formatted.svelte files have been manually edited or don't match what prettier would produce."
            );
            eprintln!(
                "All formatted.svelte files should be exact prettier output for consistency."
            );
            eprintln!();
            eprintln!("Affected fixtures:");
            for item in &unformatted_formatted {
                eprintln!("  ✗ {}", item);
            }
            eprintln!();
            eprintln!("How to fix:");
            eprintln!("  # Regenerate all formatted.svelte files");
            eprintln!("  deno task fixtures_update_formatted");
            eprintln!();
            eprintln!("See docs/fixtures.md for formatted.svelte guidelines.");
        }

        if !unformatted_mismatch.is_empty() {
            eprintln!(
                "\n❌ unformatted_*.svelte variants don't normalize to baseline ({}):",
                unformatted_mismatch.len()
            );
            eprintln!();
            eprintln!("Running: deno task fixtures_validate");
            eprintln!("When formatted with prettier, variants should match the baseline.");
            eprintln!("(baseline = formatted.svelte if exists, otherwise input.svelte)");
            eprintln!();
            eprintln!("Affected fixtures:");
            for item in &unformatted_mismatch {
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

        if !unformatted_duplicates.is_empty() {
            eprintln!(
                "\n❌ Duplicate unformatted_*.svelte files detected ({} fixtures):",
                unformatted_duplicates.len()
            );
            eprintln!(
                "The following fixtures have unformatted_*.svelte files with identical content:"
            );
            eprintln!(
                "Remove duplicate files - each unformatted_*.svelte variant should be unique.\n"
            );

            for (fixture_path, variants, content) in &unformatted_duplicates {
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

                eprintln!(
                    "\n  Suggestion: Remove all but one file, or differentiate their content"
                );
                eprintln!();
            }
        }

        if !prettier_quirk_duplicates.is_empty() {
            eprintln!(
                "\n❌ Duplicate prettier_quirk_*.svelte files detected ({} fixtures):",
                prettier_quirk_duplicates.len()
            );
            eprintln!();
            eprintln!(
                "prettier_quirk_* files should test different prettier quirks (different whitespace/formatting)."
            );
            eprintln!("Each variant should produce different output when formatted with prettier.");
            eprintln!();
            eprintln!("Affected fixtures:");
            for (fixture_path, variants, _) in &prettier_quirk_duplicates {
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
            eprintln!(
                "  diff FIXTURE/prettier_quirk_file1.svelte FIXTURE/prettier_quirk_file2.svelte"
            );
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

        if !duplicates.is_empty() {
            eprintln!(
                "\n❌ Duplicate input.svelte files detected ({} groups):",
                duplicates.len()
            );
            eprintln!("The following fixtures have identical input.svelte content:");
            eprintln!("Consider removing duplicates or differentiating them.\n");

            for (idx, (content, paths)) in duplicates.iter().enumerate() {
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

        // Show summary of failed fixtures
        let mut failed_list: Vec<String> = failed_fixtures.into_iter().collect();
        failed_list.sort();
        let passed = checked - failed_list.len();

        eprintln!("\n════════════════════");
        eprintln!();
        eprintln!("{} / {} fixtures failed:\n", failed_list.len(), checked);
        for fixture in &failed_list {
            eprintln!("  ✗ {}", fixture);
        }
        eprintln!();
        eprintln!(
            "Results Summary: {} passed, {} failed out of {} total",
            passed,
            failed_list.len(),
            checked
        );
        eprintln!();

        std::process::exit(1);
    }
}
