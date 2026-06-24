use crate::fixtures::validation;
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
        let (fixture_list, total_count) = super::walk_and_filter(&self.filters);

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

        // Validate fixtures concurrently using tokio streams. Bulk workload:
        // spread the JS work (prettier/parsers) across a small sidecar pool.
        let concurrency = crate::deno::init_bulk_pool();
        let prettier_only = self.prettier_only;

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
