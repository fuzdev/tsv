use crate::cli::CliError;
use crate::deno::{parse_css, parse_svelte, parse_typescript};
use crate::fixtures;
use crate::fixtures::InputType;
use argh::FromArgs;
use futures_util::StreamExt;
use tsv_cli::json_utils::to_json_with_tabs;

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
        let rt = super::create_runtime();
        rt.block_on(run(self.list, &self.filters))
    }
}

async fn run(list_only: bool, filters: &[String]) -> Result<(), CliError> {
    let (fixture_list, total_count) = super::walk_and_filter(filters)?;

    if list_only {
        super::print_fixture_list(&fixture_list, filters, total_count);
        return Ok(());
    }

    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let mut failed = 0;

    let matched_count = fixture_list.len();

    // Fixture order (`ResultOrder::Fixture`) so progress lines print deterministically.
    let mut results = super::spawn_fixture_stream(
        fixture_list,
        super::ResultOrder::Fixture,
        |fixture| async move {
            let result = generate_expected_fixture(&fixture).await;
            (fixture, result)
        },
    );

    while let Some(joined) = results.next().await {
        let (fixture, result) = super::task_result(joined, "update")?;
        match result {
            FixtureResult::Created => {
                if fixture.has_expected_ours() {
                    println!(
                        "✓ Created {}/expected_ours.json + expected_svelte.json",
                        fixture.relative_path
                    );
                } else {
                    println!("✓ Created {}/expected.json", fixture.relative_path);
                }
                created += 1;
            }
            FixtureResult::Updated => {
                if fixture.has_expected_ours() {
                    println!(
                        "✓ Updated {}/expected_ours.json + expected_svelte.json",
                        fixture.relative_path
                    );
                } else {
                    println!("✓ Updated {}/expected.json", fixture.relative_path);
                }
                updated += 1;
            }
            FixtureResult::Unchanged => {
                if fixture.has_expected_ours() {
                    println!(
                        "- {}/expected_ours.json + expected_svelte.json are up to date",
                        fixture.relative_path
                    );
                } else {
                    println!("- {}/expected.json is up to date", fixture.relative_path);
                }
                unchanged += 1;
            }
            FixtureResult::Failed(err) => {
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

enum FixtureResult {
    Created,
    Updated,
    Unchanged,
    Failed(String),
}

async fn generate_expected_fixture(fixture: &fixtures::Fixture) -> FixtureResult {
    // Read input file
    let source = match fixtures::read_file(&fixture.input_path()) {
        Ok(s) => s,
        Err(e) => return FixtureResult::Failed(e),
    };

    // Check if this fixture uses the divergence pattern
    if fixture.has_expected_ours() {
        // Generate expected_ours.json + expected_svelte.json
        return generate_divergence_fixture(fixture, &source).await;
    }

    // Generate expected.json from appropriate parser based on input type
    let json = match fixture.input_type() {
        InputType::SvelteTs | InputType::TypeScript => {
            // TypeScript and SvelteTs fixtures use acorn+typescript parser
            match parse_typescript(&source).await {
                Ok(ast) => match to_json_with_tabs(&ast) {
                    Ok(json) => format!("{json}\n"),
                    Err(e) => {
                        return FixtureResult::Failed(format!(
                            "Failed to serialize TypeScript AST: {e}"
                        ));
                    }
                },
                Err(e) => return FixtureResult::Failed(format!("TypeScript parse error: {e}")),
            }
        }
        InputType::Css => {
            // CSS fixtures use Svelte's parseCss (external canonical source)
            match parse_css(&source).await {
                Ok(ast) => match to_json_with_tabs(&ast) {
                    Ok(json) => format!("{json}\n"),
                    Err(e) => {
                        return FixtureResult::Failed(format!("Failed to serialize CSS AST: {e}"));
                    }
                },
                Err(e) => return FixtureResult::Failed(format!("CSS parse error: {e}")),
            }
        }
        InputType::Svelte => {
            // Svelte fixtures use Svelte's parser
            match parse_svelte(&source).await {
                Ok(ast) => match to_json_with_tabs(&ast) {
                    Ok(json) => format!("{json}\n"),
                    Err(e) => {
                        return FixtureResult::Failed(format!(
                            "Failed to serialize Svelte AST: {e}"
                        ));
                    }
                },
                Err(e) => return FixtureResult::Failed(format!("Svelte parse error: {e}")),
            }
        }
    };

    let expected_path = fixture.expected_path();

    // Check if expected.json exists and compare
    let existing = fixtures::read_file(&expected_path).ok();

    if Some(&json) == existing.as_ref() {
        FixtureResult::Unchanged
    } else if existing.is_none() {
        match fixtures::write_file(&expected_path, &json) {
            Ok(()) => FixtureResult::Created,
            Err(e) => FixtureResult::Failed(e),
        }
    } else {
        match fixtures::write_file(&expected_path, &json) {
            Ok(()) => FixtureResult::Updated,
            Err(e) => FixtureResult::Failed(e),
        }
    }
}

async fn generate_divergence_fixture(fixture: &fixtures::Fixture, source: &str) -> FixtureResult {
    // Generate expected_ours.json from our parser.
    // Parse directly and serialize the struct (not via serde_json::Value) to
    // preserve field order. Dispatch on input type (mirrors generate_expected_fixture).
    let our_json = match fixture.input_type() {
        InputType::SvelteTs | InputType::TypeScript => {
            let arena = bumpalo::Bump::new();
            let ast = match tsv_ts::parse(source, &arena) {
                Ok(ast) => ast,
                Err(e) => return FixtureResult::Failed(format!("Our parser error: {e:?}")),
            };
            let json_value = tsv_ts::convert_ast_json(&ast, source);
            match to_json_with_tabs(&json_value) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    return FixtureResult::Failed(format!("Failed to serialize our AST: {e}"));
                }
            }
        }
        InputType::Css => {
            let arena = bumpalo::Bump::new();
            let ast = match tsv_css::parse(source, &arena) {
                Ok(ast) => ast,
                Err(e) => return FixtureResult::Failed(format!("Our parser error: {e:?}")),
            };
            let json_value = tsv_css::convert_ast_json(&ast, source);
            match to_json_with_tabs(&json_value) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    return FixtureResult::Failed(format!("Failed to serialize our AST: {e}"));
                }
            }
        }
        InputType::Svelte => {
            let arena = bumpalo::Bump::new();
            let ast = match tsv_svelte::parse(source, &arena) {
                Ok(ast) => ast,
                Err(e) => return FixtureResult::Failed(format!("Our parser error: {e:?}")),
            };
            let json_value = tsv_svelte::convert_ast_json(&ast, source);
            match to_json_with_tabs(&json_value) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    return FixtureResult::Failed(format!("Failed to serialize our AST: {e}"));
                }
            }
        }
    };

    // Generate expected_svelte.json from the external canonical parser
    // (Svelte, acorn-typescript, or parseCss), falling back to the error marker.
    let svelte_json = match fixture.input_type() {
        InputType::SvelteTs | InputType::TypeScript => match parse_typescript(source).await {
            Ok(ast) => match to_json_with_tabs(&ast) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    return FixtureResult::Failed(format!(
                        "Failed to serialize TypeScript AST: {e}"
                    ));
                }
            },
            Err(_) => fixtures::EXPECTED_SVELTE_ERROR_JSON.to_string(),
        },
        InputType::Css => match parse_css(source).await {
            Ok(ast) => match to_json_with_tabs(&ast) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    return FixtureResult::Failed(format!("Failed to serialize CSS AST: {e}"));
                }
            },
            Err(_) => fixtures::EXPECTED_SVELTE_ERROR_JSON.to_string(),
        },
        InputType::Svelte => match parse_svelte(source).await {
            Ok(ast) => match to_json_with_tabs(&ast) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    return FixtureResult::Failed(format!("Failed to serialize Svelte AST: {e}"));
                }
            },
            Err(_) => fixtures::EXPECTED_SVELTE_ERROR_JSON.to_string(),
        },
    };

    let expected_ours_path = fixture.expected_ours_path();
    let expected_svelte_path = fixture.expected_svelte_path();

    // Check if files exist and compare
    let existing_ours = fixtures::read_file(&expected_ours_path).ok();
    let existing_svelte = fixtures::read_file(&expected_svelte_path).ok();

    let ours_unchanged = Some(&our_json) == existing_ours.as_ref();
    let svelte_unchanged = Some(&svelte_json) == existing_svelte.as_ref();

    if ours_unchanged && svelte_unchanged {
        return FixtureResult::Unchanged;
    }

    // Write expected_ours.json
    if !ours_unchanged && let Err(e) = fixtures::write_file(&expected_ours_path, &our_json) {
        return FixtureResult::Failed(format!("Failed to write expected_ours.json: {e}"));
    }

    // Write expected_svelte.json
    if !svelte_unchanged && let Err(e) = fixtures::write_file(&expected_svelte_path, &svelte_json) {
        return FixtureResult::Failed(format!("Failed to write expected_svelte.json: {e}"));
    }

    // Determine result based on what existed before
    if existing_ours.is_none() || existing_svelte.is_none() {
        FixtureResult::Created
    } else {
        FixtureResult::Updated
    }
}
