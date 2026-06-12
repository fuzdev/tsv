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

        // Bulk workload: spread the JS work (prettier/parsers) across a small
        // sidecar pool — a single sidecar is one single-threaded process and
        // becomes the wall-clock bound
        crate::deno::set_pool_size(crate::deno::bulk_pool_size(concurrency));

        // tokio::spawn per fixture: buffer_unordered alone only interleaves at
        // await points on the single stream-driving task — the CPU-bound Rust
        // work (parse, format, serde, diff) would serialize on one core.
        // Spawned tasks run on all runtime workers.
        let mut results = stream::iter(fixture_list)
            .map(|fixture| {
                tokio::spawn(
                    async move { validation::validate_fixture(&fixture, prettier_only).await },
                )
            })
            .buffer_unordered(concurrency);

        // Aggregate results, printing each fixture's buffered failure diffs as it
        // completes — phases never print directly, so concurrent fixtures can't
        // interleave output (each fixture's diffs stay contiguous, in completion order)
        let mut summary = validation::ValidationSummary::new();
        while let Some(joined) = results.next().await {
            let result = match joined {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("fixture validation task panicked: {e}");
                    std::process::exit(2);
                }
            };
            if !result.diff_output.is_empty() {
                eprint!("{}", result.diff_output);
            }
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
