use std::fs;
use std::path::Path;
use tsv_debug::fixtures::{
    Fixture, discover_unformatted_variants, remove_locations, validate_fixture_structure,
    walk_fixtures,
};

/// Test round-trip: parse → format → parse → compare ASTs (ignoring spans)
/// Uses formatted.* as baseline if it exists (for fixtures with structural changes),
/// otherwise uses input.* (for fixtures where formatter only changes formatting)
fn test_format_round_trip(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

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
            tsv_svelte::parse(&input).map_err(|e| format!("Failed to parse input: {}", e))?;

        let formatted = tsv_svelte::format(&ast1, &input);

        let ast2 = tsv_svelte::parse(&formatted)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;

        // Compare ASTs (ignoring spans)
        let json1 = serde_json::to_value(&tsv_svelte::convert_ast(&ast1, &input))
            .map_err(|e| format!("Failed to serialize AST1: {}", e))?;
        let json2 = serde_json::to_value(&tsv_svelte::convert_ast(&ast2, &formatted))
            .map_err(|e| format!("Failed to serialize AST2: {}", e))?;

        let json1_no_loc = remove_locations(json1);
        let json2_no_loc = remove_locations(json2);

        if json1_no_loc != json2_no_loc {
            return Err(format!(
                "Round-trip failed: AST changed after format for {}",
                fixture.relative_path
            ));
        }
    } else if file_type == "typescript" {
        // TypeScript round-trip
        let ast1 = tsv_ts::parse(&input).map_err(|e| format!("Failed to parse input: {}", e))?;

        let formatted = tsv_ts::format(&ast1);

        let ast2 = tsv_ts::parse(&formatted)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;

        // Compare ASTs (ignoring spans)
        let json1 = serde_json::to_value(&tsv_ts::convert_ast(&ast1, &input))
            .map_err(|e| format!("Failed to serialize AST1: {}", e))?;
        let json2 = serde_json::to_value(&tsv_ts::convert_ast(&ast2, &formatted))
            .map_err(|e| format!("Failed to serialize AST2: {}", e))?;

        let json1_no_loc = remove_locations(json1);
        let json2_no_loc = remove_locations(json2);

        if json1_no_loc != json2_no_loc {
            return Err(format!(
                "Round-trip failed: AST changed after format for {}",
                fixture.relative_path
            ));
        }
    } else {
        // CSS round-trip
        let ast1 =
            tsv_css::parse(&input, 0).map_err(|e| format!("Failed to parse input: {}", e))?;

        let formatted = tsv_css::format(&ast1, &input);

        let ast2 = tsv_css::parse(&formatted, 0)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;

        // For CSS, we'll compare formatted strings directly since CSS AST is simple
        // If we parse and format again, it should be identical
        let formatted2 = tsv_css::format(&ast2, &formatted);

        if formatted != formatted2 {
            return Err(format!(
                "Round-trip failed: formatted output changed after re-parsing for {}",
                fixture.relative_path
            ));
        }
    }

    Ok(())
}

/// Test prettier baseline: format → compare against formatted.* or input.*
fn test_format_matches_prettier(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

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
        let ast = tsv_svelte::parse(&input).map_err(|e| format!("Failed to parse: {}", e))?;
        tsv_svelte::format(&ast, &input)
    } else if file_type == "typescript" {
        let ast = tsv_ts::parse(&input).map_err(|e| format!("Failed to parse: {}", e))?;
        tsv_ts::format(&ast)
    } else {
        let ast = tsv_css::parse(&input, 0).map_err(|e| format!("Failed to parse: {}", e))?;
        tsv_css::format(&ast, &input)
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
            fixture.relative_path, expected, formatted
        ));
    }

    Ok(())
}

/// Test idempotency: format → parse → format → should be identical
/// Uses formatted.* as baseline if it exists (for fixtures with structural changes),
/// otherwise uses input.* (for fixtures where formatter only changes formatting)
fn test_format_idempotent(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

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
            tsv_svelte::parse(&input).map_err(|e| format!("Failed to parse input: {}", e))?;
        let format1 = tsv_svelte::format(&ast1, &input);

        // Format again
        let ast2 = tsv_svelte::parse(&format1)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;
        let format2 = tsv_svelte::format(&ast2, &format1);

        (format1, format2)
    } else if file_type == "typescript" {
        // Format once
        let ast1 = tsv_ts::parse(&input).map_err(|e| format!("Failed to parse input: {}", e))?;
        let format1 = tsv_ts::format(&ast1);

        // Format again
        let ast2 = tsv_ts::parse(&format1)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;
        let format2 = tsv_ts::format(&ast2);

        (format1, format2)
    } else {
        // CSS - Format once
        let ast1 =
            tsv_css::parse(&input, 0).map_err(|e| format!("Failed to parse input: {}", e))?;
        let format1 = tsv_css::format(&ast1, &input);

        // Format again
        let ast2 = tsv_css::parse(&format1, 0)
            .map_err(|e| format!("Failed to parse formatted output: {}", e))?;
        let format2 = tsv_css::format(&ast2, &format1);

        (format1, format2)
    };

    if format1 != format2 {
        return Err(format!(
            "Formatter not idempotent for {}",
            fixture.relative_path
        ));
    }

    Ok(())
}

