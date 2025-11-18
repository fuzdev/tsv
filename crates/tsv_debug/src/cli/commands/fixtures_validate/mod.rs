use crate::fixtures;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

mod reporting;
mod validations;

/// fixtures-validate command - validate all fixture files (input.svelte, output_prettier.svelte, prettier_quirk_*.svelte, unformatted_*.svelte)
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

        // Run validations
        let results = validations::run_validations(&fixtures, &self.filters);

        // Check for errors
        let has_errors = !results.invalid_inputs.is_empty()
            || !results.structure_errors.is_empty()
            || !results.unformatted_mismatch.is_empty()
            || !results.duplicates.is_empty()
            || !results.unformatted_duplicates.is_empty()
            || !results.prettier_quirk_duplicates.is_empty()
            || !results.prettier_quirk_not_idempotent.is_empty()
            || !results.non_idempotent.is_empty()
            || !results.prettier_quirk_not_normalized.is_empty()
            || !results.unformatted_not_normalized.is_empty()
            || !results.unformatted_ours_not_normalized.is_empty()
            || !results.outdated_expected_ours.is_empty()
            || !results.outdated_expected.is_empty()
            || !results.outdated_expected_svelte.is_empty()
            || !results.outdated_output_prettier.is_empty()
            || !results.redundant_unformatted.is_empty();

        if !has_errors {
            if self.filters.is_empty() {
                println!(
                    "✓ All {} fixtures validated ({} unformatted_*.svelte, {} unformatted_ours_*.svelte, {} prettier_quirk_*.svelte)",
                    results.checked, results.unformatted_checked, results.unformatted_ours_checked, results.prettier_quirk_checked
                );
            } else {
                println!(
                    "✓ Matched {} of {} fixtures pass validation ({} unformatted_*.svelte, {} unformatted_ours_*.svelte, {} prettier_quirk_*.svelte)",
                    results.checked,
                    total_count,
                    results.unformatted_checked,
                    results.unformatted_ours_checked,
                    results.prettier_quirk_checked
                );
                println!(
                    "⚠️  Skipping cross-fixture duplicate detection (run without filters for full validation)"
                );
            }
            std::process::exit(0);
        }

        // Report all errors
        reporting::report_all_errors(&results);

        // Show summary of failed fixtures
        let mut failed_list: Vec<String> = results.failed_fixtures.into_iter().collect();
        failed_list.sort();
        let passed = results.checked - failed_list.len();

        eprintln!("\n════════════════════");
        eprintln!();
        eprintln!(
            "{} / {} fixtures failed:\n",
            failed_list.len(),
            results.checked
        );
        for fixture in &failed_list {
            eprintln!("  ✗ {}", fixture);
        }
        eprintln!();
        eprintln!(
            "Results Summary: {} passed, {} failed out of {} total",
            passed,
            failed_list.len(),
            results.checked
        );
        eprintln!();

        std::process::exit(1);
    }
}
