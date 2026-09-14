use crate::cli::CliError;
use crate::fixtures::validation::parsed_input::{input_ast_paths, parse_input};
use crate::fixtures::{self, CanonicalParseError, ExpectedVariantPin, FixtureFiles, WriteOutcome};
use argh::FromArgs;
use futures_util::StreamExt;

/// Regenerate expected.json (or expected_ours.json + expected_svelte.json) files, plus
/// every variant parse pin (`expected_<stem>.json`) a fixture holds.
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
            Ok((WriteOutcome::Created, files)) => {
                println!("✓ Created {}/{files}", fixture.relative_path);
                created += 1;
            }
            Ok((WriteOutcome::Updated, files)) => {
                println!("✓ Updated {}/{files}", fixture.relative_path);
                updated += 1;
            }
            Ok((WriteOutcome::Unchanged, files)) => {
                println!("- {}/{files} up to date", fixture.relative_path);
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
fn expected_files_desc(fixture: &fixtures::Fixture, files: &FixtureFiles) -> String {
    let main = if fixture.tsv_rejects_path().exists() {
        "expected_svelte.json"
    } else if uses_divergence_pattern(fixture) {
        "expected_ours.json + expected_svelte.json"
    } else {
        "expected.json"
    };
    files
        .expected_variant
        .iter()
        .fold(main.to_string(), |desc, entry| {
            format!("{desc} + {}", entry.pin)
        })
}

/// The outcome of two writes read as one: created if either file is new, unchanged only
/// when neither moved.
fn combine_outcomes(a: WriteOutcome, b: WriteOutcome) -> WriteOutcome {
    match (a, b) {
        (WriteOutcome::Unchanged, WriteOutcome::Unchanged) => WriteOutcome::Unchanged,
        (WriteOutcome::Created, _) | (_, WriteOutcome::Created) => WriteOutcome::Created,
        _ => WriteOutcome::Updated,
    }
}

/// Regenerate every variant parse pin (`expected_<stem>.json`) the fixture holds from the
/// canonical parser's AST of its `<stem><ext>` sibling — the P4 oracle, the same derivation
/// `expected.json` takes. A pin whose variant is missing is S24's error, restated here so
/// the updater never writes a file the validator would then refuse; a canonical rejection
/// is an error too (the pin holds an AST, never the rejection marker).
///
/// To create a pin, add an empty `expected_<stem>.json` beside the variant and run this
/// command — the empty file is "outdated", and this fills it.
async fn generate_variant_pins(
    fixture: &fixtures::Fixture,
    files: &FixtureFiles,
) -> Result<WriteOutcome, String> {
    let mut outcome = WriteOutcome::Unchanged;
    for ExpectedVariantPin { pin, variant } in &files.expected_variant {
        let variant_path = fixture.path.join(variant);
        if !variant_path.exists() {
            return Err(format!(
                "{pin} has no {variant} to pin (S24) — add the variant or delete the pin"
            ));
        }
        let source = fixtures::read_file(&variant_path)?;
        let json = canonical_json_or_error(fixture, &source, &format!("{variant}: ")).await?;
        let written = fixtures::write_if_changed(&fixture.path.join(pin), &json)
            .map_err(|e| format!("Failed to write {pin}: {e}"))?;
        outcome = combine_outcomes(outcome, written);
    }
    Ok(outcome)
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

/// Regenerate a fixture's expected files — the input's own, then every variant pin — and
/// name them for the progress line, off one directory scan.
async fn generate_expected_fixture(
    fixture: &fixtures::Fixture,
) -> Result<(WriteOutcome, String), String> {
    let files = FixtureFiles::scan(fixture);
    let input = generate_input_expected(fixture).await?;
    let pins = generate_variant_pins(fixture, &files).await?;
    Ok((
        combine_outcomes(input, pins),
        expected_files_desc(fixture, &files),
    ))
}

/// The input's own expected file(s): `expected_svelte.json` alone for a `tsv_rejects.txt`
/// fixture, the `expected_ours.json` + `expected_svelte.json` pair for a divergence
/// fixture, `expected.json` otherwise.
async fn generate_input_expected(fixture: &fixtures::Fixture) -> Result<WriteOutcome, String> {
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
    let json = canonical_json_or_error(fixture, &source, "").await?;
    fixtures::write_if_changed(&fixture.expected_path(), &json)
}

/// The canonical parser's AST of `source` in `expected*.json` bytes, or the message the
/// updater fails with: a rejection (the parser named by language — an AST file is never
/// written from one), a sidecar fault, or an unserializable AST. `subject` prefixes the
/// rejection so a variant's names itself; the input passes `""`.
async fn canonical_json_or_error(
    fixture: &fixtures::Fixture,
    source: &str,
    subject: &str,
) -> Result<String, String> {
    let input_type = fixture.input_type();
    match fixtures::canonical_expected_json(source, input_type, fixture.goal()).await {
        Ok(json) => Ok(json),
        Err(CanonicalParseError::Rejected(message)) => Err(format!(
            "{subject}{} parse error: {message}",
            input_type.language_name()
        )),
        Err(CanonicalParseError::Sidecar(e)) => {
            Err(fixtures::canonical_sidecar_failure(input_type, &e))
        }
        Err(CanonicalParseError::Unserializable(message)) => Err(message),
    }
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

    Ok(combine_outcomes(ours, svelte))
}
