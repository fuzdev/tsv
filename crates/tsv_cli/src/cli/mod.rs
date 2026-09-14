pub mod commands;
pub mod discover;
pub mod format_source;
pub mod input;
pub mod out;
mod pool;
pub mod stack;

use argh::FromArgs;
use commands::{format::FormatCommand, parse::ParseCommand};

/// tsv — TypeScript/Svelte/CSS parser & formatter.
#[derive(FromArgs, Debug)]
pub struct TopLevel {
    /// print the tsv version
    #[argh(switch)]
    pub version: bool,

    #[argh(subcommand)]
    pub nested: Subcommand,
}

/// The one argv `tsv` answers without argh: a bare `--version` (repeated or not — argh
/// counts a repeated switch, and so does the JS mirror's transcription of it).
///
/// The switch is declared on [`TopLevel`] so `--help` lists it, but argh would refuse
/// the bare form for its missing subcommand — and making the subcommand optional to admit
/// it meant restating argh's own required-subcommand error by hand for a bare `tsv`.
/// `main` answers this shape itself; every other argv, `--version` beside a subcommand
/// included, goes to argh and then [`TopLevel::run`].
pub fn is_bare_version(args: &[&str]) -> bool {
    !args.is_empty() && args.iter().all(|arg| *arg == "--version")
}

/// What a bare `--version` prints: `tsv <version>`, the workspace version.
pub fn version_line() -> String {
    format!("tsv {}", env!("CARGO_PKG_VERSION"))
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
pub enum Subcommand {
    Parse(ParseCommand),
    Format(FormatCommand),
}

impl TopLevel {
    pub fn run(self) {
        // The bare switch never reaches here (`is_bare_version`), so a set switch means
        // a subcommand stands beside it — the one argv shape argh accepts that means two
        // things at once, refused rather than silently dropping either
        if self.version {
            out::exit_with_error(
                1,
                "Error: --version cannot be combined with a subcommand\n\nRun tsv --help for more information.",
            );
        }
        match self.nested {
            Subcommand::Parse(c) => c.run(),
            Subcommand::Format(c) => c.run(),
        }
    }
}
