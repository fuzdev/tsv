use std::process::{Command as StdCommand, exit};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures-update command - regenerate both expected.json and output_prettier.svelte files
pub struct FixturesUpdateCommand;

impl Command for FixturesUpdateCommand {
    fn name(&self) -> &str {
        "fixtures_update"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesUpdateExecutable { filters }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_update                   Regenerate both expected.json and output_prettier.svelte"
                .to_string(),
            "fixtures_update <filter>...       Regenerate matching fixtures".to_string(),
        ]
    }
}

struct FixturesUpdateExecutable {
    filters: Vec<String>,
}

impl Executable for FixturesUpdateExecutable {
    fn execute(&self) {
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
