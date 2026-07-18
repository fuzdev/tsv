use crate::cli::CliError;
use crate::fixtures::validation;
use argh::FromArgs;
use futures_util::StreamExt;

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
    pub(crate) fn run(self) -> Result<(), CliError> {
        let rt = crate::cli::commands::create_runtime();
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> Result<(), CliError> {
        let (fixture_list, total_count) = super::walk_and_filter(&self.filters)?;

        if self.list {
            super::print_fixture_list(&fixture_list, &self.filters, total_count);
            return Ok(());
        }

        // Validate fixtures concurrently on the bulk sidecar pool, in completion
        // order — each fixture's failure diffs are buffered into its result, so
        // phases never print directly and concurrent fixtures can't interleave
        // output (each fixture's diffs stay contiguous).
        let prettier_only = self.prettier_only;
        let mut results = super::spawn_work_stream(
            fixture_list,
            super::ResultOrder::Completion,
            move |fixture| async move { validation::validate_fixture(&fixture, prettier_only).await },
        );

        let mut summary = validation::ValidationSummary::new();
        while let Some(joined) = results.next().await {
            let result = super::task_result(joined, "validation")?;
            if !result.diff_output.is_empty() {
                eprint!("{}", result.diff_output);
            }
            summary.add(result);
        }

        // Check for cross-fixture duplicates (only when not filtering)
        if self.filters.is_empty() {
            summary.detect_cross_fixture_duplicates();
            summary.detect_stale_benign_render_equiv();
        }

        // Print results with verbose mode
        validation::print_validation_results(&summary, self.verbose);

        // REGRESSION PIN — see `validation::FIXTURES_MIN` (shared with the
        // fixtures_tests integration test, the form CI runs).
        if self.filters.is_empty() && summary.total_fixtures < validation::FIXTURES_MIN {
            eprintln!(
                "Error: pinned minimum — validated {} fixtures < pinned {}. \
                 Fixture discovery shrank; if deliberate (fixtures deleted), re-pin FIXTURES_MIN.",
                summary.total_fixtures,
                validation::FIXTURES_MIN
            );
            return Err(CliError::Failed);
        }

        // Exit with appropriate code
        if summary.is_valid() {
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}
