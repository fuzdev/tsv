//! `imports` — tsv's import records for one file, as the JSON a binding returns.
//!
//! The parser follows the extension: `.svelte` reads a component
//! (`tsv_svelte::parse_imports`), anything else TypeScript, at the goal the extension
//! settles (`.mjs` / `.mts` a module by name, otherwise Module retried as Script). The
//! triage view of one input the import fixtures or a corpus comparison flags.

use crate::cli::CliError;
use argh::FromArgs;
use std::path::PathBuf;

/// Print tsv's import records for a file, tab-indented.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "imports")]
pub struct ImportsCommand {
    /// the TypeScript, JavaScript or Svelte file
    #[argh(positional)]
    pub path: PathBuf,
}

impl ImportsCommand {
    /// Run the command.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the file can't be read or does not parse.
    pub fn run(self) -> Result<(), CliError> {
        let source = std::fs::read_to_string(&self.path).map_err(|e| {
            eprintln!("Error: reading {}: {e}", self.path.display());
            CliError::Failed
        })?;
        let is_svelte = self
            .path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svelte"));
        let bytes = if is_svelte {
            tsv_svelte::parse_imports(&source).map(|imports| {
                tsv_ts::__private::write_imports_json(
                    &imports,
                    &source,
                    tsv_svelte::WIRE_COORDINATES,
                )
            })
        } else {
            let arena = bumpalo::Bump::new();
            let goal = tsv_ts::Goal::from_extension(&self.path);
            tsv_ts::__private::parse_imports_in(&source, goal, &arena).map(|imports| {
                tsv_ts::__private::write_imports_json(&imports, &source, tsv_ts::WIRE_COORDINATES)
            })
        }
        .map_err(|e| {
            eprintln!("parse error: {e:#}");
            CliError::Failed
        })?;
        let value: serde_json::Value = crate::json::wire_value(&bytes);
        super::print_json_tabs(&value, CliError::Failed)
    }
}
