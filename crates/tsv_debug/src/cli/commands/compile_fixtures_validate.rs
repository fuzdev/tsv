use crate::cli::CliError;
use crate::compile_fixtures::{
    COMPILE_FIXTURES_DIR, CompileFixture, EXPECTED_SERVER_JS, walk_compile_fixtures,
    with_trailing_newline,
};
use crate::deno::{self, SvelteGenerate};
use crate::diff::{DiffOptions, diff_to_string};
use argh::FromArgs;
use std::path::Path;
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_svelte_compile::{CompileError, CompileOptions, canonicalize_js, compile};

/// Validate compile fixtures against the canonical Svelte compiler.
///
/// Per fixture, three checks:
///
/// (a) **Oracle freshness** — `canonicalize_js(oracle(input.svelte).js)` must equal
///     the committed `expected_server.js`, and the oracle CSS must match
///     `expected.css` (both absent counts as a match). Catches a stale expectation
///     after an oracle (Svelte pin) or canonicalizer change.
/// (b) **Ours** — `tsv_svelte_compile::compile` → canonicalize vs expected. Today
///     every fixture reports `unimplemented` (the walking skeleton) — a distinct,
///     expected status, NOT a failure; the gate flips when codegen lands.
/// (c) **Expected idempotence** — the committed `expected_server.js` must be a
///     `canonicalize_js` fixed point (it reparses by construction — canonicalize
///     self-validates).
///
/// Exits non-zero on (a)/(c) failures only, for now.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_fixtures_validate")]
pub struct CompileFixturesValidateCommand {
    /// list matching fixtures without validating
    #[argh(switch)]
    list: bool,

    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,

    /// filter patterns (multiple = OR, case-insensitive substring)
    #[argh(positional)]
    patterns: Vec<String>,
}

/// One fixture's validation outcome (the `--json` row).
#[derive(serde::Serialize)]
struct FixtureReport {
    fixture: String,
    /// Check (a): canonicalized oracle output matches the committed expectations.
    oracle_fresh: bool,
    /// Check (c): the committed expected_server.js is a canonicalize fixed point.
    expected_idempotent: bool,
    /// Check (b): "unimplemented" | "parity" | "mismatch" | "error".
    ours_status: &'static str,
    /// Human-readable failure details (empty when everything passed).
    errors: Vec<String>,
}

impl CompileFixturesValidateCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let root = Path::new(COMPILE_FIXTURES_DIR);
        if !root.exists() {
            eprintln!("Error: compile fixtures directory not found: {COMPILE_FIXTURES_DIR}");
            return Err(CliError::Failed);
        }
        let all = walk_compile_fixtures(root).map_err(|e| {
            eprintln!("Error walking compile fixtures: {e}");
            CliError::Failed
        })?;
        let total = all.len();
        let fixtures: Vec<_> = all
            .into_iter()
            .filter(|f| f.matches_filters(&self.patterns))
            .collect();
        if fixtures.is_empty() {
            if self.patterns.is_empty() {
                eprintln!("No compile fixtures found");
            } else {
                eprintln!(
                    "No compile fixtures found matching: {}",
                    self.patterns.join(" ")
                );
            }
            return Err(CliError::Failed);
        }

        if self.list {
            println!("Found compile fixtures:");
            for fixture in &fixtures {
                println!("  {}", fixture.relative_path);
            }
            if self.patterns.is_empty() {
                println!("\nTotal: {}", fixtures.len());
            } else {
                println!("\nMatched: {} of {total} fixtures", fixtures.len());
            }
            return Ok(());
        }

        let rt = super::create_runtime();
        rt.block_on(self.validate_all(fixtures))
    }

    async fn validate_all(&self, fixtures: Vec<CompileFixture>) -> Result<(), CliError> {
        let mut reports = Vec::with_capacity(fixtures.len());
        for fixture in &fixtures {
            reports.push(validate_fixture(fixture).await);
        }

        let gating_failures = reports
            .iter()
            .filter(|r| !r.oracle_fresh || !r.expected_idempotent)
            .count();
        let unimplemented = reports
            .iter()
            .filter(|r| r.ours_status == "unimplemented")
            .count();
        let parity = reports.iter().filter(|r| r.ours_status == "parity").count();

        if self.json {
            #[derive(serde::Serialize)]
            struct Summary {
                total: usize,
                gating_failures: usize,
                ours_parity: usize,
                ours_unimplemented: usize,
                fixtures: Vec<FixtureReport>,
            }
            let summary = Summary {
                total: reports.len(),
                gating_failures,
                ours_parity: parity,
                ours_unimplemented: unimplemented,
                fixtures: reports,
            };
            match to_json_with_tabs(&summary) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("Error serializing report: {e}");
                    return Err(CliError::Failed);
                }
            }
        } else {
            for report in &reports {
                let ok = report.oracle_fresh && report.expected_idempotent;
                let mark = if ok { "✓" } else { "✗" };
                println!("{mark} {} [ours: {}]", report.fixture, report.ours_status);
                for error in &report.errors {
                    eprintln!("  {error}");
                }
            }
            println!(
                "\n{} fixtures: {} gating failure(s), ours: {parity} parity / {unimplemented} unimplemented",
                reports.len(),
                gating_failures
            );
        }

        if gating_failures > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

