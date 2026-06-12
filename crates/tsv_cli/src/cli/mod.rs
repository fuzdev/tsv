pub mod commands;
pub mod discover;
pub mod format_source;
pub mod input;

use argh::FromArgs;
use commands::{format::FormatCommand, parse::ParseCommand};

/// tsv — TypeScript/Svelte/CSS parser & formatter.
#[derive(FromArgs, Debug)]
pub struct TopLevel {
    #[argh(subcommand)]
    pub nested: Subcommand,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
pub enum Subcommand {
    Parse(ParseCommand),
    Format(FormatCommand),
}

impl TopLevel {
    pub fn run(self) {
        match self.nested {
            Subcommand::Parse(c) => c.run(),
            Subcommand::Format(c) => c.run(),
        }
    }
}
