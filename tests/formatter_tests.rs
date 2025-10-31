mod test_helpers;

use std::fs;
use std::path::{Path, PathBuf};
use test_helpers::{
    discover_fixtures, discover_unformatted_variants, remove_locations, validate_fixture_structure,
};

/// Test round-trip: parse → format → parse → compare ASTs (ignoring spans)
/// Uses formatted.* as baseline if it exists (for fixtures with structural changes),
/// otherwise uses input.* (for fixtures where formatter only changes formatting)
fn test_format_round_trip(fixture_path: &Path) -> Result<(), String> {
    let fixture_dir = fixture_path;

    // Check for Svelte, TypeScript, or CSS file
    let (input_file, formatted_file, file_type) = if fixture_dir.join("input.svelte").exists() {
        ("input.svelte", "formatted.svelte", "svelte")
    } else if fixture_dir.join("input.ts").exists() {
        ("input.ts", "formatted.ts", "typescript")
    } else if fixture_dir.join("input.css").exists() {
        ("input.css", "formatted.css", "css")
    } else {
        return Ok(()); // No supported input file
    };

    // Use formatted.* as baseline if it exists (for structural changes like reordering),
    // otherwise use input.* (for pure formatting changes)
    let baseline_file = if fixture_dir.join(formatted_file).exists() {
        formatted_file
    } else {
        input_file
    };

    let input = fs::read_to_string(fixture_dir.join(baseline_file))
        .map_err(|e| format!("Failed to read {}: {}", baseline_file, e))?;

    if file_type == "svelte" {
        // Svelte round-trip
        let ast1 =
            tsv::parse_svelte_ast(&input).map_err(|e| format!("Failed to parse input: {}", e))?;

        let formatted = tsv::format_svelte(&ast1, &input);

        let ast2 = tsv::parse_svelte_ast(&formatted)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;

        // Compare ASTs (ignoring spans)
        let json1 = serde_json::to_value(&tsv::convert_root(&ast1, &input))
            .map_err(|e| format!("Failed to serialize AST1: {}", e))?;
        let json2 = serde_json::to_value(&tsv::convert_root(&ast2, &formatted))
            .map_err(|e| format!("Failed to serialize AST2: {}", e))?;

        let json1_no_loc = remove_locations(json1);
        let json2_no_loc = remove_locations(json2);

        if json1_no_loc != json2_no_loc {
            return Err(format!(
                "Round-trip failed: AST changed after format for {}",
                fixture_path.display()
            ));
        }
    } else if file_type == "typescript" {
        // TypeScript round-trip
        let ast1 = tsv::parse_typescript_ast(&input)
            .map_err(|e| format!("Failed to parse input: {}", e))?;

        let formatted = tsv::format_typescript(&ast1);

        let ast2 = tsv::parse_typescript_ast(&formatted)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;

        // Compare ASTs (ignoring spans)
        let json1 = serde_json::to_value(&tsv::convert_program(
            &ast1,
            &tsv::LocationTracker::new(&input),
        ))
        .map_err(|e| format!("Failed to serialize AST1: {}", e))?;
        let json2 = serde_json::to_value(&tsv::convert_program(
            &ast2,
            &tsv::LocationTracker::new(&formatted),
        ))
        .map_err(|e| format!("Failed to serialize AST2: {}", e))?;

        let json1_no_loc = remove_locations(json1);
        let json2_no_loc = remove_locations(json2);

        if json1_no_loc != json2_no_loc {
            return Err(format!(
                "Round-trip failed: AST changed after format for {}",
                fixture_path.display()
            ));
        }
    } else {
        // CSS round-trip
        let ast1 =
            tsv::parse_css_ast(&input).map_err(|e| format!("Failed to parse input: {}", e))?;

        let formatted = tsv::format_css(&ast1);

        let ast2 = tsv::parse_css_ast(&formatted)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;

        // For CSS, we'll compare formatted strings directly since CSS AST is simple
        // If we parse and format again, it should be identical
        let formatted2 = tsv::format_css(&ast2);

        if formatted != formatted2 {
            return Err(format!(
                "Round-trip failed: formatted output changed after re-parsing for {}",
                fixture_path.display()
            ));
        }
    }

    Ok(())
}

