use crate::{deno, fixtures};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-update-formatted command - regenerate formatted.* files
pub struct FixturesUpdateFormattedCommand;

impl Command for FixturesUpdateFormattedCommand {
    fn name(&self) -> &str {
        "fixtures_update_formatted"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesUpdateFormattedExecutable { filters }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_update_formatted                   Regenerate all formatted.* files".to_string(),
            "fixtures_update_formatted <filter>...       Regenerate matching fixtures".to_string(),
        ]
    }
}

struct FixturesUpdateFormattedExecutable {
    filters: Vec<String>,
}

impl Executable for FixturesUpdateFormattedExecutable {
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

        let mut created = 0;
        let mut updated = 0;
        let mut removed = 0;
        let mut unchanged = 0;
        let mut failed = 0;

        for fixture in &fixtures {
            match update_formatted_file(fixture) {
                FormattedResult::Created => {
                    println!("✓ Created {}/formatted.*", fixture.relative_path);
                    created += 1;
                }
                FormattedResult::Updated => {
                    println!("✓ Updated {}/formatted.*", fixture.relative_path);
                    updated += 1;
                }
                FormattedResult::Removed => {
                    println!("✓ Removed {}/formatted.* (identical to input)", fixture.relative_path);
                    removed += 1;
                }
                FormattedResult::Unchanged => {
                    println!("- {}/formatted.* is up to date", fixture.relative_path);
                    unchanged += 1;
                }
                FormattedResult::NotNeeded => {
                    // Input is already formatted, no formatted.* file needed
                    unchanged += 1;
                }
                FormattedResult::Failed(err) => {
                    eprintln!("✗ Failed to process {}: {}", fixture.relative_path, err);
                    failed += 1;
                }
            }
        }

        println!(
            "\nSummary: {} created, {} updated, {} removed, {} unchanged, {} failed (total: {})",
            created, updated, removed, unchanged, failed, fixtures.len()
        );

        if created > 0 || updated > 0 || removed > 0 {
            println!("⚠️  Updated source of truth files (formatted.*)");
        }

        if failed > 0 {
            std::process::exit(1);
        }
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

fn update_formatted_file(fixture: &fixtures::Fixture) -> FormattedResult {
    // Read input file
    let input = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => return FormattedResult::Failed(e),
    };

    // Determine filepath for prettier
    let filepath = match fixture.file_type() {
        fixtures::FileType::Svelte => "temp.svelte",
        fixtures::FileType::TypeScript => "temp.ts",
        fixtures::FileType::Css => "temp.css",
        fixtures::FileType::Unknown => return FormattedResult::Failed("Unknown file type".to_string()),
    };

    // Run prettier
    let formatted = match deno::run_prettier(&input, filepath) {
        Ok(f) => f,
        Err(e) => return FormattedResult::Failed(format!("Prettier error: {}", e)),
    };

    let formatted_path = fixture.formatted_path();

    // If formatted output is identical to input, remove formatted.* file
    if formatted == input {
        if formatted_path.exists() {
            match fixtures::delete_file_if_exists(&formatted_path) {
                Ok(_) => FormattedResult::Removed,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            FormattedResult::NotNeeded
        }
    } else {
        // Formatted output differs from input, write/update formatted.* file
        let existing = fixtures::read_file(&formatted_path).ok();

        if Some(&formatted) == existing.as_ref() {
            FormattedResult::Unchanged
        } else if existing.is_none() {
            match fixtures::write_file(&formatted_path, &formatted) {
                Ok(_) => FormattedResult::Created,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            match fixtures::write_file(&formatted_path, &formatted) {
                Ok(_) => FormattedResult::Updated,
                Err(e) => FormattedResult::Failed(e),
            }
        }
    }
}
