use crate::cli::CliError;
use crate::fixtures::validation::parsed_input::{input_ast_paths, parse_input};
use crate::fixtures::{self, CanonicalParseError, WriteOutcome};
use argh::FromArgs;
use futures_util::StreamExt;

/// Regenerate expected.json (or expected_ours.json + expected_svelte.json) files.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fixtures_update_parsed")]
pub struct FixturesUpdateParsedCommand {
    /// list matching fixtures only (do not regenerate)
    #[argh(switch)]
    list: bool,

    /// fixture filter patterns (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

impl FixturesUpdateParsedCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if self.list {
            let (fixture_list, total_count) = super::walk_and_filter(&self.filters)?;
            super::print_fixture_list(&fixture_list, &self.filters, total_count);
            return Ok(());
        }
        let rt = super::create_runtime();
        rt.block_on(run(&self.filters))
    }
}

pub(super) async fn run(filters: &[String]) -> Result<(), CliError> {
    let (fixture_list, total_count) = super::walk_and_filter(filters)?;

    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let mut failed = 0;

    let matched_count = fixture_list.len();

    // Input order (`ResultOrder::Input`) so progress lines print deterministically.
    let mut results = super::spawn_work_stream(
        fixture_list,
        super::ResultOrder::Input,
        |fixture| async move {
            let result = generate_expected_fixture(&fixture).await;
            (fixture, result)
        },
    );

    while let Some(joined) = results.next().await {
        let (fixture, result) = super::task_result(joined, "fixture update")?;
        match result {
            Ok(WriteOutcome::Created) => {
                println!(
                    "✓ Created {}/{}",
                    fixture.relative_path,
                    expected_files_desc(&fixture)
                );
                created += 1;
            }
            Ok(WriteOutcome::Updated) => {
                println!(
                    "✓ Updated {}/{}",
                    fixture.relative_path,
                    expected_files_desc(&fixture)
                );
                updated += 1;
            }
            Ok(WriteOutcome::Unchanged) => {
                println!(
                    "- {}/{} up to date",
                    fixture.relative_path,
                    expected_files_desc(&fixture)
                );
                unchanged += 1;
            }
            Err(err) => {
                eprintln!("✗ Failed to generate {}: {}", fixture.relative_path, err);
                failed += 1;
            }
        }
    }

    if filters.is_empty() {
        println!(
            "\nSummary: {created} created, {updated} updated, {unchanged} unchanged, {failed} failed ({matched_count} fixtures)"
        );
    } else {
        println!(
            "\nSummary: {created} created, {updated} updated, {unchanged} unchanged, {failed} failed (matched {matched_count} of {total_count} fixtures)"
        );
    }

    if created > 0 || updated > 0 {
        println!("⚠️  Updated source of truth files (expected.json)");
    }

    if failed > 0 {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}

/// Which expected-JSON file(s) this fixture regenerates, for progress output.
fn expected_files_desc(fixture: &fixtures::Fixture) -> &'static str {
    if fixture.tsv_rejects_path().exists() {
        "expected_svelte.json"
    } else if uses_divergence_pattern(fixture) {
        "expected_ours.json + expected_svelte.json"
    } else {
        "expected.json"
    }
}

/// Whether this fixture regenerates the `expected_ours.json` + `expected_svelte.json`
/// pair rather than a lone `expected.json`.
///
/// The **suffix** decides it, not the files on disk: structure rule S13 makes the pair
/// mandatory in a svelte-divergence dir, so asking `has_expected_ours()` alone could only
/// ever regenerate a pair that already exists — a *new* divergence fixture would take the
/// `expected.json` path, where a canonical parser that rejects (the over-acceptance case
/// the pattern exists for) is a hard error instead of the `expected_svelte.json` error
/// marker. A `tsv_rejects.txt` fixture is neither, and its caller returns before this.
fn uses_divergence_pattern(fixture: &fixtures::Fixture) -> bool {
    fixture.has_expected_ours() || fixture.is_svelte_divergence()
}

/// Our parser's AST for a fixture input, in the exact bytes an `expected_ours.json` holds —
/// the same derivation the validator's parser phases compare against.
fn our_json(fixture: &fixtures::Fixture, source: &str) -> Result<String, String> {
    let arena = bumpalo::Bump::new();
    let parsed = parse_input(source, fixture.input_type(), fixture.goal(), &arena)
        .map_err(|e| format!("Our parser: {e}"))?;
    Ok(input_ast_paths(&parsed, source).ast_json_tabs)
}

