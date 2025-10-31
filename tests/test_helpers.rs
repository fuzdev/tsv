use std::fs;
use std::path::{Path, PathBuf};

/// Recursively discovers all fixtures in a directory tree
pub fn discover_fixtures(base_dir: &Path) -> Vec<PathBuf> {
    let mut fixtures = Vec::new();
    discover_fixtures_recursive(base_dir, base_dir, &mut fixtures);
    fixtures.sort();
    fixtures
}

/// Discovers unformatted variant files (unformatted_*.{ts,svelte,css}) in a fixture directory
#[allow(dead_code)]
pub fn discover_unformatted_variants(fixture_dir: &Path) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str() {
                if filename.starts_with("unformatted_")
                    && (filename.ends_with(".ts")
                        || filename.ends_with(".svelte")
                        || filename.ends_with(".css"))
                {
                    variants.push(filename.to_string());
                }
            }
        }
    }

    variants.sort();
    variants
}

fn discover_fixtures_recursive(base_dir: &Path, current_dir: &Path, fixtures: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                discover_fixtures_recursive(base_dir, &path, fixtures);
            }
        }
    }

    // Check if current_dir is a fixture (has input file)
    let has_input = current_dir.join("input.ts").exists()
        || current_dir.join("input.svelte").exists()
        || current_dir.join("input.css").exists();

    if has_input {
        // Get relative path from base_dir
        let rel_path = current_dir.strip_prefix(base_dir).unwrap_or(current_dir);
        fixtures.push(rel_path.to_path_buf());
    }
}

/// Recursively remove location/span fields from JSON for comparison
#[allow(dead_code)]
pub fn remove_locations(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Object(map) => {
            map.remove("start");
            map.remove("end");
            map.remove("loc");
            for v in map.values_mut() {
                *v = remove_locations(v.clone());
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                *v = remove_locations(v.clone());
            }
        }
        _ => {}
    }
    value
}

/// Validate fixture structure and conventions
///
/// Checks:
/// 1. `input.*` exists for every fixture
/// 2. `unformatted_*` variants are NOT identical to `input.*`
/// 3. `formatted.*` files exist only when different from `input.*`
/// 4. `expected.json` exists (required for parser tests)
#[allow(dead_code)]
pub fn validate_fixture_structure(fixture_dir: &Path) -> Result<(), String> {
    // Find input file
    let input_file = if fixture_dir.join("input.ts").exists() {
        fixture_dir.join("input.ts")
    } else if fixture_dir.join("input.svelte").exists() {
        fixture_dir.join("input.svelte")
    } else if fixture_dir.join("input.css").exists() {
        fixture_dir.join("input.css")
    } else {
        return Err("No input.* file found".to_string());
    };

    // Check expected.json exists (required for parser tests)
    let expected_path = fixture_dir.join("expected.json");
    if !expected_path.exists() {
        return Err(format!(
            "Missing expected.json (required for parser tests, run: npm run fixtures:update-expected)"
        ));
    }

    let input_content =
        fs::read_to_string(&input_file).map_err(|e| format!("Failed to read input file: {}", e))?;

    // Get file extension for finding other variants
    let ext = input_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown");

    // Check unformatted_* variants are not identical to input
    let unformatted_variants = discover_unformatted_variants(fixture_dir);
    for variant_name in unformatted_variants {
        let variant_path = fixture_dir.join(&variant_name);
        let variant_content = fs::read_to_string(&variant_path)
            .map_err(|e| format!("Failed to read {}: {}", variant_name, e))?;

        if variant_content == input_content {
            return Err(format!(
                "unformatted_* variant '{}' is identical to input.* (should be different for testing normalization)",
                variant_name
            ));
        }
    }

    // Check formatted.* files only exist when different from input
    let formatted_path = fixture_dir.join(format!("formatted.{}", ext));
    if formatted_path.exists() {
        let formatted_content = fs::read_to_string(&formatted_path)
            .map_err(|e| format!("Failed to read formatted.{}: {}", ext, e))?;

        if formatted_content == input_content {
            return Err(format!(
                "formatted.{} is identical to input.{} (should be deleted if no changes needed)",
                ext, ext
            ));
        }
    }

    Ok(())
}
