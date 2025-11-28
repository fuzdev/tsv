use crate::fixtures;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-update-formatted command - regenerate output_prettier.svelte files
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
            "fixtures_update_formatted                   Regenerate all output_prettier.svelte files"
                .to_string(),
            "fixtures_update_formatted <filter>...       Regenerate matching fixtures".to_string(),
        ]
    }
}

struct FixturesUpdateFormattedExecutable {
    filters: Vec<String>,
}

impl Executable for FixturesUpdateFormattedExecutable {
    fn execute(&self) {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(run(&self.filters));
    }
}

async fn run(filters: &[String]) {
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
    let fixture_list: Vec<_> = all_fixtures
        .into_iter()
        .filter(|f| f.matches_filters(filters))
        .collect();

    if fixture_list.is_empty() {
        if filters.is_empty() {
            eprintln!("No fixtures found");
        } else {
            eprintln!("No fixtures found matching: {}", filters.join(" "));
        }
        std::process::exit(1);
    }

    let mut created = 0;
    let mut updated = 0;
    let mut removed = 0;
    let mut unchanged = 0;
    let mut failed = 0;

    for fixture in &fixture_list {
        match update_formatted_file(fixture).await {
            FormattedResult::Created => {
                println!("✓ Created {}/output_prettier.svelte", fixture.relative_path);
                created += 1;
            }
            FormattedResult::Updated => {
                println!("✓ Updated {}/output_prettier.svelte", fixture.relative_path);
                updated += 1;
            }
            FormattedResult::Removed => {
                println!(
                    "✓ Removed {}/output_prettier.svelte (identical to input)",
                    fixture.relative_path
                );
                removed += 1;
            }
            FormattedResult::Unchanged => {
                println!(
                    "- {}/output_prettier.svelte is up to date",
                    fixture.relative_path
                );
                unchanged += 1;
            }
            FormattedResult::NotNeeded => {
                println!(
                    "- {}/output_prettier.svelte not needed (input already formatted)",
                    fixture.relative_path
                );
                unchanged += 1;
            }
            FormattedResult::Failed(err) => {
                eprintln!("✗ Failed to process {}: {}", fixture.relative_path, err);
                failed += 1;
            }
        }
    }

    if filters.is_empty() {
        println!(
            "\nSummary: {} created, {} updated, {} removed, {} unchanged, {} failed ({} fixtures)",
            created,
            updated,
            removed,
            unchanged,
            failed,
            fixture_list.len()
        );
    } else {
        println!(
            "\nSummary: {} created, {} updated, {} removed, {} unchanged, {} failed (matched {} of {} fixtures)",
            created,
            updated,
            removed,
            unchanged,
            failed,
            fixture_list.len(),
            total_count
        );
    }

    if created > 0 || updated > 0 || removed > 0 {
        println!("⚠️  Updated source of truth files (output_prettier.svelte)");
    }

    if failed > 0 {
        std::process::exit(1);
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

async fn update_formatted_file(fixture: &fixtures::Fixture) -> FormattedResult {
    // Read input file
    let input = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => return FormattedResult::Failed(e),
    };

    // Determine filepath for prettier (always temp.svelte)
    let filepath = "temp.svelte";

    // Run prettier
    let formatted = match fuz_client::run_prettier(&input, filepath).await {
        Ok(f) => f,
        Err(e) => return FormattedResult::Failed(format!("Prettier error: {}", e)),
    };

    let output_prettier_path = fixture.output_prettier_path();

    // If formatted output is identical to input, remove output_prettier.svelte file
    if formatted == input {
        if output_prettier_path.exists() {
            match fixtures::delete_file_if_exists(&output_prettier_path) {
                Ok(_) => FormattedResult::Removed,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            FormattedResult::NotNeeded
        }
    } else {
        // Formatted output differs from input, write/update output_prettier.svelte file
        let existing = fixtures::read_file(&output_prettier_path).ok();

        if Some(&formatted) == existing.as_ref() {
            FormattedResult::Unchanged
        } else if existing.is_none() {
            match fixtures::write_file(&output_prettier_path, &formatted) {
                Ok(_) => FormattedResult::Created,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            match fixtures::write_file(&output_prettier_path, &formatted) {
                Ok(_) => FormattedResult::Updated,
                Err(e) => FormattedResult::Failed(e),
            }
        }
    }
}
