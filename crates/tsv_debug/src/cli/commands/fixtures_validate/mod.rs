use crate::fixtures::{self, validation};
use argh::FromArgs;
use futures_util::stream::{self, StreamExt};

/// Validate all fixture files (CI).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fixtures_validate")]
pub struct FixturesValidateCommand {
    /// list matching fixtures only (do not validate)
    #[argh(switch)]
    list: bool,

    /// show successful checks too
    #[argh(switch, short = 'v')]
    verbose: bool,

    /// skip our parser/formatter (for fixture authoring)
    #[argh(switch)]
    prettier_only: bool,

    /// fixture filter patterns (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

impl FixturesValidateCommand {
    pub fn run(self) {
        let rt = crate::cli::commands::create_runtime();
        rt.block_on(self.run_async());
    }

    async fn run_async(self) {
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

        if self.list {
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
