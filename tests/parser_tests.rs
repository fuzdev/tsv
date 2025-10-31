mod test_helpers;

use std::fs;
use std::path::{Path, PathBuf};
use test_helpers::discover_fixtures;

fn test_fixture(fixture_path: &Path) -> Result<(), String> {
    let fixture_dir = PathBuf::from("tests/fixtures").join(fixture_path);

    // Parser tests require expected.json
    if !fixture_dir.join("expected.json").exists() {
        panic!(
            "Found input file in {} but missing expected.json",
            fixture_dir.display()
        );
    }

    // Read input file (.ts, .svelte, or .css)
    let input = if fixture_dir.join("input.ts").exists() {
        fs::read_to_string(fixture_dir.join("input.ts"))
            .map_err(|e| format!("Failed to read input.ts: {}", e))?
    } else if fixture_dir.join("input.css").exists() {
        fs::read_to_string(fixture_dir.join("input.css"))
            .map_err(|e| format!("Failed to read input.css: {}", e))?
    } else {
        fs::read_to_string(fixture_dir.join("input.svelte"))
            .map_err(|e| format!("Failed to read input.svelte: {}", e))?
    };

    let expected = fs::read_to_string(fixture_dir.join("expected.json"))
        .map_err(|e| format!("Failed to read expected.json: {}", e))?;

    let actual = tsvr::parse_to_json(&input).map_err(|e| format!("Failed to parse: {}", e))?;

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
