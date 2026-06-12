use argh::FromArgs;
use std::process::{Command as StdCommand, exit};

/// Regenerate expected.json + output_prettier.* (runs parsed + formatted in sequence).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fixtures_update")]
pub struct FixturesUpdateCommand {
    /// fixture filter patterns (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

impl FixturesUpdateCommand {
    pub fn run(self) {
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
                exit(1);
            }
        };

        if !status.success() {
            eprintln!("\nfixtures_update_parsed failed");
            exit(1);
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
                exit(1);
            }
        };

        if !status.success() {
            eprintln!("\nfixtures_update_formatted failed");
            exit(1);
        }

        println!("\n════════════════════\n");
        println!("✓ Both commands completed successfully");
    }
}
