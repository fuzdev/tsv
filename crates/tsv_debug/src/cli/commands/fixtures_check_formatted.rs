use crate::{deno, fixtures};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-check-formatted command - verify formatted.svelte files are up to date (CI)
pub struct FixturesCheckFormattedCommand;

impl Command for FixturesCheckFormattedCommand {
    fn name(&self) -> &str {
        "fixtures_validate"
    }

    fn parse_args(&self, _args: &mut Args) -> Result<Box<dyn Executable>, String> {
        Ok(Box::new(FixturesCheckFormattedExecutable))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_validate                    Verify formatted.svelte files are up to date (CI)"
                .to_string(),
        ]
    }
}

struct FixturesCheckFormattedExecutable;

impl Executable for FixturesCheckFormattedExecutable {
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

        let mut outdated = Vec::new();
        let mut incorrect = Vec::new();
        let mut structure_errors = Vec::new();
        let mut unformatted_mismatch = Vec::new();
        let mut invalid_inputs = Vec::new(); // Track inputs that don't match prettier
        let mut checked = 0;
        let mut unformatted_checked = 0;

        for fixture in &all_fixtures {
            checked += 1;

            // Validate fixture structure
            if let Err(e) = fixtures::validate_fixture_structure(fixture) {
                structure_errors.push(format!("{}: {}", fixture.relative_path, e));
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

            // Check if formatted.svelte should exist
            if formatted == input {
                // Input is already formatted, formatted.svelte should NOT exist
                if formatted_path.exists() {
                    incorrect.push(format!(
                        "{}/formatted.svelte should not exist (input is already formatted)",
                        fixture.relative_path
                    ));
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

            // Check unformatted_*.svelte variants
            let unformatted_variants = fixtures::discover_unformatted_variants(&fixture.path);
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
                    unformatted_mismatch.push(format!(
                        "{}/{} does not normalize to baseline (formatted.svelte or input.svelte)",
                        fixture.relative_path, variant_name
                    ));
                }
            }
        }

        // Report results
        if outdated.is_empty()
            && invalid_inputs.is_empty()
            && incorrect.is_empty()
            && structure_errors.is_empty()
            && unformatted_mismatch.is_empty()
        {
            println!(
                "✓ All {} fixtures match Prettier formatting ({} unformatted_*.svelte variants checked)",
                checked, unformatted_checked
            );
            std::process::exit(0);
        }

        if !structure_errors.is_empty() {
            eprintln!(
                "\n❌ Fixture structure errors ({}):",
                structure_errors.len()
            );
            for item in &structure_errors {
                eprintln!("  {}", item);
            }
        }

        if !outdated.is_empty() {
            eprintln!("\n❌ Outdated formatted.svelte files ({}):", outdated.len());
            for item in &outdated {
                eprintln!("  {}", item);
            }
        }

        if !invalid_inputs.is_empty() {
            eprintln!(
                "\n❌ input.svelte differs from prettier ({}):",
                invalid_inputs.len()
            );
            eprintln!();
            eprintln!("Running: cargo run -p tsv_debug fixtures_validate");
            eprintln!("input.svelte must match prettier output (it's the baseline for tests).");
            eprintln!();
            eprintln!("Affected fixtures:");
            for (relative_path, _, _, _) in &invalid_inputs {
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
            eprintln!("     cargo run -p tsv_debug format_prettier FIXTURE/unformatted_DESCRIPTIVE_NAME.svelte > FIXTURE/input.svelte");
            eprintln!("     deno task fixtures_update_expected CATEGORY");
            eprintln!();
            eprintln!("  b) Format input in place:");
            eprintln!("     cargo run -p tsv_debug format_prettier FIXTURE/input.svelte > /tmp/formatted.svelte");
            eprintln!("     mv /tmp/formatted.svelte FIXTURE/input.svelte");
            eprintln!("     deno task fixtures_update_expected CATEGORY");
            eprintln!();
            eprintln!("  c) Create formatted.svelte if input MUST be malformed (RARE - requires allowlist):");
            eprintln!("     cargo run -p tsv_debug format_prettier FIXTURE/input.svelte > FIXTURE/formatted.svelte");
            eprintln!();
            eprintln!("See docs/fixtures.md for detailed troubleshooting procedures.");
        }

        if !incorrect.is_empty() {
            eprintln!(
                "\n❌ Unnecessary formatted.svelte files ({}):",
                incorrect.len()
            );
            for item in &incorrect {
                eprintln!("  {}", item);
            }
        }

        if !unformatted_mismatch.is_empty() {
            eprintln!(
                "\n❌ unformatted_*.svelte variants don't normalize to baseline ({}):",
                unformatted_mismatch.len()
            );
            eprintln!();
            eprintln!("Running: cargo run -p tsv_debug fixtures_validate");
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

        eprintln!("\nRun: deno task fixtures_update_formatted");
        std::process::exit(1);
    }
}