async fn generate_expected_fixture(fixture: &fixtures::Fixture) -> Result<WriteOutcome, String> {
    // Read input file
    let source = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => return Err(e),
    };

    // tsv_rejects fixtures: tsv emits no AST, so generate ONLY expected_svelte.json
    // from the canonical parser (which must accept — a rejection means the divergence
    // is dead and the command fails loudly).
    if fixture.tsv_rejects_path().exists() {
        return generate_tsv_rejects_fixture(fixture, &source).await;
    }

    // Check if this fixture uses the divergence pattern
    if uses_divergence_pattern(fixture) {
        // Generate expected_ours.json + expected_svelte.json
        return generate_divergence_fixture(fixture, &source).await;
    }

    // Generate expected.json from the input type's canonical parser
    let input_type = fixture.input_type();
    let json = match fixtures::canonical_expected_json(&source, input_type, fixture.goal()).await {
        Ok(json) => json,
        Err(CanonicalParseError::Rejected(message)) => {
            return Err(format!(
                "{} parse error: {message}",
                input_type.language_name()
            ));
        }
        Err(CanonicalParseError::Sidecar(e)) => {
            return Err(fixtures::canonical_sidecar_failure(input_type, &e));
        }
        Err(CanonicalParseError::Unserializable(message)) => return Err(message),
    };

    fixtures::write_if_changed(&fixture.expected_path(), &json)
}

/// Generate `expected_svelte.json` for a `tsv_rejects.txt` fixture from the
/// canonical parser only (tsv over-rejects, so it emits no AST).
///
/// The canonical parser MUST accept the input — a rejection means both parsers
/// now agree (the divergence is dead), which fails the command with a hint to
/// convert the fixture to `input_invalid_*`. This is the self-heal the retired
/// ad-hoc Rust-test pins couldn't provide: a canonical-parser bump that starts
/// rejecting the input surfaces here instead of silently rotting.
async fn generate_tsv_rejects_fixture(
    fixture: &fixtures::Fixture,
    source: &str,
) -> Result<WriteOutcome, String> {
    let input_type = fixture.input_type();
    let svelte_json = match fixtures::canonical_expected_json(source, input_type, fixture.goal())
        .await
    {
        Ok(json) => json,
        Err(CanonicalParseError::Rejected(message)) => {
            return Err(format!(
                "canonical parser ({}) REJECTED a tsv_rejects input — the divergence is dead \
                    (both parsers reject now). Convert the fixture to input_invalid_*. Error: {message}",
                input_type.canonical_parser_name()
            ));
        }
        Err(CanonicalParseError::Sidecar(e)) => {
            return Err(fixtures::canonical_sidecar_failure(input_type, &e));
        }
        Err(CanonicalParseError::Unserializable(message)) => return Err(message),
    };

    fixtures::write_if_changed(&fixture.expected_svelte_path(), &svelte_json)
}

async fn generate_divergence_fixture(
    fixture: &fixtures::Fixture,
    source: &str,
) -> Result<WriteOutcome, String> {
    // Generate expected_ours.json from our parser.
    let our_json = match our_json(fixture, source) {
        Ok(json) => json,
        Err(e) => return Err(e),
    };

    // Generate expected_svelte.json from the external canonical parser
    // (Svelte, acorn-typescript, or parseCss), falling back to the error marker only
    // for the parser's own rejection — a sidecar fault writes nothing.
    let input_type = fixture.input_type();
    let svelte_json = match fixtures::canonical_expected_json(source, input_type, fixture.goal())
        .await
    {
        Ok(json) => json,
        Err(CanonicalParseError::Rejected(_)) => fixtures::EXPECTED_SVELTE_ERROR_JSON.to_string(),
        Err(CanonicalParseError::Sidecar(e)) => {
            return Err(fixtures::canonical_sidecar_failure(input_type, &e));
        }
        Err(CanonicalParseError::Unserializable(message)) => return Err(message),
    };

    let ours = match fixtures::write_if_changed(&fixture.expected_ours_path(), &our_json) {
        Ok(outcome) => outcome,
        Err(e) => return Err(format!("Failed to write expected_ours.json: {e}")),
    };
    let svelte = match fixtures::write_if_changed(&fixture.expected_svelte_path(), &svelte_json) {
        Ok(outcome) => outcome,
        Err(e) => {
            return Err(format!("Failed to write expected_svelte.json: {e}"));
        }
    };

    // Created when either file is new; Unchanged only when neither moved.
    match (ours, svelte) {
        (WriteOutcome::Unchanged, WriteOutcome::Unchanged) => Ok(WriteOutcome::Unchanged),
        (WriteOutcome::Created, _) | (_, WriteOutcome::Created) => Ok(WriteOutcome::Created),
        _ => Ok(WriteOutcome::Updated),
    }
}
