//! `imports_fixtures_update` — write each import fixture's `expected.json` from the oracle.
//!
//! The oracle is the canonical parser's tree walked by `imports_fixtures::oracle_json`, or,
//! for a fixture marked `oracle.txt`, the same walk over tsv's own parse wire
//! (`crate::imports_fixtures` module doc). Never tsv's collector.

use crate::cli::CliError;
use crate::imports_fixtures::{
    IMPORTS_FIXTURES_DIR, ImportsFixture, ImportsLanguage, ORACLE_MARKER, TSV_WIRE, oracle_json,
    render_expected, tsv_wire, walk_imports_fixtures,
};
use crate::sidecar::{self, SidecarError};
use argh::FromArgs;
use futures_util::StreamExt;
use std::path::Path;

/// Regenerate the import fixtures' `expected.json` from the oracle (the canonical parsers
/// through the sidecar, or tsv's parse wire for an `oracle.txt` fixture).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "imports_fixtures_update")]
pub struct ImportsFixturesUpdateCommand {
    /// fixture path patterns (case-insensitive substrings; multiple = OR)
    #[argh(positional)]
    patterns: Vec<String>,
}

impl ImportsFixturesUpdateCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let fixtures = walk_and_filter(&self.patterns)?;
        let rt = super::create_runtime();
        rt.block_on(async {
            let mut stream = super::spawn_work_stream(
                fixtures,
                super::ResultOrder::Input,
                |fixture| async move {
                    let outcome = update_one(&fixture).await;
                    (fixture, outcome)
                },
            );
            let (mut written, mut unchanged, mut failed) = (0, 0, 0);
            while let Some(joined) = stream.next().await {
                let (fixture, outcome) = super::task_result(joined, "imports-update")?;
                match outcome {
                    Ok(true) => {
                        written += 1;
                        println!("✓ wrote {}", fixture.relative_path);
                    }
                    Ok(false) => unchanged += 1,
                    Err(e) => {
                        failed += 1;
                        eprintln!("✗ {}: {e}", fixture.relative_path);
                    }
                }
            }
            println!("\n{written} written, {unchanged} unchanged, {failed} failed");
            if failed > 0 {
                Err(CliError::Failed)
            } else {
                Ok(())
            }
        })
    }
}

/// Walk the tree and keep the fixtures matching `patterns`, reporting a walk error or an
/// empty match.
pub(crate) fn walk_and_filter(patterns: &[String]) -> Result<Vec<ImportsFixture>, CliError> {
    let fixtures = walk_imports_fixtures(Path::new(IMPORTS_FIXTURES_DIR)).map_err(|e| {
        eprintln!("Error walking {IMPORTS_FIXTURES_DIR}: {e}");
        CliError::Failed
    })?;
    let fixtures: Vec<_> = fixtures
        .into_iter()
        .filter(|fixture| fixture.matches_filters(patterns))
        .collect();
    if fixtures.is_empty() {
        eprintln!("No import fixtures found matching: {}", patterns.join(" "));
        return Err(CliError::Failed);
    }
    Ok(fixtures)
}

/// Write one fixture's `expected.json`; `Ok(true)` when its bytes changed.
async fn update_one(fixture: &ImportsFixture) -> Result<bool, String> {
    let expected = render_expected(&oracle_expected(fixture).await?)?;
    let path = fixture.expected_path();
    if std::fs::read_to_string(&path).is_ok_and(|current| current == expected) {
        return Ok(false);
    }
    std::fs::write(&path, expected).map_err(|e| format!("writing {}: {e}", path.display()))?;
    Ok(true)
}

/// The oracle's records for `fixture`: the canonical parser's tree, or tsv's parse wire
/// for an `oracle.txt` fixture — refusing a rejected input with no marker, and a marker
/// whose input the canonical parser accepts.
///
/// A TypeScript input the canonical parser rejects at Module goal is parsed again at
/// Script goal, as `tsv_ts::parse_imports` retries it. An unmarked fixture's records are
/// also walked over tsv's own parse wire, at the goal the canonical parser accepted, and
/// must come out identical: the walk grades the two parsers' trees against each other
/// before it grades tsv's collector.
///
/// # Errors
///
/// The input can't be read, the marker is malformed or stale, the canonical parser
/// rejects an unmarked input, the sidecar fails, the walk does, or the walk over tsv's
/// wire disagrees with the walk over the canonical parser's.
pub(crate) async fn oracle_expected(fixture: &ImportsFixture) -> Result<serde_json::Value, String> {
    let source = fixture.read_input()?;
    let marker = fixture.tsv_wire_reason()?;
    let (goal, canonical) = canonical_parse(fixture.language, &source).await;
    match (marker, canonical) {
        (None, Ok(wire)) => {
            let oracle = oracle_json(fixture.language, &source, &wire)?;
            let ours_wire = tsv_wire(fixture.language, &source, goal).map_err(|e| {
                format!("tsv's parser rejects an input the canonical one accepts: {e}")
            })?;
            let cross = oracle_json(fixture.language, &source, &ours_wire)?;
            if cross != oracle {
                return Err(format!(
                    "the walk over tsv's parse wire disagrees with the walk over the canonical \
                     parser's:\n  canonical: {oracle}\n  tsv:       {cross}"
                ));
            }
            Ok(oracle)
        }
        (None, Err(SidecarError::ToolError { message })) => Err(format!(
            "the canonical parser rejects the input ({message}); fix it, or mark the \
             fixture with an {ORACLE_MARKER} (`{TSV_WIRE}` and a reason)"
        )),
        (Some(_), Ok(_)) => Err(format!(
            "stale {ORACLE_MARKER}: the canonical parser accepts the input — delete the marker"
        )),
        (Some(_), Err(SidecarError::ToolError { .. })) => {
            let wire = tsv_wire(fixture.language, &source, tsv_ts::Goal::Module)
                .or_else(|module_error| match fixture.language {
                    ImportsLanguage::TypeScript => {
                        tsv_wire(fixture.language, &source, tsv_ts::Goal::Script)
                            .map_err(|_| module_error)
                    }
                    ImportsLanguage::Svelte => Err(module_error),
                })
                .map_err(|e| format!("tsv's parser rejects the input too: {e}"))?;
            oracle_json(fixture.language, &source, &wire)
        }
        (_, Err(e)) => Err(super::describe_sidecar_error(&e)),
    }
}

/// The canonical parser's tree of `source`, and the goal it was read at: a TypeScript
/// input at Module goal, retried at Script goal when that is a parse error; a Svelte
/// component as Svelte's `parse` reads it (its scripts are modules).
async fn canonical_parse(
    language: ImportsLanguage,
    source: &str,
) -> (tsv_ts::Goal, Result<serde_json::Value, SidecarError>) {
    match language {
        ImportsLanguage::TypeScript => {
            let module = sidecar::parse_typescript_with_goal(source, tsv_ts::Goal::Module).await;
            if !matches!(module, Err(SidecarError::ToolError { .. })) {
                return (tsv_ts::Goal::Module, module);
            }
            let script = sidecar::parse_typescript_with_goal(source, tsv_ts::Goal::Script).await;
            match script {
                Ok(wire) => (tsv_ts::Goal::Script, Ok(wire)),
                // both rejected: the Module error is the one to report
                Err(_) => (tsv_ts::Goal::Module, module),
            }
        }
        ImportsLanguage::Svelte => (tsv_ts::Goal::Module, sidecar::parse_svelte(source).await),
    }
}