/// Test prettier baseline: format → compare against formatted.* or input.*
fn test_format_matches_prettier(fixture_path: &Path) -> Result<(), String> {
    let fixture_dir = fixture_path;

    // Check for Svelte, TypeScript, or CSS file
    let (input_file, formatted_file, file_type) = if fixture_dir.join("input.svelte").exists() {
        ("input.svelte", "formatted.svelte", "svelte")
    } else if fixture_dir.join("input.ts").exists() {
        ("input.ts", "formatted.ts", "typescript")
    } else if fixture_dir.join("input.css").exists() {
        ("input.css", "formatted.css", "css")
    } else {
        return Ok(()); // No supported input file
    };

    let input = fs::read_to_string(fixture_dir.join(input_file))
        .map_err(|e| format!("Failed to read {}: {}", input_file, e))?;

    // Parse and format based on file type
    let formatted = if file_type == "svelte" {
        let ast = tsv::parse_svelte_ast(&input).map_err(|e| format!("Failed to parse: {}", e))?;
        tsv::format_svelte(&ast, &input)
    } else if file_type == "typescript" {
        let ast =
            tsv::parse_typescript_ast(&input).map_err(|e| format!("Failed to parse: {}", e))?;
        tsv::format_typescript(&ast)
    } else {
        let ast = tsv::parse_css_ast(&input).map_err(|e| format!("Failed to parse: {}", e))?;
        tsv::format_css(&ast)
    };

    // Check against formatted.* if it exists, otherwise input.*
    let expected = if fixture_dir.join(formatted_file).exists() {
        fs::read_to_string(fixture_dir.join(formatted_file))
            .map_err(|e| format!("Failed to read {}: {}", formatted_file, e))?
    } else {
        input
    };

    if formatted != expected {
        return Err(format!(
            "Formatter output doesn't match prettier baseline for {}\n\nExpected:\n{}\n\nActual:\n{}",
            fixture_path.display(),
            expected,
            formatted
        ));
    }

    Ok(())
}

/// Test idempotency: format → parse → format → should be identical
/// Uses formatted.* as baseline if it exists (for fixtures with structural changes),
/// otherwise uses input.* (for fixtures where formatter only changes formatting)
fn test_format_idempotent(fixture_path: &Path) -> Result<(), String> {
    let fixture_dir = fixture_path;

    // Check for Svelte, TypeScript, or CSS file
    let (input_file, formatted_file, file_type) = if fixture_dir.join("input.svelte").exists() {
        ("input.svelte", "formatted.svelte", "svelte")
    } else if fixture_dir.join("input.ts").exists() {
        ("input.ts", "formatted.ts", "typescript")
    } else if fixture_dir.join("input.css").exists() {
        ("input.css", "formatted.css", "css")
    } else {
        return Ok(()); // No supported input file
    };

    // Use formatted.* as baseline if it exists (for structural changes like reordering),
    // otherwise use input.* (for pure formatting changes)
    let baseline_file = if fixture_dir.join(formatted_file).exists() {
        formatted_file
    } else {
        input_file
    };

    let input = fs::read_to_string(fixture_dir.join(baseline_file))
        .map_err(|e| format!("Failed to read {}: {}", baseline_file, e))?;

    let (format1, format2) = if file_type == "svelte" {
        // Format once
        let ast1 =
            tsv::parse_svelte_ast(&input).map_err(|e| format!("Failed to parse input: {}", e))?;
        let format1 = tsv::format_svelte(&ast1, &input);

        // Format again
        let ast2 = tsv::parse_svelte_ast(&format1)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;
        let format2 = tsv::format_svelte(&ast2, &format1);

        (format1, format2)
    } else if file_type == "typescript" {
        // Format once
        let ast1 = tsv::parse_typescript_ast(&input)
            .map_err(|e| format!("Failed to parse input: {}", e))?;
        let format1 = tsv::format_typescript(&ast1);

        // Format again
        let ast2 = tsv::parse_typescript_ast(&format1)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;
        let format2 = tsv::format_typescript(&ast2);

        (format1, format2)
    } else {
        // CSS - Format once
        let ast1 =
            tsv::parse_css_ast(&input).map_err(|e| format!("Failed to parse input: {}", e))?;
        let format1 = tsv::format_css(&ast1);

        // Format again
        let ast2 = tsv::parse_css_ast(&format1)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;
        let format2 = tsv::format_css(&ast2);

        (format1, format2)
    };

    if format1 != format2 {
        return Err(format!(
            "Formatter not idempotent for {}",
            fixture_path.display()
        ));
    }

    Ok(())
}

