use crate::cli::CliError;
use argh::FromArgs;

/// Regenerate expected.json + output_prettier.* (runs parsed + formatted in sequence).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fixtures_update")]
pub struct FixturesUpdateCommand {
    /// fixture filter patterns (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

impl FixturesUpdateCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let rt = super::create_runtime();
        // One process, one sidecar pool: the formatted step reuses the pool the parsed
        // step spawned.
        rt.block_on(async {
            println!("Running fixtures_update_parsed...\n");
            super::fixtures_update_parsed::run(&self.filters)
                .await
                .inspect_err(|_| eprintln!("\nfixtures_update_parsed failed"))?;

            println!("\n════════════════════\n");
            println!("Running fixtures_update_formatted...\n");
            super::fixtures_update_formatted::run(&self.filters)
                .await
                .inspect_err(|_| eprintln!("\nfixtures_update_formatted failed"))?;

            println!("\n════════════════════\n");
            println!("✓ Both commands completed successfully");
            Ok(())
        })
    }
}
