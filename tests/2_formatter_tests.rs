use std::fs;
use std::path::Path;
use tsv_debug::fixtures::{
    discover_prettier_quirk_variants, discover_unformatted_ours_variants,
    discover_unformatted_variants, remove_locations, validate_fixture_structure, walk_fixtures,
    Fixture,
};

/// Test round-trip: parse → format → parse → compare ASTs (ignoring spans)
/// Uses input.svelte as baseline (always canonical)
fn test_format_round_trip(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // All fixtures use input.svelte (Svelte components with embedded CSS/TypeScript)
    let input_file = "input.svelte";
    if !fixture_dir.join(input_file).exists() {
        return Ok(()); // No input file (shouldn't happen in valid fixtures)
    }

    let input = fs::read_to_string(fixture_dir.join(input_file))
        .map_err(|e| format!("Failed to read {}: {}", input_file, e))?;

    // Svelte round-trip (all fixtures are .svelte files with embedded CSS/TypeScript)
    let ast1 = tsv_svelte::parse(&input).map_err(|e| format!("Failed to parse input: {}", e))?;

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

    Ok(())
}

/// Test prettier baseline: format → compare against input.svelte (always canonical)
fn test_format_matches_prettier(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Skip if our formatter intentionally differs from prettier
    // (implicit detection via output_prettier.svelte existence)
    if fixture.output_prettier_path().exists() {
        return Ok(());
    }

    // All fixtures use input.svelte
    let input_file = "input.svelte";
    if !fixture_dir.join(input_file).exists() {
        return Ok(()); // No input file
    }

    let input = fs::read_to_string(fixture_dir.join(input_file))
        .map_err(|e| format!("Failed to read {}: {}", input_file, e))?;

    // Parse and format (all fixtures are Svelte files)
    let ast = tsv_svelte::parse(&input).map_err(|e| format!("Failed to parse: {}", e))?;
    let formatted = tsv_svelte::format(&ast, &input);

    // Check that formatting is idempotent (input.svelte formats to itself)
    if formatted != input {
        return Err(format!(
            "Formatter output doesn't match prettier baseline for {}\n\nExpected:\n{}\n\nActual:\n{}",
            fixture.relative_path, input, formatted
        ));
    }

    Ok(())
}

/// Test idempotency: format → parse → format → should be identical
/// Uses input.svelte as baseline (always canonical)
fn test_format_idempotent(fixture: &Fixture) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // All fixtures use input.svelte
    let input_file = "input.svelte";
    if !fixture_dir.join(input_file).exists() {
        return Ok(()); // No input file
    }

    let input = fs::read_to_string(fixture_dir.join(input_file))
        .map_err(|e| format!("Failed to read {}: {}", input_file, e))?;

    // Format once
    let ast1 = tsv_svelte::parse(&input).map_err(|e| format!("Failed to parse input: {}", e))?;
    let format1 = tsv_svelte::format(&ast1, &input);

    // Format again
    let ast2 = tsv_svelte::parse(&format1)
        .map_err(|e| format!("Failed to parse formatted output: {}", e))?;
    let format2 = tsv_svelte::format(&ast2, &format1);

    if format1 != format2 {
        return Err(format!(
            "Formatter not idempotent for {}",
            fixture.relative_path
        ));
    }

    Ok(())
}

/// Test normalization: unformatted variants → format → should match input.svelte (always canonical)
fn test_format_normalization(fixture: &Fixture, unformatted_filename: &str) -> Result<(), String> {
    let fixture_dir = &fixture.path;

    // Read the unformatted variant
    let unformatted_path = fixture_dir.join(unformatted_filename);
    let unformatted_input = fs::read_to_string(&unformatted_path)
        .map_err(|e| format!("Failed to read {}: {}", unformatted_filename, e))?;

    // Parse and format the unformatted input (all variants are .svelte files)
    let ast = tsv_svelte::parse(&unformatted_input)
        .map_err(|e| format!("Failed to parse {}: {}", unformatted_filename, e))?;
    let formatted = tsv_svelte::format(&ast, &unformatted_input);

    // Expected output is always input.svelte (canonical baseline)
    let expected = fs::read_to_string(fixture_dir.join("input.svelte"))
        .map_err(|e| format!("Failed to read input.svelte: {}", e))?;

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

// Test all fixtures with printer
#[test]
fn test_formatter_output_quality() {
    let fixtures_dir = Path::new("tests/fixtures");
    let fixtures = walk_fixtures(fixtures_dir).expect("Failed to discover fixtures");

    println!("\nFormatter: Testing {} fixtures\n", fixtures.len());

    let mut results: Vec<FixtureTestResult> = Vec::new();

    for fixture in &fixtures {
        let mut result = FixtureTestResult::new(fixture);

        // Fixture structure validation (checks conventions are followed)
        if let Err(e) = validate_fixture_structure(fixture) {
            result.add_failure(format!("[Fixture structure] {}", e));
            // TODO why is this needed instead of pushing to results like elsewhere?
            println!("✗ {}", result.relative_path());
            println!("    [Fixture structure] {}", e);
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

        // Normalization tests (auto-discover unformatted_*.svelte variants)
        let unformatted_variants = discover_unformatted_variants(&fixture.path);
        for variant in unformatted_variants {
            if let Err(e) = test_format_normalization(fixture, &variant) {
                result.add_failure(format!("[Normalization] {}", e));
            }
        }

        // Normalization tests for unformatted_ours_*.svelte (only in _prettier_divergence directories)
        // These files test OUR formatter's normalization when prettier validation is skipped
        let unformatted_ours_variants = discover_unformatted_ours_variants(&fixture.path);
        for variant in unformatted_ours_variants {
            if let Err(e) = test_format_normalization(fixture, &variant) {
                result.add_failure(format!("[Normalization (Ours)] {}", e));
            }
        }

        // Prettier quirk tests (auto-discover prettier_quirk_*.svelte variants)
        // These files are preserved by prettier but we normalize them (better normalization)
        let prettier_quirk_variants = discover_prettier_quirk_variants(&fixture.path);
        for variant in &prettier_quirk_variants {
            if let Err(e) = test_format_normalization(fixture, variant) {
                result.add_failure(format!("[Prettier Quirk Normalization] {}", e));
            }
        }

        if result.passed() {
            println!("✓ {}", result.relative_path());
        } else {
            println!("✗ {}", result.relative_path());
            for failure in &result.failures {
                println!("    {}", failure);
            }
        }

        results.push(result);
    }

    // Compute statistics
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed()).count();
    let failed = total - passed;

    println!("\n════════════════════\n");

    // Print summary
    if failed > 0 {
        println!("{} / {} fixtures failed:\n", failed, total);

        for result in results.iter().filter(|r| !r.passed()) {
            println!("  ✗ {}", result.relative_path());
        }

        println!(
            "\nResults Summary: {} passed, {} failed out of {} total\n",
            passed, failed, total
        );

        panic!("\nFormatter test failed");
    } else {
        println!(
            "Results Summary: {} passed, {} failed out of {} total",
            passed, failed, total
        );
    }
}