/// Test normalization: unformatted variants → format → should match expected
fn test_format_normalization(
    fixture_path: &Path,
    unformatted_filename: &str,
) -> Result<(), String> {
    let fixture_dir = fixture_path;

    // Read the unformatted variant
    let unformatted_path = fixture_dir.join(unformatted_filename);
    let unformatted_input = fs::read_to_string(&unformatted_path)
        .map_err(|e| format!("Failed to read {}: {}", unformatted_filename, e))?;

    // Detect file type from extension
    let is_svelte = unformatted_filename.ends_with(".svelte");
    let is_css = unformatted_filename.ends_with(".css");

    // Parse and format the unformatted input
    let formatted = if is_svelte {
        let ast = tsv::parse_svelte_ast(&unformatted_input)
            .map_err(|e| format!("Failed to parse {}: {}", unformatted_filename, e))?;
        tsv::format_svelte(&ast, &unformatted_input)
    } else if is_css {
        let ast = tsv::parse_css_ast(&unformatted_input)
            .map_err(|e| format!("Failed to parse {}: {}", unformatted_filename, e))?;
        tsv::format_css(&ast)
    } else {
        let ast = tsv::parse_typescript_ast(&unformatted_input)
            .map_err(|e| format!("Failed to parse {}: {}", unformatted_filename, e))?;
        tsv::format_typescript(&ast)
    };

    // Determine expected output based on file type
    let (formatted_file, input_file) = if is_svelte {
        ("formatted.svelte", "input.svelte")
    } else if is_css {
        ("formatted.css", "input.css")
    } else {
        ("formatted.ts", "input.ts")
    };

    let expected = if fixture_dir.join(formatted_file).exists() {
        fs::read_to_string(fixture_dir.join(formatted_file))
            .map_err(|e| format!("Failed to read {}: {}", formatted_file, e))?
    } else {
        fs::read_to_string(fixture_dir.join(input_file))
            .map_err(|e| format!("Failed to read {}: {}", input_file, e))?
    };

    // Compare formatted output to expected
    if formatted != expected {
        return Err(format!(
            "Failed to normalize {} in {}\n\nExpected:\n{}\n\nActual:\n{}",
            unformatted_filename,
            fixture_path.display(),
            expected,
            formatted
        ));
    }

    Ok(())
}

// Test all fixtures with formatter
#[test]
fn test_formatter_all_fixtures() {
    let fixtures_dir = PathBuf::from("tests/fixtures");
    let fixtures = discover_fixtures(&fixtures_dir);

    println!("\nDiscovered {} fixtures\n", fixtures.len());

    let mut failures = Vec::new();

    for fixture in &fixtures {
        // Fixture structure validation (checks conventions are followed)
        if let Err(e) = validate_fixture_structure(fixture) {
            failures.push(format!(
                "[Fixture structure] Failed to validate {}: {}",
                fixture.display(),
                e
            ));
            continue; // Skip further tests if structure is invalid
        }

        // Round-trip test
        if let Err(e) = test_format_round_trip(fixture) {
            failures.push(format!("[Round-trip] {}", e));
        }

        // Prettier baseline test
        if let Err(e) = test_format_matches_prettier(fixture) {
            failures.push(format!("[Prettier] {}", e));
        }

        // Idempotency test
        if let Err(e) = test_format_idempotent(fixture) {
            failures.push(format!("[Idempotent] {}", e));
        }

        // Normalization tests (auto-discover unformatted_*.ts variants)
        let unformatted_variants = discover_unformatted_variants(fixture);
        for variant in unformatted_variants {
            if let Err(e) = test_format_normalization(fixture, &variant) {
                failures.push(format!("[Normalization] {}", e));
            }
        }
    }

    if !failures.is_empty() {
        panic!("Formatter tests failed:\n{}", failures.join("\n"));
    }
}
