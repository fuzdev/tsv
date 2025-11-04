use crate::{deno, fixtures};
use std::path::Path;
use std::process::Command as ProcessCommand;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-update-expected command - regenerate expected.json files
pub struct FixturesUpdateExpectedCommand;

impl Command for FixturesUpdateExpectedCommand {
    fn name(&self) -> &str {
        "fixtures_update_expected"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let list_only = args.flag("list");

        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesUpdateExpectedExecutable {
            list_only,
            filters,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_update_expected                    Regenerate all expected.json files"
                .to_string(),
            "fixtures_update_expected --list             List all fixtures".to_string(),
            "fixtures_update_expected <filter>...        Regenerate matching fixtures".to_string(),
        ]
    }
}

struct FixturesUpdateExpectedExecutable {
    list_only: bool,
    filters: Vec<String>,
}

impl Executable for FixturesUpdateExpectedExecutable {
    fn execute(&self) {
        let fixtures_dir = Path::new("tests/fixtures");

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

        let mut created = 0;
        let mut updated = 0;
        let mut unchanged = 0;
        let mut failed = 0;

        for fixture in &fixtures {
            match generate_expected_fixture(fixture) {
                FixtureResult::Created => {
                    println!("✓ Created {}/expected.json", fixture.relative_path);
                    created += 1;
                }
                FixtureResult::Updated => {
                    println!("✓ Updated {}/expected.json", fixture.relative_path);
                    updated += 1;
                }
                FixtureResult::Unchanged => {
                    println!("- {}/expected.json is up to date", fixture.relative_path);
                    unchanged += 1;
                }
                FixtureResult::Failed(err) => {
                    eprintln!("✗ Failed to generate {}: {}", fixture.relative_path, err);
                    failed += 1;
                }
            }
        }

        if self.filters.is_empty() {
            println!(
                "\nSummary: {} created, {} updated, {} unchanged, {} failed ({} fixtures)",
                created,
                updated,
                unchanged,
                failed,
                fixtures.len()
            );
        } else {
            println!(
                "\nSummary: {} created, {} updated, {} unchanged, {} failed (matched {} of {} fixtures)",
                created,
                updated,
                unchanged,
                failed,
                fixtures.len(),
                total_count
            );
        }

        if created > 0 || updated > 0 {
            println!("⚠️  Updated source of truth files (expected.json)");
        }

        if failed > 0 {
            std::process::exit(1);
        }
    }
}

enum FixtureResult {
    Created,
    Updated,
    Unchanged,
    Failed(String),
}

fn generate_expected_fixture(fixture: &fixtures::Fixture) -> FixtureResult {
    // Read input file
    let source = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => return FixtureResult::Failed(e),
    };

    // Parse based on file type
    let json = match fixture.file_type() {
        fixtures::FileType::Svelte => match deno::parse_svelte(&source) {
            Ok(json) => json,
            Err(e) => return FixtureResult::Failed(format!("Svelte parse error: {}", e)),
        },
        fixtures::FileType::SvelteTypeScript | fixtures::FileType::TypeScript => {
            match deno::parse_typescript(&source) {
                Ok(json) => json,
                Err(e) => return FixtureResult::Failed(format!("TypeScript parse error: {}", e)),
            }
        }
        fixtures::FileType::Css => {
            // Use our Rust parser for CSS
            match parse_css_via_rust(&fixture.input_path()) {
                Ok(json) => json,
                Err(e) => return FixtureResult::Failed(format!("CSS parse error: {}", e)),
            }
        }
        fixtures::FileType::Unknown => {
            return FixtureResult::Failed("Unknown file type".to_string());
        }
    };

    // Ensure JSON ends with newline
    let json = if json.ends_with('\n') {
        json
    } else {
        format!("{}\n", json)
    };

    let expected_path = fixture.expected_path();

    // Check if expected.json exists and compare
    let existing = fixtures::read_file(&expected_path).ok();

    if Some(&json) == existing.as_ref() {
        FixtureResult::Unchanged
    } else if existing.is_none() {
        match fixtures::write_file(&expected_path, &json) {
            Ok(_) => FixtureResult::Created,
            Err(e) => FixtureResult::Failed(e),
        }
    } else {
        match fixtures::write_file(&expected_path, &json) {
            Ok(_) => FixtureResult::Updated,
            Err(e) => FixtureResult::Failed(e),
        }
    }
}

fn parse_css_via_rust(input_path: &Path) -> Result<String, String> {
    // Read the CSS file content
    let source = fixtures::read_file(input_path)?;

    // Parse via --content to avoid path issues
    let output = ProcessCommand::new("cargo")
        .args([
            "run",
            "-p",
            "tsv_cli",
            "--quiet",
            "--",
            "parse",
            "--content",
        ])
        .arg(&source)
        .args(["--pretty"])
        .output()
        .map_err(|e| format!("Failed to execute cargo: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