/// Run the three checks for one fixture.
async fn validate_fixture(fixture: &CompileFixture) -> FixtureReport {
    let mut errors = Vec::new();
    let name = fixture.relative_path.clone();

    let input = match std::fs::read_to_string(fixture.input_path()) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("cannot read input.svelte: {e}"));
            return FixtureReport {
                fixture: name,
                oracle_fresh: false,
                expected_idempotent: false,
                ours_status: "error",
                errors,
            };
        }
    };
    let expected_js = match std::fs::read_to_string(fixture.expected_server_js_path()) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("cannot read {EXPECTED_SERVER_JS}: {e}"));
            String::new()
        }
    };
    let expected_css = std::fs::read_to_string(fixture.expected_css_path()).ok();

    // (c) Expected idempotence — pure Rust, no sidecar.
    let expected_idempotent = if expected_js.is_empty() {
        false
    } else {
        match canonicalize_js(&expected_js) {
            Ok(again) if again == expected_js => true,
            Ok(_) => {
                errors.push(format!(
                    "{EXPECTED_SERVER_JS} is not a canonicalize fixed point — regenerate via compile_fixture_init"
                ));
                false
            }
            Err(e) => {
                errors.push(format!("{EXPECTED_SERVER_JS} fails to canonicalize: {e}"));
                false
            }
        }
    };

    // (a) Oracle freshness — sidecar-dependent.
    let oracle_fresh = match deno::svelte_compile(&input, SvelteGenerate::Server, false).await {
        Ok(compiled) => {
            let mut fresh = true;
            match canonicalize_js(&compiled.js) {
                Ok(canonical) => {
                    if canonical != expected_js {
                        fresh = false;
                        errors.push(format!(
                            "{EXPECTED_SERVER_JS} is stale (oracle output differs) — regenerate via compile_fixture_init"
                        ));
                        errors.push(diff_to_string(
                            &expected_js,
                            &canonical,
                            &DiffOptions::freshness(),
                        ));
                    }
                }
                Err(e) => {
                    fresh = false;
                    errors.push(format!("could not canonicalize oracle output: {e}"));
                }
            }
            let oracle_css = compiled.css.as_deref().map(with_trailing_newline);
            if oracle_css != expected_css {
                fresh = false;
                errors.push(match (&oracle_css, &expected_css) {
                    (Some(_), None) => "expected.css missing (oracle produces css)".to_string(),
                    (None, Some(_)) => "expected.css is stale (oracle produces none)".to_string(),
                    _ => "expected.css is stale (oracle css differs)".to_string(),
                });
            }
            fresh
        }
        Err(e) => {
            errors.push(format!("oracle compile failed: {e} (hint: {})", e.hint()));
            false
        }
    };

    // (b) Ours — reported, not gating, until codegen lands.
    // TODO: when M1 lands, a "mismatch"/"error" here becomes a gating failure
    // and "unimplemented" becomes one too (every fixture must compile).
    let ours_status = match compile(&input, &CompileOptions::default()) {
        Ok(ours) => match canonicalize_js(&ours.js) {
            Ok(canonical) if canonical == expected_js => "parity",
            Ok(_) => {
                errors.push("ours differs from expected (non-gating until M1)".to_string());
                "mismatch"
            }
            Err(e) => {
                errors.push(format!("could not canonicalize our output: {e}"));
                "error"
            }
        },
        Err(CompileError::Codegen) => "unimplemented",
        Err(e) => {
            errors.push(format!("tsv compile failed: {e}"));
            "error"
        }
    };

    FixtureReport {
        fixture: name,
        oracle_fresh,
        expected_idempotent,
        ours_status,
        errors,
    }
}
