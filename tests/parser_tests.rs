use std::fs;
use std::path::Path;
use tsv_debug::fixtures::{Fixture, walk_fixtures};

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

fn test_fixture(fixture: &Fixture) -> Result<(), String> {
    // Parser tests require expected.json
    if !fixture.expected_path().exists() {
        panic!(
            "Found input file in {} but missing expected.json",
            fixture.relative_path
        );
    }

    // Read input file
    let input = fs::read_to_string(fixture.input_path())
        .map_err(|e| format!("Failed to read {}: {}", fixture.input_file, e))?;

    // Determine file type
    let file_type = if fixture.input_file.ends_with(".ts") {
        "typescript"
    } else if fixture.input_file.ends_with(".css") {
        "css"
    } else {
        "svelte"
    };

    let expected = fs::read_to_string(fixture.expected_path())
        .map_err(|e| format!("Failed to read expected.json: {}", e))?;

    let actual = parse_to_json(&input, file_type)?;

    let actual_json: serde_json::Value =
        serde_json::from_str(&actual).map_err(|e| format!("Failed to parse actual JSON: {}", e))?;
    let expected_json: serde_json::Value = serde_json::from_str(&expected)
        .map_err(|e| format!("Failed to parse expected JSON: {}", e))?;

    if actual_json != expected_json {
        // Format both for better diff viewing
        let actual_pretty = serde_json::to_string_pretty(&actual_json).unwrap();
        let expected_pretty = serde_json::to_string_pretty(&expected_json).unwrap();

        return Err(format!(
            "AST mismatch for {}\n\nExpected:\n{}\n\nActual:\n{}\n",
            fixture.relative_path, expected_pretty, actual_pretty
        ));
    }

    Ok(())
}

#[test]
fn test_all_fixtures() {
    let fixtures_path = Path::new("tests/fixtures");
    let fixtures = walk_fixtures(fixtures_path).expect("Failed to discover fixtures");

    println!("\nDiscovered {} fixtures\n", fixtures.len());

    let mut failures = Vec::new();
    let mut passes = 0;

    for fixture in &fixtures {
        match test_fixture(fixture) {
            Ok(()) => {
                println!("✓ {}", fixture.relative_path);
                passes += 1;
            }
            Err(e) => {
                println!("✗ {}: {}", fixture.relative_path, e);
                failures.push(format!("{}: {}", fixture.relative_path, e));
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
