use crate::fixtures::{self, validation};
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

        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesValidateExecutable {
            list_only,
            verbose,
            filters,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_validate                           Validate all fixture files (CI)"
                .to_string(),
            "fixtures_validate --list                    List all fixtures".to_string(),
            "fixtures_validate --verbose                 Show successful checks too".to_string(),
            "fixtures_validate <filter>...               Validate matching fixtures".to_string(),
        ]
    }
}

struct FixturesValidateExecutable {
    list_only: bool,
    verbose: bool,
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

        // Create validation context for cross-fixture duplicate detection
        let mut context = validation::ValidationContext::new();
        let mut summary = validation::ValidationSummary::new();

        // Validate each fixture
        for fixture in &fixture_list {
            let result = validation::validate_fixture(fixture, &mut context).await;
            summary.add(result);
        }

        // Check for cross-fixture duplicates (only when not filtering)
        if self.filters.is_empty() {
            summary.cross_fixture_duplicates = context.find_duplicates();
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
