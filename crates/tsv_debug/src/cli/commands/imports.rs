//! `imports` — tsv's import records for one file, as the JSON a binding returns, or on
//! request for the import corpus differential.
//!
//! The parser follows the extension: `.svelte` reads a component
//! (`tsv_svelte::parse_imports`), anything else TypeScript, at the goal the extension
//! settles (`.mjs` / `.mts` a module by name, otherwise Module retried as Script). The
//! triage view of one input the import fixtures or a corpus comparison flags.
//!
//! ## `--stdin`: the import differential's tsv side
//!
//! `benches/js/imports_corpus.ts` (`deno task imports:corpus`) grades tsv's records against
//! oxc-parser's module record over the corpus, and asks this process for tsv's side: one
//! NDJSON request per line on stdin, `{"path", "source"}` (the path picks the parser and the
//! goal, as above; it is never read), answered by exactly one line on stdout, flushed before
//! the next request is read:
//!
//! - `{"imports"}` — the records, the JSON a binding returns (`{hasModuleSyntax, imports}`,
//!   UTF-16 offsets);
//! - `{"error"}` — tsv's parser rejects the source;
//! - `{"panic"}` — the parse or the walk panicked. Each request runs under `catch_unwind`
//!   (the `corpus` profile unwinds), and a panic answers as a panic, so the caller fails on
//!   it as one.
//!
//! A malformed request is a harness bug, not a tsv verdict: it ends the process with an
//! error rather than answering.

use crate::cli::CliError;
use argh::FromArgs;
use std::path::{Path, PathBuf};

/// Print tsv's import records for a file, tab-indented, or answer import requests over
/// stdin (`--stdin`, the import corpus differential's tsv side).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "imports")]
pub struct ImportsCommand {
    /// answer NDJSON import requests read from stdin instead of reading a file
    #[argh(switch)]
    pub stdin: bool,

    /// the TypeScript, JavaScript or Svelte file
    #[argh(positional)]
    pub path: Option<PathBuf>,
}

impl ImportsCommand {
    /// Run the command.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the file can't be read or does not parse, a
    /// `--stdin` request is malformed, or stdin / stdout fails.
    pub fn run(self) -> Result<(), CliError> {
        match (self.stdin, self.path) {
            (true, None) => serve_requests(),
            (false, Some(path)) => imports_file(&path),
            _ => {
                eprintln!("Error: imports takes a file path or --stdin, not both or neither");
                Err(CliError::Failed)
            }
        }
    }
}

fn imports_file(path: &Path) -> Result<(), CliError> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        eprintln!("Error: reading {}: {e}", path.display());
        CliError::Failed
    })?;
    let arena = bumpalo::Bump::new();
    let bytes = imports_at_path(path, &source, &arena).map_err(|e| {
        eprintln!("parse error: {e:#}");
        CliError::Failed
    })?;
    let value: serde_json::Value = crate::json::wire_value(&bytes);
    super::print_json_tabs(&value, CliError::Failed)
}

/// The records of `source` read from `path`, as the JSON a binding returns: a `.svelte`
/// path reads a component, any other TypeScript at the goal its extension settles.
fn imports_at_path(
    path: &Path,
    source: &str,
    arena: &bumpalo::Bump,
) -> Result<Vec<u8>, tsv_lang::Error> {
    let is_svelte = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svelte"));
    if is_svelte {
        tsv_svelte::__private::parse_imports_in(source, arena).map(|imports| {
            tsv_ts::__private::write_imports_json(&imports, source, tsv_svelte::WIRE_COORDINATES)
        })
    } else {
        let goal = tsv_ts::Goal::from_extension(path);
        tsv_ts::__private::parse_imports_into(source, goal, &mut tsv_ts::ast::AstStores::new()).map(
            |imports| {
                tsv_ts::__private::write_imports_json(&imports, source, tsv_ts::WIRE_COORDINATES)
            },
        )
    }
}

/// One `--stdin` request (module doc).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportsRequest {
    path: PathBuf,
    source: String,
}

/// Answer import requests until stdin closes (module doc §`--stdin`).
fn serve_requests() -> Result<(), CliError> {
    super::serve_ndjson("imports", |request: &ImportsRequest, arena| {
        serde_json::to_vec(&answer(&request.path, &request.source, arena))
            .map_err(|e| e.to_string())
    })
}

/// The reply for one source (module doc §`--stdin`).
fn answer(path: &Path, source: &str, arena: &mut bumpalo::Bump) -> serde_json::Value {
    match super::catch_in_arena(arena, |arena| imports_at_path(path, source, arena)) {
        Ok(Ok(bytes)) => serde_json::json!({ "imports": crate::json::wire_value(&bytes) }),
        Ok(Err(e)) => serde_json::json!({ "error": format!("{e:#}") }),
        Err(message) => serde_json::json!({ "panic": message }),
    }
}
