pub mod commands;
pub mod discover;
pub mod format_source;
pub mod input;

use argh::FromArgs;
use commands::{format::FormatCommand, parse::ParseCommand};

/// tsv — TypeScript/Svelte/CSS parser & formatter.
#[derive(FromArgs, Debug)]
pub struct TopLevel {
    /// print the tsv version
    #[argh(switch)]
    pub version: bool,

    #[argh(subcommand)]
    pub nested: Option<Subcommand>,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
pub enum Subcommand {
    Parse(ParseCommand),
    Format(FormatCommand),
}

impl TopLevel {
    pub fn run(self) {
        if self.version {
            println!("tsv {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        match self.nested {
            Some(Subcommand::Parse(c)) => c.run(),
            Some(Subcommand::Format(c)) => c.run(),
            // The subcommand is optional only so a bare `--version` parses;
            // a bare `tsv` must keep argh's required-subcommand behavior, so
            // this mirrors the exact text argh printed when the field was
            // required (the npm cli.js pins the same contract: help-shaped
            // stderr, exit 1).
            None => {
                eprintln!(
                    "One of the following subcommands must be present:\n    help\n    parse\n    format\n\nRun tsv --help for more information."
                );
                std::process::exit(1);
            }
        }
    }
}
