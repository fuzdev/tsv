//! `loc_wires` — both parse wires of every fixture document, for the `loc` cross-grade.
//!
//! tsv's `loc` has two implementations of one definition: the Rust writers, which emit it
//! on the loc-bearing wire, and the shipped `crates/tsv_wasm/npm/locations.js`, which
//! reconstructs it in JS from the span-only wire plus the source. Neither carries a
//! hand-written expectation, so each is the other's drift check — and since the fixtures
//! pin the span-only wire, the cross-grade is the only `deno task check` leg that grades
//! `loc` at all.
//!
//! This command is the Rust half of that leg (`deno task check:loc`,
//! `scripts/check_loc.ts`): one process that parses every fixture document once and streams
//! one NDJSON record per document to stdout —
//! `{"path", "language", "source", "loc", "span"}`, the two wires embedded verbatim — so
//! the JS half pays no per-file process spawn. `language` is the line rule's language
//! (`svelte` / `typescript` / `css`; a `.svelte.ts` module is `typescript`), and `source` is
//! the exact string parsed, a leading BOM included (reading the file in JS would lose it:
//! a `TextDecoder` strips one by default).
//!
//! The documents are every parseable source file in each fixture directory, at the
//! fixture's `goal` marker's goal — `loc` needs no oracle, so a format variant grades as
//! well as an input does. They come in two claims, and a document tsv fails to parse is
//! held to its claim:
//!
//! - **required** — the fixture's `input.*` and every variant the fixture tree asks tsv's
//!   own formatter to read (`unformatted_*`, `unformatted_ours_*`, `prettier_variant_*`,
//!   `variant_*`, `divergent_variant_*`, which every `expected_<stem>.json` pin's variant is
//!   one of). The fixture gate already requires tsv to accept each, so a parse failure here
//!   is reported on stderr and fails the run.
//! - **prettier-side** — the forms only prettier's side of the fixture claims anything
//!   about: `output_prettier.*`, `unformatted_prettier_*` and the `prettier_intermediate*_*`
//!   kinds. Prettier's output is not always valid source, so the tree makes no claim tsv
//!   parses these; one tsv rejects is streamed as a `{"path", "rejected"}` record, which
//!   the JS half grades against its ledger of the exact prettier-side documents tsv
//!   rejects (`scripts/check_loc_rejects.txt`).
//!
//! Not graded at all, each by a declaration the fixture tree makes: an `input_invalid_*`
//! file (it must fail both parsers) and the input of a `tsv_rejects.txt` fixture (tsv
//! rejects it on purpose, and such a fixture holds no format variant).

