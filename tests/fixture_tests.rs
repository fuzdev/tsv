use std::fs;
use std::path::{Path, PathBuf};

/// Discovers all fixtures in a given directory
fn discover_fixtures(base_dir: &Path) -> Vec<(String, String)> {
    let mut fixtures = Vec::new();

    if let Ok(categories) = fs::read_dir(base_dir) {
        for category_entry in categories.flatten() {
            let category_path = category_entry.path();
            if !category_path.is_dir() {
                continue;
            }

            let category_name = category_entry.file_name().to_string_lossy().to_string();

            if let Ok(fixture_dirs) = fs::read_dir(&category_path) {
                for fixture_entry in fixture_dirs.flatten() {
                    let fixture_path = fixture_entry.path();
                    if !fixture_path.is_dir() {
                        continue;
                    }

                    let fixture_name = fixture_entry.file_name().to_string_lossy().to_string();

                    // Check if this looks like a valid fixture (has input and expected files)
                    let has_input = fixture_path.join("input.ts").exists()
                        || fixture_path.join("input.svelte").exists()
                        || fixture_path.join("input.css").exists();
                    let has_expected = fixture_path.join("expected.json").exists();

                    if has_input && has_expected {
                        fixtures.push((category_name.clone(), fixture_name));
                    }
                }
            }
        }
    }

    fixtures.sort();
    fixtures
}

fn test_fixture(category: &str, name: &str) -> Result<(), String> {
    let fixture_dir = PathBuf::from("tests/fixtures").join(category).join(name);

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

    let actual = tsvr::parse_to_json(&input)
        .map_err(|e| format!("Failed to parse: {}", e))?;

    let actual_json: serde_json::Value = serde_json::from_str(&actual)
        .map_err(|e| format!("Failed to parse actual JSON: {}", e))?;
    let expected_json: serde_json::Value = serde_json::from_str(&expected)
        .map_err(|e| format!("Failed to parse expected JSON: {}", e))?;

    if actual_json != expected_json {
        return Err(format!("AST mismatch for {}/{}", category, name));
    }

    Ok(())
}

#[test]
fn test_all_fixtures() {
    let fixtures_path = PathBuf::from("tests/fixtures");
    let fixtures = discover_fixtures(&fixtures_path);

    println!("\nDiscovered {} fixtures:", fixtures.len());
    for (category, name) in &fixtures {
        println!("  {}/{}", category, name);
    }
    println!();

    let mut failures = Vec::new();
    let mut passes = 0;

    for (category, name) in &fixtures {
        match test_fixture(category, name) {
            Ok(()) => {
                println!("✓ {}/{}", category, name);
                passes += 1;
            }
            Err(e) => {
                println!("✗ {}/{}: {}", category, name, e);
                failures.push(format!("{}/{}: {}", category, name, e));
            }
        }
    }

    println!("\nResults: {} passed, {} failed out of {} total",
             passes, failures.len(), fixtures.len());

    if !failures.is_empty() {
        panic!("\n{} fixture(s) failed:\n  - {}",
               failures.len(),
               failures.join("\n  - "));
    }
}
