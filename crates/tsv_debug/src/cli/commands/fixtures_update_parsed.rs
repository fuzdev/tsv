use crate::fixtures;
use std::path::Path;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::json_utils::{ensure_trailing_newline, to_json_with_tabs};

/// fixtures-update-parsed command - regenerate expected.json (or expected_ours.json + expected_svelte.json) files
pub struct FixturesUpdateParsedCommand;

impl Command for FixturesUpdateParsedCommand {
    fn name(&self) -> &str {
        "fixtures_update_parsed"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let list_only = args.flag("list");

        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesUpdateParsedExecutable {
            list_only,
            filters,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_update_parsed                    Regenerate all expected.json files"
                .to_string(),
            "fixtures_update_parsed --list             List all fixtures".to_string(),
            "fixtures_update_parsed <filter>...        Regenerate matching fixtures".to_string(),
        ]
    }
}

struct FixturesUpdateParsedExecutable {
    list_only: bool,
    filters: Vec<String>,
}

impl Executable for FixturesUpdateParsedExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        rt.block_on(run(self.list_only, &self.filters));
    }
}

async fn run(list_only: bool, filters: &[String]) {
    let fixtures_dir = Path::new("tests/fixtures");

    if !fixtures_dir.exists() {
        eprintln!("Error: fixtures directory not found: tests/fixtures");
        std::process::exit(1);
    }

    let all_fixtures = match fixtures::walk_fixtures(fixtures_dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error walking fixtures: {e}");
            std::process::exit(1);
        }
    };

    let total_count = all_fixtures.len();

    // Apply filters
    let fixture_list: Vec<_> = all_fixtures
        .into_iter()
        .filter(|f| f.matches_filters(filters))
        .collect();

    if fixture_list.is_empty() {
        if filters.is_empty() {
            eprintln!("No fixtures found");
        } else {
            eprintln!("No fixtures found matching: {}", filters.join(" "));
        }
        std::process::exit(1);
    }

    if list_only {
        println!("Found fixtures:");
        for fixture in &fixture_list {
            println!("  {} ({})", fixture.relative_path, fixture.input_file);
        }
        if filters.is_empty() {
            println!("\nTotal: {}", fixture_list.len());
        } else {
            println!(
                "\nMatched: {} of {} fixtures",
                fixture_list.len(),
                total_count
            );
        }
        return;
    }

    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let mut failed = 0;

    for fixture in &fixture_list {
        match generate_expected_fixture(fixture).await {
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
            "\nSummary: {} created, {} updated, {} unchanged, {} failed ({} fixtures)",
            created,
            updated,
            unchanged,
            failed,
            fixture_list.len()
        );
    } else {
        println!(
            "\nSummary: {} created, {} updated, {} unchanged, {} failed (matched {} of {} fixtures)",
            created,
            updated,
            unchanged,
            failed,
            fixture_list.len(),
            total_count
        );
    }

    if created > 0 || updated > 0 {
        println!("⚠️  Updated source of truth files (expected.json)");
    }

    if failed > 0 {
        std::process::exit(1);
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

    // Standard pattern: generate expected.json from Svelte's parser
    let json = match fuz_client::parse_svelte(&source).await {
        Ok(json) => ensure_trailing_newline(json),
        Err(e) => return FixtureResult::Failed(format!("Svelte parse error: {e}")),
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
    // Generate expected_ours.json from our parser
    // Parse directly and serialize the struct (not via serde_json::Value) to preserve field order
    let ast = match tsv_svelte::parse(source) {
        Ok(ast) => ast,
        Err(e) => return FixtureResult::Failed(format!("Our parser error: {e:?}")),
    };
    let public_ast = tsv_svelte::convert_ast(&ast, source);

    // Serialize with tab indentation (matching CLI and existing expected.json files)
    let our_json = match to_json_with_tabs(&public_ast) {
        Ok(json) => format!("{json}\n"),
        Err(e) => return FixtureResult::Failed(format!("Failed to serialize our AST: {e}")),
    };

    // Generate expected_svelte.json from Svelte's parser (or error marker)
    let svelte_json = match fuz_client::parse_svelte(source).await {
        Ok(json) => ensure_trailing_newline(json),
        Err(_) => {
            // Svelte parse failed - use canonical error marker
            fixtures::EXPECTED_SVELTE_ERROR_JSON.to_string()
        }
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
