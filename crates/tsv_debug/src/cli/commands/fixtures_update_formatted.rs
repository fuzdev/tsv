use crate::fixtures::{self, discover_unformatted_ours_variants, has_prettier_divergence_suffix};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-update-formatted command - regenerate output_prettier.* and prettier_intermediate_* files
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
            "fixtures_update_formatted                   Regenerate output_prettier.* and prettier_intermediate_* files"
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
        let rt = super::create_runtime();
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
            eprintln!("Error walking fixtures: {e}");
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

    // Separate counters for prettier_intermediate_*
    let mut intermediate_created = 0;
    let mut intermediate_updated = 0;
    let mut intermediate_removed = 0;
    let mut intermediate_unchanged = 0;

    for fixture in &fixture_list {
        // Update output_prettier.*
        let output_filename = fixture.output_prettier_filename();
        match update_formatted_file(fixture).await {
            FormattedResult::Created => {
                println!("✓ Created {}/{}", fixture.relative_path, output_filename);
                created += 1;
            }
            FormattedResult::Updated => {
                println!("✓ Updated {}/{}", fixture.relative_path, output_filename);
                updated += 1;
            }
            FormattedResult::Removed => {
                println!(
                    "✓ Removed {}/{} (identical to input)",
                    fixture.relative_path, output_filename
                );
                removed += 1;
            }
            FormattedResult::Unchanged => {
                println!(
                    "- {}/{} is up to date",
                    fixture.relative_path, output_filename
                );
                unchanged += 1;
            }
            FormattedResult::NotNeeded => {
                println!(
                    "- {}/{} not needed (input already formatted)",
                    fixture.relative_path, output_filename
                );
                unchanged += 1;
            }
            FormattedResult::Failed(err) => {
                eprintln!("✗ Failed to process {}: {}", fixture.relative_path, err);
                failed += 1;
            }
        }

        // Update prettier_intermediate_* files (only in _prettier_divergence directories)
        let dir_name = fixture
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if has_prettier_divergence_suffix(dir_name) {
            let input_ext = fixture.input_type().extension();
            let results = update_intermediate_files(fixture, input_ext).await;
            for (filename, result) in results {
                match result {
                    FormattedResult::Created => {
                        println!("✓ Created {}/{}", fixture.relative_path, filename);
                        intermediate_created += 1;
                    }
                    FormattedResult::Updated => {
                        println!("✓ Updated {}/{}", fixture.relative_path, filename);
                        intermediate_updated += 1;
                    }
                    FormattedResult::Removed => {
                        println!(
                            "✓ Removed {}/{} (prettier normalizes directly)",
                            fixture.relative_path, filename
                        );
                        intermediate_removed += 1;
                    }
                    FormattedResult::Unchanged => {
                        println!("- {}/{} is up to date", fixture.relative_path, filename);
                        intermediate_unchanged += 1;
                    }
                    FormattedResult::NotNeeded => {
                        intermediate_unchanged += 1;
                    }
                    FormattedResult::Failed(err) => {
                        eprintln!(
                            "✗ Failed to process {}/{}: {}",
                            fixture.relative_path, filename, err
                        );
                        failed += 1;
                    }
                }
            }
        }
    }

    let total_created = created + intermediate_created;
    let total_updated = updated + intermediate_updated;
    let total_removed = removed + intermediate_removed;
    let total_unchanged = unchanged + intermediate_unchanged;

    if filters.is_empty() {
        println!(
            "\nSummary: {} created, {} updated, {} removed, {} unchanged, {} failed ({} fixtures)",
            total_created,
            total_updated,
            total_removed,
            total_unchanged,
            failed,
            fixture_list.len()
        );
    } else {
        println!(
            "\nSummary: {} created, {} updated, {} removed, {} unchanged, {} failed (matched {} of {} fixtures)",
            total_created,
            total_updated,
            total_removed,
            total_unchanged,
            failed,
            fixture_list.len(),
            total_count
        );
    }

    if created > 0 || updated > 0 || removed > 0 {
        println!("⚠️  Updated source of truth files (output_prettier.*)");
    }
    if intermediate_created > 0 || intermediate_updated > 0 || intermediate_removed > 0 {
        println!("⚠️  Updated prettier_intermediate_* files");
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

    // Run prettier
    let formatted =
        match crate::deno::run_prettier(&input, fixture.input_type().prettier_parser()).await {
            Ok(f) => f,
            Err(e) => return FormattedResult::Failed(format!("Prettier error: {e}")),
        };

    let output_prettier_path = fixture.output_prettier_path();

    // If formatted output is identical to input, remove output_prettier file
    if formatted == input {
        if output_prettier_path.exists() {
            match fixtures::delete_file_if_exists(&output_prettier_path) {
                Ok(()) => FormattedResult::Removed,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            FormattedResult::NotNeeded
        }
    } else {
        // Formatted output differs from input, write/update output_prettier file
        let existing = fixtures::read_file(&output_prettier_path).ok();

        if Some(&formatted) == existing.as_ref() {
            FormattedResult::Unchanged
        } else if existing.is_none() {
            match fixtures::write_file(&output_prettier_path, &formatted) {
                Ok(()) => FormattedResult::Created,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            match fixtures::write_file(&output_prettier_path, &formatted) {
                Ok(()) => FormattedResult::Updated,
                Err(e) => FormattedResult::Failed(e),
            }
        }
    }
}

/// Update prettier_intermediate_* files for a fixture
///
/// For each unformatted_ours_* file, runs prettier to get first-pass output.
/// If output differs from input (unstable intermediate), writes/updates prettier_intermediate_*.
/// If output equals input (prettier normalizes directly), removes any existing prettier_intermediate_*.
async fn update_intermediate_files(
    fixture: &fixtures::Fixture,
    input_ext: &str,
) -> Vec<(String, FormattedResult)> {
    let mut results = Vec::new();

    // Read input file for comparison
    let input = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => {
            results.push((
                "prettier_intermediate_*".to_string(),
                FormattedResult::Failed(e),
            ));
            return results;
        }
    };

    let unformatted_ours_variants = discover_unformatted_ours_variants(&fixture.path, input_ext);

    for variant_name in unformatted_ours_variants {
        // Extract suffix: unformatted_ours_X.svelte -> X
        let suffix = variant_name
            .strip_prefix("unformatted_ours_")
            .and_then(|s| s.strip_suffix(input_ext))
            .unwrap_or("");

        let intermediate_filename = format!("prettier_intermediate_{suffix}{input_ext}");
        let intermediate_path = fixture.path.join(&intermediate_filename);

        // Read variant file
        let variant_path = fixture.path.join(&variant_name);
        let variant_content = match fixtures::read_file(&variant_path) {
            Ok(s) => s,
            Err(e) => {
                results.push((intermediate_filename, FormattedResult::Failed(e)));
                continue;
            }
        };

        // Run prettier on variant
        let formatted = match crate::deno::run_prettier(
            &variant_content,
            fixture.input_type().prettier_parser(),
        )
        .await
        {
            Ok(f) => f,
            Err(e) => {
                results.push((
                    intermediate_filename,
                    FormattedResult::Failed(format!("Prettier error: {e}")),
                ));
                continue;
            }
        };

        // If prettier normalizes directly to input, remove any existing intermediate file
        if formatted == input {
            if intermediate_path.exists() {
                match fixtures::delete_file_if_exists(&intermediate_path) {
                    Ok(()) => results.push((intermediate_filename, FormattedResult::Removed)),
                    Err(e) => results.push((intermediate_filename, FormattedResult::Failed(e))),
                }
            }
            // No intermediate needed - prettier goes directly to input
            continue;
        }

        // Prettier produces unstable intermediate output, write/update the file
        let existing = fixtures::read_file(&intermediate_path).ok();

        let result = if Some(&formatted) == existing.as_ref() {
            FormattedResult::Unchanged
        } else if existing.is_none() {
            match fixtures::write_file(&intermediate_path, &formatted) {
                Ok(()) => FormattedResult::Created,
                Err(e) => FormattedResult::Failed(e),
            }
        } else {
            match fixtures::write_file(&intermediate_path, &formatted) {
                Ok(()) => FormattedResult::Updated,
                Err(e) => FormattedResult::Failed(e),
            }
        };

        results.push((intermediate_filename, result));
    }

    results
}
