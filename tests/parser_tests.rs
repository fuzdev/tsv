mod test_helpers;

use std::fs;
use std::path::{Path, PathBuf};
use test_helpers::discover_fixtures;

/// Parse input using the new multi-crate API
fn parse_to_json(input: &str, file_type: &str) -> Result<String, String> {
    let json = match file_type {
        "svelte" => {
            let ast =
                tsv_svelte::parse(input).map_err(|e| format!("Failed to parse Svelte: {}", e))?;
            let public_ast = tsv_svelte::convert_ast(&ast, input);
            serde_json::to_string(&public_ast)
                .map_err(|e| format!("Failed to serialize Svelte AST: {}", e))?
        }
        "typescript" => {
            let ast =
                tsv_ts::parse(input).map_err(|e| format!("Failed to parse TypeScript: {}", e))?;
            let public_ast = tsv_ts::convert_ast(&ast, input);
            serde_json::to_string(&public_ast)
                .map_err(|e| format!("Failed to serialize TypeScript AST: {}", e))?
        }
        "css" => {
            let ast =
                tsv_css::parse(input, 0).map_err(|e| format!("Failed to parse CSS: {}", e))?;
            let public_ast = tsv_css::convert_ast(&ast, input);
            serde_json::to_string(&public_ast)
                .map_err(|e| format!("Failed to serialize CSS AST: {}", e))?
        }
        _ => return Err(format!("Unknown file type: {}", file_type)),
    };
    Ok(json)
}

fn test_fixture(fixture_path: &Path) -> Result<(), String> {
    let fixture_dir = fixture_path;

    // Parser tests require expected.json
    if !fixture_dir.join("expected.json").exists() {
        panic!(
            "Found input file in {} but missing expected.json",
            fixture_dir.display()
        );
    }

    // Read input file (.ts, .svelte, or .css)
    let (input, file_type) = if fixture_dir.join("input.ts").exists() {
        (
            fs::read_to_string(fixture_dir.join("input.ts"))
                .map_err(|e| format!("Failed to read input.ts: {}", e))?,
            "typescript",
        )
    } else if fixture_dir.join("input.css").exists() {
        (
            fs::read_to_string(fixture_dir.join("input.css"))
                .map_err(|e| format!("Failed to read input.css: {}", e))?,
            "css",
        )
    } else {
        (
            fs::read_to_string(fixture_dir.join("input.svelte"))
                .map_err(|e| format!("Failed to read input.svelte: {}", e))?,
            "svelte",
        )
    };

    let expected = fs::read_to_string(fixture_dir.join("expected.json"))
        .map_err(|e| format!("Failed to read expected.json: {}", e))?;

    let actual = parse_to_json(&input, file_type)?;

    let actual_json: serde_json::Value =
        serde_json::from_str(&actual).map_err(|e| format!("Failed to parse actual JSON: {}", e))?;
    let expected_json: serde_json::Value = serde_json::from_str(&expected)
        .map_err(|e| format!("Failed to parse expected JSON: {}", e))?;

    if actual_json != expected_json {
        // TODO need less verbose diffing
        // Format both for better diff viewing
        // let actual_pretty = serde_json::to_string_pretty(&actual_json).unwrap();
        // let expected_pretty = serde_json::to_string_pretty(&expected_json).unwrap();

        return Err(format!(
            "AST mismatch for {}",
            // "AST mismatch for {}\n\nExpected:\n{}\n\nActual:\n{}\n",
            fixture_path.display(), //expected_pretty, actual_pretty
        ));
    }

    Ok(())
}

#[test]
fn test_all_fixtures() {
    let fixtures_path = PathBuf::from("tests/fixtures");
    let fixtures = discover_fixtures(&fixtures_path);

    println!("\nDiscovered {} fixtures\n", fixtures.len());

    let mut failures = Vec::new();
    let mut passes = 0;

    for fixture in &fixtures {
        match test_fixture(fixture) {
            Ok(()) => {
                println!("✓ {}", fixture.display());
                passes += 1;
            }
            Err(e) => {
                println!("✗ {}: {}", fixture.display(), e);
                failures.push(format!("{}: {}", fixture.display(), e));
            }
        }
    }

    println!(
        "\nResults: {} passed, {} failed out of {} total",
        passes,
        failures.len(),
        fixtures.len()
    );

    if !failures.is_empty() {
        panic!(
            "\n{} fixture(s) failed:\n  - {}",
            failures.len(),
            failures.join("\n  - ")
        );
    }
}
