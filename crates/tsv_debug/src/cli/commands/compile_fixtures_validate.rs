use crate::cli::CliError;
use crate::cli::commands::print_json_tabs;
use crate::compile_fixtures::{
    COMPILE_FIXTURES_DIR, CompileFixture, EXPECTED_SERVER_JS, walk_compile_fixtures,
    with_trailing_newline,
};
use crate::deno::{self, SvelteGenerate};
use crate::diff::{DiffOptions, diff_to_string};
use argh::FromArgs;
use futures_util::StreamExt;
use std::path::Path;
use tsv_svelte_compile::{CompileOptions, canonicalize_js, compare_canonical, compile};

/// Validate compile fixtures against the canonical Svelte compiler.
///
/// Per fixture, three checks — all gating:
///
/// (a) **Oracle freshness** — `canonicalize_js(oracle(input.svelte).js)` must equal
///     the committed `expected_server.js`, and the oracle CSS must match
///     `expected.css` (both absent counts as a match). Catches a stale expectation
///     after an oracle (Svelte pin) or canonicalizer change.
/// (b) **Ours** — `tsv_svelte_compile::compile` must succeed and its canonicalized
///     JS + CSS must equal the committed expectations (`parity`; anything else —
///     `mismatch` / `error` — fails the run).
/// (c) **Expected idempotence** — the committed `expected_server.js` must be a
///     `canonicalize_js` fixed point (it reparses by construction — canonicalize
///     self-validates).
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

/// Check (b)'s verdict on tsv's own compile. Serializes as [`Self::label`], so the `--json`
/// row and the human report spell it one way.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OursStatus {
    /// The canonicalized JS and the CSS both match the committed expectations.
    Parity,
    /// tsv compiled, but its JS or CSS differs from the expectations.
    Mismatch,
    /// tsv failed to compile, or its output failed to canonicalize.
    Error,
}

impl OursStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Parity => "parity",
            Self::Mismatch => "mismatch",
            Self::Error => "error",
        }
    }
}

impl serde::Serialize for OursStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.label())
    }
}

/// One fixture's validation outcome (the `--json` row).
#[derive(serde::Serialize)]
struct FixtureReport {
    fixture: String,
    /// Check (a): canonicalized oracle output matches the committed expectations.
    oracle_fresh: bool,
    /// Check (c): the committed expected_server.js is a canonicalize fixed point.
    expected_idempotent: bool,
    /// Check (b): tsv's own compile against the expectations.
    ours_status: OursStatus,
    /// Human-readable failure details (empty when everything passed).
    errors: Vec<String>,
}

impl FixtureReport {
    /// All three checks gate: oracle freshness, expected idempotence, AND ours parity.
    fn passed(&self) -> bool {
        self.oracle_fresh && self.expected_idempotent && self.ours_status == OursStatus::Parity
    }
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
        // Validate fixtures concurrently on the bulk sidecar pool, in input
        // order — each fixture's failure details are buffered into its report,
        // so concurrent fixtures can't interleave output and the printed order
        // stays deterministic.
        let count = fixtures.len();
        let mut stream =
            super::spawn_work_stream(fixtures, super::ResultOrder::Input, |fixture| async move {
                validate_fixture(&fixture).await
            });
        let mut reports = Vec::with_capacity(count);
        while let Some(joined) = stream.next().await {
            reports.push(super::task_result(joined, "compile-validation")?);
        }

        let gating_failures = reports.iter().filter(|r| !r.passed()).count();
        let parity = reports
            .iter()
            .filter(|r| r.ours_status == OursStatus::Parity)
            .count();

        if self.json {
            #[derive(serde::Serialize)]
            struct Summary {
                total: usize,
                gating_failures: usize,
                ours_parity: usize,
                fixtures: Vec<FixtureReport>,
            }
            let summary = Summary {
                total: reports.len(),
                gating_failures,
                ours_parity: parity,
                fixtures: reports,
            };
            print_json_tabs(&summary, CliError::Failed)?;
        } else {
            for report in &reports {
                let mark = if report.passed() { "✓" } else { "✗" };
                println!(
                    "{mark} {} [ours: {}]",
                    report.fixture,
                    report.ours_status.label()
                );
                for error in &report.errors {
                    eprintln!("  {error}");
                }
            }
            println!(
                "\n{} fixtures: {} gating failure(s), {parity} ours-parity",
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
                ours_status: OursStatus::Error,
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
    // A missing expected.css means an unstyled component; one that exists but cannot be read is
    // an error, not "unstyled" — that reading would pass a CSS check it never ran.
    let expected_css = match std::fs::read_to_string(fixture.expected_css_path()) {
        Ok(css) => Some(css),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            errors.push(format!(
                "cannot read {}: {e}",
                fixture.expected_css_path().display()
            ));
            None
        }
    };

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
            let hint = e.hint();
            if hint.is_empty() {
                errors.push(format!("oracle compile failed: {e}"));
            } else {
                errors.push(format!("oracle compile failed: {e} (hint: {hint})"));
            }
            false
        }
    };

    // (b) Ours — the compiler must reproduce the expectations (gating): the
    // canonicalized JS must equal expected_server.js and the CSS must match
    // expected.css.
    let ours_status = match compile(&input, &CompileOptions::default()) {
        Ok(ours) => match canonicalize_js(&ours.js) {
            Ok(canonical) => {
                // Parity tolerates a comment-POSITION difference (tsv's placement vs
                // the oracle's, recorded in expected_server.js) — same code, same
                // comment sequence, no bundler annotation. See `compare_canonical`.
                let js_parity = compare_canonical(&canonical, &expected_js).is_parity();
                if !js_parity {
                    errors.push("ours differs from expected_server.js".to_string());
                    errors.push(diff_to_string(
                        &canonical,
                        &expected_js,
                        &DiffOptions::compile_compare(),
                    ));
                }
                let ours_css = ours.css.as_deref().map(with_trailing_newline);
                let css_parity = ours_css == expected_css;
                if !css_parity {
                    errors.push("ours css differs from expected.css".to_string());
                }
                if js_parity && css_parity {
                    OursStatus::Parity
                } else {
                    OursStatus::Mismatch
                }
            }
            Err(e) => {
                errors.push(format!("could not canonicalize our output: {e}"));
                OursStatus::Error
            }
        },
        Err(e) => {
            errors.push(format!("tsv compile failed: {e}"));
            OursStatus::Error
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
