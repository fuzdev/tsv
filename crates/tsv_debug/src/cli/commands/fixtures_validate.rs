use crate::{deno, fixtures};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-validate command - validate all fixture files (input.*, formatted.*, unformatted_*)
pub struct FixturesValidateCommand;

impl Command for FixturesValidateCommand {
    fn name(&self) -> &str {
        "fixtures_validate"
    }

    fn parse_args(&self, _args: &mut Args) -> Result<Box<dyn Executable>, String> {
        Ok(Box::new(FixturesValidateExecutable))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_validate                           Validate all fixture files (CI)"
                .to_string(),
        ]
    }
}

struct FixturesValidateExecutable;

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

        let mut outdated = Vec::new();
        let mut incorrect = Vec::new();
        let mut structure_errors = Vec::new();
        let mut unformatted_mismatch = Vec::new();
        let mut invalid_inputs = Vec::new(); // Track inputs that don't match prettier
        let mut input_contents: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new(); // Track duplicate inputs
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

            // Track input content for duplicate detection
            input_contents
                .entry(input.clone())
                .or_insert_with(Vec::new)
                .push(fixture.relative_path.clone());

            // Determine filepath for prettier
            let filepath = match fixture.file_type() {
                fixtures::FileType::Svelte => "temp.svelte",
                fixtures::FileType::SvelteTypeScript => "temp.svelte.ts",
                fixtures::FileType::TypeScript => "temp.ts",
                fixtures::FileType::Css => "temp.css",
                fixtures::FileType::Unknown => continue,
            };

            // Run prettier on input
            let formatted = match deno::run_prettier(&input, filepath) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("✗ Prettier error for {}: {}", fixture.relative_path, e);
                    continue;
                }
            };

            let formatted_path = fixture.formatted_path();

            // Check if formatted.* should exist
            if formatted == input {
                // Input is already formatted, formatted.* should NOT exist
                if formatted_path.exists() {
                    incorrect.push(format!(
                        "{}/formatted.{} should not exist (input is already formatted)",
                        fixture.relative_path,
                        fixture.extension()
                    ));
                }
            } else {
                // Input differs from prettier output, formatted.* SHOULD exist
                if !formatted_path.exists() {
                    // Store detailed info for better error message
                    invalid_inputs.push((
                        fixture.relative_path.clone(),
                        fixture.input_path(),
                        fixture.extension().to_string(),
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
                            "{}/formatted.{} is outdated",
                            fixture.relative_path,
                            fixture.extension()
                        ));
                    }
                }
            }

            // Determine baseline for unformatted_* comparison
            let baseline = if formatted_path.exists() {
                match fixtures::read_file(&formatted_path) {
                    Ok(s) => s,
                    Err(_) => formatted.clone(), // Fallback to prettier output
                }
            } else {
                input.clone()
            };

            // Check unformatted_* variants
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
                        "{}/{} does not normalize to baseline (formatted.{} or input.{})",
                        fixture.relative_path,
                        variant_name,
                        fixture.extension(),
                        fixture.extension()
                    ));
                }
            }
        }

        // Check for duplicate inputs
        let mut duplicates: Vec<(String, Vec<String>)> = input_contents
            .iter()
            .filter(|(_, paths)| paths.len() > 1)
            .map(|(content, paths)| (content.clone(), paths.clone()))
            .collect();
        duplicates.sort_by(|a, b| a.1.len().cmp(&b.1.len()).reverse());

        // Report results
        if outdated.is_empty()
            && invalid_inputs.is_empty()
            && incorrect.is_empty()
            && structure_errors.is_empty()
            && unformatted_mismatch.is_empty()
            && duplicates.is_empty()
        {
            println!(
                "✓ All {} fixtures match Prettier formatting ({} unformatted_* variants checked)",
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
            eprintln!("\n❌ Outdated formatted.* files ({}):", outdated.len());
            for item in &outdated {
                eprintln!("  {}", item);
            }
        }

        if !invalid_inputs.is_empty() {
            eprintln!(
                "\n❌ Invalid input.* files - don't match prettier output ({}):",
                invalid_inputs.len()
            );
            eprintln!("These fixtures have input.* files that differ from prettier.");
            eprintln!("Fix by formatting the input, or create formatted.* if intentional (rare).\n");

            for (relative_path, input_path, extension, input_content, prettier_output) in &invalid_inputs {
                eprintln!("──────────────────────────────────────────────");
                eprintln!("❌ {}", relative_path);

                // Show first few lines of difference
                let input_lines: Vec<&str> = input_content.lines().collect();
                let prettier_lines: Vec<&str> = prettier_output.lines().collect();
                let show_lines = 3.min(input_lines.len()).min(prettier_lines.len());

                if show_lines > 0 {
                    eprintln!("\n  Input (first {} lines):", show_lines);
                    for line in input_lines.iter().take(show_lines) {
                        eprintln!("    {}", line);
                    }

                    eprintln!("\n  Prettier (expected first {} lines):", show_lines);
                    for line in prettier_lines.iter().take(show_lines) {
                        eprintln!("    {}", line);
                    }
                }

                eprintln!("\n  Fix (RECOMMENDED):");
                eprintln!("    cargo run -p tsv_debug format_prettier {} > /tmp/formatted.{}",
                    input_path.display(), extension);
                eprintln!("    mv /tmp/formatted.{} {}", extension, input_path.display());
                eprintln!("    deno task fixtures_update_expected -- {}",
                    relative_path.split('/').next().unwrap_or(relative_path));

                eprintln!("\n  Or create formatted.{} if malformed input is intentional (RARE):", extension);
                eprintln!("    cargo run -p tsv_debug format_prettier {} > tests/fixtures/{}/formatted.{}",
                    input_path.display(), relative_path, extension);
                eprintln!("    # See docs/fixtures.md");
                eprintln!("    # Requires allowlist for section reordering, parser robustness tests, etc.");
                eprintln!();
            }
        }

        if !incorrect.is_empty() {
            eprintln!("\n❌ Unnecessary formatted.* files ({}):", incorrect.len());
            for item in &incorrect {
                eprintln!("  {}", item);
            }
        }

        if !unformatted_mismatch.is_empty() {
            eprintln!(
                "\n❌ unformatted_* variants don't normalize to baseline ({}):",
                unformatted_mismatch.len()
            );
            for item in &unformatted_mismatch {
                eprintln!("  {}", item);
            }
            eprintln!(
                "\nNote: unformatted_* variants should format to the same output as formatted.* (or input.* if no formatted.* exists)"
            );
        }

        if !duplicates.is_empty() {
            eprintln!("\n❌ Duplicate input.* files detected ({} groups):", duplicates.len());
            eprintln!("The following fixtures have identical input.* content:");
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

        eprintln!("\nRun: deno task fixtures_update_formatted");
        std::process::exit(1);
    }
}
