pub mod commands;
pub mod discover;
pub mod format_source;
pub mod input;
pub mod out;
pub mod stack;

use crate::out_line;
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
            out_line!("tsv {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        match self.nested {
            Some(Subcommand::Parse(c)) => c.run(),
            Some(Subcommand::Format(c)) => c.run(),
            // The subcommand is optional only so a bare `--version` parses;
            // a bare `tsv` must keep argh's required-subcommand behavior, so
            // this mirrors the exact text argh printed when the field was
            // required (`cli.js` prints the same bytes, exit 1).
            None => out::exit_with_error(
                1,
                "One of the following subcommands must be present:\n    help\n    parse\n    format\n\nRun tsv --help for more information.",
            ),
        }
    }
}
