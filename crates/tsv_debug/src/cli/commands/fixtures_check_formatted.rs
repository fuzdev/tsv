use crate::{deno, fixtures};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-check-formatted command - verify formatted.* files are up to date (CI)
pub struct FixturesCheckFormattedCommand;

impl Command for FixturesCheckFormattedCommand {
    fn name(&self) -> &str {
        "fixtures_check_formatted"
    }

    fn parse_args(&self, _args: &mut Args) -> Result<Box<dyn Executable>, String> {
        Ok(Box::new(FixturesCheckFormattedExecutable))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_check_formatted                    Verify formatted.* files are up to date (CI)"
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
        let mut missing = Vec::new();
        let mut incorrect = Vec::new();
        let mut checked = 0;

        for fixture in &all_fixtures {
            checked += 1;

            // Read input file
            let input = match fixtures::read_file(&fixture.input_path()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("✗ Failed to read {}: {}", fixture.relative_path, e);
                    continue;
                }
            };

            // Determine filepath for prettier
            let filepath = match fixture.file_type() {
                fixtures::FileType::Svelte => "temp.svelte",
                fixtures::FileType::TypeScript => "temp.ts",
                fixtures::FileType::Css => "temp.css",
                fixtures::FileType::Unknown => continue,
            };

            // Run prettier
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
                        "{}/formatted.* should not exist (input is already formatted)",
                        fixture.relative_path
                    ));
                }
            } else {
                // Input differs from prettier output, formatted.* SHOULD exist
                if !formatted_path.exists() {
                    missing.push(format!(
                        "{}/formatted.* is missing",
                        fixture.relative_path
                    ));
                } else {
                    // Check if content matches
                    let existing = match fixtures::read_file(&formatted_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("✗ Failed to read formatted file for {}: {}", fixture.relative_path, e);
                            continue;
                        }
                    };

                    if existing != formatted {
                        outdated.push(format!(
                            "{}/formatted.* is outdated",
                            fixture.relative_path
                        ));
                    }
                }
            }
        }

        // Report results
        if outdated.is_empty() && missing.is_empty() && incorrect.is_empty() {
            println!("✓ All {} formatted.* files are up to date", checked);
            std::process::exit(0);
        }

        if !outdated.is_empty() {
            eprintln!("\n❌ Outdated formatted.* files ({}):", outdated.len());
            for item in &outdated {
                eprintln!("  {}", item);
            }
        }

        if !missing.is_empty() {
            eprintln!("\n❌ Missing formatted.* files ({}):", missing.len());
            for item in &missing {
                eprintln!("  {}", item);
            }
        }

        if !incorrect.is_empty() {
            eprintln!("\n❌ Incorrect formatted.* files ({}):", incorrect.len());
            for item in &incorrect {
                eprintln!("  {}", item);
            }
        }

        eprintln!("\nRun: npm run fixtures_update_formatted");
        std::process::exit(1);
    }
}
