use crate::fixtures::{self, validation};
use futures_util::stream::{self, StreamExt};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-validate command - validate all fixture files
pub struct FixturesValidateCommand;

impl Command for FixturesValidateCommand {
    fn name(&self) -> &str {
        "fixtures_validate"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let list_only = args.flag("list");
        let verbose = args.flag("verbose") || args.flag("v");
        let prettier_only = args.flag("prettier-only");

        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesValidateExecutable {
            list_only,
            verbose,
            prettier_only,
            filters,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_validate                           Validate all fixture files (CI)"
                .to_string(),
            "fixtures_validate --list                    List all fixtures".to_string(),
            "fixtures_validate --verbose                 Show successful checks too".to_string(),
            "fixtures_validate --prettier-only           Skip our parser/formatter (for fixture authoring)"
                .to_string(),
            "fixtures_validate <filter>...               Validate matching fixtures".to_string(),
        ]
    }
}

struct FixturesValidateExecutable {
    list_only: bool,
    verbose: bool,
    prettier_only: bool,
    filters: Vec<String>,
}

impl Executable for FixturesValidateExecutable {
    fn execute(&self) {
        let rt = crate::cli::commands::create_runtime();
        rt.block_on(self.run());
    }
}

impl FixturesValidateExecutable {
    async fn run(&self) {
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
            .filter(|f| f.matches_filters(&self.filters))
            .collect();

        if fixture_list.is_empty() {
            if self.filters.is_empty() {
                eprintln!("No fixtures found");
            } else {
                eprintln!("No fixtures found matching: {}", self.filters.join(" "));
            }
            std::process::exit(1);
        }

        if self.list_only {
            println!("Found fixtures:");
            for fixture in &fixture_list {
                println!("  {} ({})", fixture.relative_path, fixture.input_file);
            }
            if self.filters.is_empty() {
                println!("\nTotal: {}", fixture_list.len());
            } else {
                println!(
                    "\nMatched: {} of {} fixtures",
                    fixture_list.len(),
                    total_count
                );
            }
            return;
        }

        // Validate fixtures concurrently using tokio streams
        let concurrency = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(4);
        let prettier_only = self.prettier_only;

        let results: Vec<_> =
            stream::iter(fixture_list)
                .map(|fixture| async move {
                    validation::validate_fixture(&fixture, prettier_only).await
                })
                .buffer_unordered(concurrency)
                .collect()
                .await;

        // Aggregate results
        let mut summary = validation::ValidationSummary::new();
        for result in results {
            summary.add(result);
        }

        // Check for cross-fixture duplicates (only when not filtering)
        if self.filters.is_empty() {
            summary.detect_cross_fixture_duplicates();
        }

        // Print results with verbose mode
        validation::print_validation_results(&summary, self.verbose);

        // Exit with appropriate code
        if summary.is_valid() {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }
}
