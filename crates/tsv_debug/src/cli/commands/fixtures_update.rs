use crate::cli::CliError;
use argh::FromArgs;
use std::process::Command as StdCommand;

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
        println!("Running fixtures_update_parsed...\n");

        // Build command: cargo run -p tsv_debug --quiet fixtures_update_parsed [filters...]
        let mut cmd = StdCommand::new("cargo");
        cmd.arg("run")
            .arg("-p")
            .arg("tsv_debug")
            .arg("--quiet")
            .arg("fixtures_update_parsed");

        for filter in &self.filters {
            cmd.arg(filter);
        }

        let status = match cmd.status() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to run fixtures_update_parsed: {e}");
                return Err(CliError::Failed);
            }
        };

        if !status.success() {
            eprintln!("\nfixtures_update_parsed failed");
            return Err(CliError::Failed);
        }

        println!("\n════════════════════\n");
        println!("Running fixtures_update_formatted...\n");

        // Build command: cargo run -p tsv_debug --quiet fixtures_update_formatted [filters...]
        let mut cmd = StdCommand::new("cargo");
        cmd.arg("run")
            .arg("-p")
            .arg("tsv_debug")
            .arg("--quiet")
            .arg("fixtures_update_formatted");

        for filter in &self.filters {
            cmd.arg(filter);
        }

        let status = match cmd.status() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to run fixtures_update_formatted: {e}");
                return Err(CliError::Failed);
            }
        };

        if !status.success() {
            eprintln!("\nfixtures_update_formatted failed");
            return Err(CliError::Failed);
        }

        println!("\n════════════════════\n");
        println!("✓ Both commands completed successfully");
        Ok(())
    }
}