use crate::cli::CliError;
use crate::fixtures::validation::parsed_input::{ParsedInput, parse_input};
use crate::fixtures::{self, Fixture, FixtureFiles, InputType};
use argh::FromArgs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Stream both parse wires of every fixture document as NDJSON (the `loc` cross-grade's input).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "loc_wires")]
pub struct LocWiresCommand {
    /// fixture tree to walk (default: tests/fixtures)
    #[argh(positional, default = "PathBuf::from(\"tests/fixtures\")")]
    pub root: PathBuf,
}

impl LocWiresCommand {
    /// Run the command.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the tree can't be walked, a document can't be
    /// read or parsed, or stdout can't be written.
    pub fn run(self) -> Result<(), CliError> {
        let fixtures = fixtures::walk_fixtures(&self.root).map_err(|e| {
            eprintln!("Error: {e}");
            CliError::Failed
        })?;
        let stdout = std::io::stdout();
        let mut out = std::io::BufWriter::new(stdout.lock());
        let mut failed = false;
        for fixture in &fixtures {
            if fixture.tsv_rejects_path().exists() {
                continue;
            }
            let input_type = fixture.input_type();
            let goal = fixture.goal();
            for (name, claim) in documents(fixture) {
                let path = fixture.path.join(&name);
                let relative = format!("{}/{name}", fixture.relative_path);
                let line = match record(&path, input_type, goal) {
                    Ok(line) => line,
                    Err(Failure::Parse(e)) if claim == Claim::PrettierSide => {
                        let mut line = b",\"rejected\":".to_vec();
                        line.extend_from_slice(json_string(&e).as_bytes());
                        line.push(b'}');
                        line
                    }
                    Err(Failure::Parse(e) | Failure::Read(e)) => {
                        eprintln!("{relative}: {e}");
                        failed = true;
                        continue;
                    }
                };
                let written = write!(out, "{{\"path\":")
                    .and_then(|()| out.write_all(json_string(&relative).as_bytes()))
                    .and_then(|()| out.write_all(&line))
                    .and_then(|()| out.write_all(b"\n"));
                if let Err(e) = written {
                    eprintln!("Error: writing stdout: {e}");
                    return Err(CliError::Failed);
                }
            }
        }
        if let Err(e) = out.flush() {
            eprintln!("Error: writing stdout: {e}");
            return Err(CliError::Failed);
        }
        if failed {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

/// What the fixture tree claims about a document's parse (module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Claim {
    /// tsv must parse it; a failure fails the run.
    Required,
    /// Only prettier's side claims anything; a tsv rejection is streamed and counted.
    PrettierSide,
}

/// Every graded document in `fixture`'s directory, by filename, with its claim.
fn documents(fixture: &Fixture) -> Vec<(String, Claim)> {
    let files = FixtureFiles::scan(fixture);
    let mut documents = vec![(fixture.input_file.clone(), Claim::Required)];
    for list in [
        &files.unformatted,
        &files.unformatted_ours,
        &files.prettier_variant,
        &files.variant,
        &files.divergent_variant,
    ] {
        documents.extend(list.iter().map(|name| (name.clone(), Claim::Required)));
    }
    let [(intermediate, _), (to_variant, _), (to_divergent, _)] = files.intermediate_kinds();
    for list in [
        &files.unformatted_prettier,
        intermediate,
        to_variant,
        to_divergent,
    ] {
        documents.extend(list.iter().map(|name| (name.clone(), Claim::PrettierSide)));
    }
    if fixture.output_prettier_path().exists() {
        documents.push((
            fixture.output_prettier_filename().to_string(),
            Claim::PrettierSide,
        ));
    }
    documents
}

/// Why a document produced no wire record.
enum Failure {
    /// The file could not be read — fails the run whatever the claim.
    Read(String),
    /// tsv rejected the source.
    Parse(String),
}

/// The record for one document after its `path`: `,"language":…,"source":…,"loc":…,"span":…}`.
fn record(path: &Path, input_type: InputType, goal: tsv_ts::Goal) -> Result<Vec<u8>, Failure> {
    let source = fixtures::read_file(path).map_err(Failure::Read)?;
    let arena = bumpalo::Bump::new();
    let parsed = parse_input(&source, input_type, goal, &arena).map_err(Failure::Parse)?;
    let (language, loc, span) = match &parsed {
        ParsedInput::Svelte(ast) => (
            "svelte",
            tsv_svelte::convert_ast_json_bytes(ast, &source),
            tsv_svelte::convert_ast_json_bytes_no_locations(ast, &source),
        ),
        ParsedInput::Ts(ast) => (
            "typescript",
            tsv_ts::convert_ast_json_bytes(ast, &source),
            tsv_ts::convert_ast_json_bytes_no_locations(ast, &source),
        ),
        ParsedInput::Css(ast) => (
            "css",
            tsv_css::convert_ast_json_bytes(ast, &source),
            tsv_css::convert_ast_json_bytes_no_locations(ast, &source),
        ),
    };
    let mut line = Vec::with_capacity(loc.len() + span.len() + source.len() + 64);
    line.extend_from_slice(b",\"language\":\"");
    line.extend_from_slice(language.as_bytes());
    line.extend_from_slice(b"\",\"source\":");
    line.extend_from_slice(json_string(&source).as_bytes());
    line.extend_from_slice(b",\"loc\":");
    line.extend_from_slice(&loc);
    line.extend_from_slice(b",\"span\":");
    line.extend_from_slice(&span);
    line.push(b'}');
    Ok(line)
}

/// `s` as a JSON string literal.
fn json_string(s: &str) -> String {
    // serializing a `str` cannot fail
    serde_json::to_string(s).unwrap_or_default()
}
