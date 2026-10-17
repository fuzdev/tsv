//! `strip` — `tsv_ts::strip` on one file, or on request for the strip corpus differential.
//!
//! With a path, prints the stripped JS to stdout; a refusal or a parse error goes to
//! stderr and exits 1. The path's extension settles the goal as `tsv strip` reads it
//! (`tsv_ts::Goal::from_extension`). The triage view of one file the differential flags.
//!
//! ## `--stdin`: the strip differential's tsv side
//!
//! `benches/js/strip_corpus.ts` (`deno task strip:corpus`) grades tsv's strip against
//! ts-blank-space over the corpus, and asks this process for tsv's side: one NDJSON request
//! per line on stdin, `{"source"}`, answered by exactly one line on stdout, flushed before
//! the next request is read:
//!
//! - `{"code", "findings"}` — the stripped JS, and what it breaks of the strip's own
//!   contract (`strip_fixtures::contract_violations`), its idempotence
//!   (`strip_fixtures::idempotence_failure`), or the no-copy promise (an output equal to
//!   its source must be borrowed), one message each; `findings` is empty on a clean strip;
//! - `{"refusal"}` — the refusal as `refusal.txt` spells it
//!   (`Kind<TAB>line:column<TAB>line:column` and a newline);
//! - `{"error"}` — tsv's parser rejects the source;
//! - `{"panic"}` — the strip panicked. Each request runs under `catch_unwind` (the `corpus`
//!   profile unwinds), and a panic answers as a panic, so the caller fails on it as one.
//!
//! A malformed request is a harness bug, not a tsv verdict: it ends the process with an
//! error rather than answering.

use crate::cli::CliError;
use crate::strip_fixtures::{contract_violations, idempotence_failure, refusal_line};
use argh::FromArgs;
use std::borrow::Cow;
use std::io::Write;
use std::path::PathBuf;

/// Strip the types from a TypeScript file to stdout, or answer strip requests over stdin
/// (`--stdin`, the strip corpus differential's tsv side).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "strip")]
pub struct StripCommand {
    /// answer NDJSON strip requests read from stdin instead of stripping a file
    #[argh(switch)]
    pub stdin: bool,

    /// the TypeScript file to strip
    #[argh(positional)]
    pub path: Option<PathBuf>,
}

impl StripCommand {
    /// Run the command.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the file can't be read, is refused or does not
    /// parse, a `--stdin` request is malformed, or stdin / stdout fails.
    pub fn run(self) -> Result<(), CliError> {
        match (self.stdin, self.path) {
            (true, None) => serve_requests(),
            (false, Some(path)) => strip_file(&path),
            _ => {
                eprintln!("Error: strip takes a file path or --stdin, not both or neither");
                Err(CliError::Failed)
            }
        }
    }
}

fn strip_file(path: &std::path::Path) -> Result<(), CliError> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        eprintln!("Error: reading {}: {e}", path.display());
        CliError::Failed
    })?;
    let arena = bumpalo::Bump::new();
    match strip_at_path(path, &source, &arena) {
        Ok(stripped) => {
            print!("{}", stripped.code);
            Ok(())
        }
        Err(tsv_ts::StripError::Unsupported { kind, span }) => {
            eprint!("refused: {}", refusal_line(&source, kind, span));
            Err(CliError::Failed)
        }
        Err(tsv_ts::StripError::Parse(e)) => {
            eprintln!("parse error: {e}");
            Err(CliError::Failed)
        }
    }
}

/// Strip `source` read from `path`, at the goal the path's extension settles — a `.mts` is
/// a module by name, as the shipped `tsv strip` reads a path; any other the fallback.
fn strip_at_path<'s>(
    path: &std::path::Path,
    source: &'s str,
    arena: &bumpalo::Bump,
) -> Result<tsv_ts::Stripped<'s>, tsv_ts::StripError> {
    let goal = path.to_str().and_then(tsv_ts::Goal::from_extension);
    tsv_ts::strip_with_goal(source, goal, arena)
}

/// One `--stdin` request (module doc).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StripRequest {
    source: String,
}

/// Answer strip requests until stdin closes (module doc §`--stdin`).
fn serve_requests() -> Result<(), CliError> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let mut line = String::new();
    let mut arena = bumpalo::Bump::new();
    loop {
        line.clear();
        let read = std::io::BufRead::read_line(&mut input, &mut line).map_err(|e| {
            eprintln!("Error: reading stdin: {e}");
            CliError::Failed
        })?;
        if read == 0 {
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }
        let request: StripRequest = serde_json::from_str(&line).map_err(|e| {
            eprintln!("Error: malformed strip request: {e}");
            CliError::Failed
        })?;
        let reply = answer(&request.source, &mut arena);
        let written = serde_json::to_writer(&mut out, &reply)
            .map_err(std::io::Error::from)
            .and_then(|()| out.write_all(b"\n"))
            .and_then(|()| out.flush());
        if let Err(e) = written {
            eprintln!("Error: writing stdout: {e}");
            return Err(CliError::Failed);
        }
    }
}

/// The reply for one source (module doc §`--stdin`).
fn answer(source: &str, arena: &mut bumpalo::Bump) -> serde_json::Value {
    arena.reset();
    let result = {
        let arena: &bumpalo::Bump = arena;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tsv_ts::strip(source, arena).map(|stripped| {
                let code = stripped.code.as_ref();
                let mut findings: Vec<String> = contract_violations(source, code)
                    .into_iter()
                    .map(|(_, detail)| detail)
                    .collect();
                if code == source && matches!(stripped.code, Cow::Owned(_)) {
                    findings.push("nothing is type-only, yet the output is a copy".to_string());
                }
                findings.extend(idempotence_failure(code));
                (code.to_string(), findings)
            })
        }))
    };
    match result {
        Ok(Ok((code, findings))) => serde_json::json!({ "code": code, "findings": findings }),
        Ok(Err(tsv_ts::StripError::Unsupported { kind, span })) => {
            serde_json::json!({ "refusal": refusal_line(source, kind, span) })
        }
        Ok(Err(tsv_ts::StripError::Parse(e))) => serde_json::json!({ "error": e.to_string() }),
        Err(payload) => {
            // a panicked strip may leave the arena mid-allocation; start the next one fresh
            *arena = bumpalo::Bump::new();
            let message = crate::audit::panic_hook::panic_message(payload.as_ref());
            serde_json::json!({ "panic": message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::strip_at_path;
    use std::path::Path;

    /// `await` as a name parses only at Script: a `.mts` path takes no script retry, while
    /// a `.ts` path keeps the fallback.
    #[test]
    fn a_path_takes_the_goal_its_extension_settles() {
        let source = "var await: number = 1;\n";
        let arena = bumpalo::Bump::new();
        assert!(matches!(
            strip_at_path(Path::new("w.mts"), source, &arena),
            Err(tsv_ts::StripError::Parse(_))
        ));
        assert!(matches!(
            strip_at_path(Path::new("W.MTS"), source, &arena),
            Err(tsv_ts::StripError::Parse(_))
        ));
        let stripped = strip_at_path(Path::new("w.ts"), source, &arena).expect("the script retry");
        assert_eq!(stripped.code, "var await         = 1;\n");
    }
}