/// Test normalization: unformatted variants → format → should match expected
fn test_format_normalization(fixture: &Fixture, unformatted_filename: &str) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Read the unformatted variant
    let unformatted_path = fixture_dir.join(unformatted_filename);
    let unformatted_input = fs::read_to_string(&unformatted_path)
        .map_err(|e| format!("Failed to read {}: {}", unformatted_filename, e))?;

    // Detect file type from extension
    let is_svelte = unformatted_filename.ends_with(".svelte");
    let is_css = unformatted_filename.ends_with(".css");

    // Parse and format the unformatted input
    let formatted = if is_svelte {
        let ast = tsv_svelte::parse(&unformatted_input)
            .map_err(|e| format!("Failed to parse {}: {}", unformatted_filename, e))?;
        tsv_svelte::format(&ast, &unformatted_input)
    } else if is_css {
        // For CSS, we need to pass the baseline source (not unformatted source) to the formatter
        // so that blank line preservation references the canonical layout
        let baseline_source = if fixture_dir.join("formatted.css").exists() {
            fs::read_to_string(fixture_dir.join("formatted.css"))
                .map_err(|e| format!("Failed to read formatted.css: {}", e))?
        } else {
            fs::read_to_string(fixture_dir.join("input.css"))
                .map_err(|e| format!("Failed to read input.css: {}", e))?
        };

        let ast = tsv_css::parse(&unformatted_input, 0)
            .map_err(|e| format!("Failed to parse {}: {}", unformatted_filename, e))?;
        tsv_css::format(&ast, &baseline_source)
    } else {
        let ast = tsv_ts::parse(&unformatted_input)
            .map_err(|e| format!("Failed to parse {}: {}", unformatted_filename, e))?;
        tsv_ts::format(&ast)
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
            unformatted_filename, fixture.relative_path, expected, formatted
        ));
    }

    Ok(())
}

/// Result of testing a single fixture
struct FixtureTestResult {
    relative_path: String,
    failures: Vec<String>,
}

impl FixtureTestResult {
    fn new(fixture: &Fixture) -> Self {
        Self {
            relative_path: fixture.relative_path.clone(),
            failures: Vec::new(),
        }
    }

    fn add_failure(&mut self, error: String) {
        self.failures.push(error);
    }

    fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

// Test all fixtures with formatter
#[test]
fn test_formatter_all_fixtures() {
    let fixtures_dir = Path::new("tests/fixtures");
    let fixtures = walk_fixtures(fixtures_dir).expect("Failed to discover fixtures");

    println!("\nDiscovered {} fixtures\n", fixtures.len());

    let mut results: Vec<FixtureTestResult> = Vec::new();

    for fixture in &fixtures {
        let mut result = FixtureTestResult::new(fixture);

        // Fixture structure validation (checks conventions are followed)
        if let Err(e) = validate_fixture_structure(fixture) {
            result.add_failure(format!("[Fixture structure] {}", e));
            results.push(result);
            continue; // Skip further tests if structure is invalid
        }

        // Round-trip test
        if let Err(e) = test_format_round_trip(fixture) {
            result.add_failure(format!("[Round-trip] {}", e));
        }

        // Prettier baseline test
        if let Err(e) = test_format_matches_prettier(fixture) {
            result.add_failure(format!("[Prettier] {}", e));
        }

        // Idempotency test
        if let Err(e) = test_format_idempotent(fixture) {
            result.add_failure(format!("[Idempotent] {}", e));
        }

        // Normalization tests (auto-discover unformatted_*.ts variants)
        let unformatted_variants = discover_unformatted_variants(&fixture.path);
        for variant in unformatted_variants {
            if let Err(e) = test_format_normalization(fixture, &variant) {
                result.add_failure(format!("[Normalization] {}", e));
            }
        }

        results.push(result);
    }

    // Compute statistics
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed()).count();
    let failed = total - passed;

    // Print summary
    if failed > 0 {
        println!("\n{} / {} fixtures failed:\n", failed, total);

        for result in results.iter().filter(|r| !r.passed()) {
            println!("  ✗ {}", result.relative_path());
            for failure in &result.failures {
                println!("      {}", failure);
            }
            println!();
        }

        panic!(
            "test_formatter_all_fixtures - {} / {} failed",
            failed, total
        );
    } else {
        println!("\n✓ All {} fixtures passed", total);
    }
}
