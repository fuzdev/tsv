//! Variant-file discovery: the `discover_*_variants` family plus
//! unknown-file detection for catching typos in fixture directories.

use crate::fixtures::Fixture;
use crate::fixtures::audit_signature::AUDIT_SIGNATURE_FILENAME;
use std::fs;
use std::path::Path;

/// Discover fixture files matching `prefix` (and not any of
/// `exclude_prefixes`) with the given extension, sorted by name.
///
/// Shared body of every `discover_*_variants` function. The exclusions
/// exist because some variant prefixes are prefixes of more specific
/// sibling prefixes (`unformatted_` vs `unformatted_ours_`).
fn discover_prefixed_files(
    fixture_dir: &Path,
    ext: &str,
    prefix: &str,
    exclude_prefixes: &[&str],
) -> Vec<String> {
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(fixture_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with(prefix)
                && !exclude_prefixes.iter().any(|p| filename.starts_with(p))
                && filename.ends_with(ext)
            {
                variants.push(filename.to_string());
            }
        }
    }

    variants.sort();
    variants
}

/// Discover unformatted_* variant files in a fixture directory
/// (excludes unformatted_ours_* files, which are handled separately)
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(
        fixture_dir,
        ext,
        "unformatted_",
        &["unformatted_ours_", "unformatted_prettier_"],
    )
}

/// Discover prettier_variant_* variant files in a fixture directory
///
/// These files document Prettier's stable variants - inputs that Prettier preserves
/// as-is rather than normalizing to a single canonical form.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_prettier_variant_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "prettier_variant_", &[])
}

/// Discover variant_* variant files in a fixture directory
///
/// These files document dual-stable forms that our formatter also keeps stable,
/// but does NOT normalize to `input`. Unlike `prettier_variant_*` (which our formatter
/// normalizes to input), these represent dual-stable forms where both formatters
/// preserve distinct canonical outputs.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_variant_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "variant_", &[])
}

/// Discover unformatted_ours_* variant files in a fixture directory
/// These files test OUR formatter's normalization capability in _prettier_divergence directories
/// where prettier validation is skipped.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_ours_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "unformatted_ours_", &[])
}

/// Discover unformatted_prettier_* variant files in a fixture directory
///
/// These files test that PRETTIER normalizes certain inputs to `output_prettier.*`.
/// Used in `_prettier_divergence` directories where `output_prettier.*` exists.
///
/// Validation rules:
/// - `prettier(unformatted_prettier_*) == output_prettier.*` (prettier normalizes to its canonical output)
/// - Our formatter validation is NOT applied (these test prettier's behavior, not ours)
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_unformatted_prettier_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "unformatted_prettier_", &[])
}

/// Discover prettier_intermediate_* files in a fixture directory
///
/// These files capture Prettier's unstable intermediate output from `unformatted_ours_*` files.
/// They document what Prettier produces on the first pass before reaching a stable form.
///
/// Validation rules:
/// 1. `prettier(unformatted_ours_X) == prettier_intermediate_X` (captures first-pass output)
/// 2. `prettier(prettier_intermediate_X) != prettier_intermediate_X` (verifies it's unstable)
/// 3. `prettier(prettier_intermediate_X) == input` (converges to stable form)
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_prettier_intermediate_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(
        fixture_dir,
        ext,
        "prettier_intermediate_",
        &["prettier_intermediate_to_variant_"],
    )
}

/// Discover prettier_intermediate_to_variant_* files in a fixture directory
///
/// These files capture Prettier's unstable intermediate output from `unformatted_ours_*` files
/// when the second pass converges to a documented `variant_*`/`prettier_variant_*` file
/// rather than to `input`.
///
/// Validation rules (N7b):
/// 1. `prettier(unformatted_ours_X) == prettier_intermediate_to_variant_X` (captures first-pass output)
/// 2. `prettier(prettier_intermediate_to_variant_X) != prettier_intermediate_to_variant_X` (verifies it's unstable)
/// 3. `prettier(prettier_intermediate_to_variant_X) ∈ {variant_*, prettier_variant_*}` content
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_prettier_intermediate_to_variant_variants(
    fixture_dir: &Path,
    ext: &str,
) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "prettier_intermediate_to_variant_", &[])
}

/// Discover input_invalid_* files in a fixture directory
///
/// These files test that parsers correctly reject invalid syntax.
/// They should fail to parse with both our parser and the canonical parser.
///
/// The `ext` parameter should match the input file extension (e.g., ".svelte" or ".ts")
pub fn discover_invalid_variants(fixture_dir: &Path, ext: &str) -> Vec<String> {
    discover_prefixed_files(fixture_dir, ext, "input_invalid_", &[])
}

/// Discover unknown files in a fixture directory
///
/// Returns a list of files that don't match any known fixture file pattern.
/// This helps catch typos like "unformated_*.svelte" (missing 't') or other
/// unexpected files that may have been added by accident.
///
/// Known file patterns:
/// - Input files: input.svelte, input.svelte.ts, input.ts, input.css
/// - Expected JSON: expected.json, expected_ours.json, expected_svelte.json
/// - Output prettier: output_prettier.{ext}
/// - Variants: unformatted_*.{ext}, unformatted_ours_*.{ext}, prettier_variant_*.{ext}, variant_*.{ext}, input_invalid_*.{ext}
/// - Documentation: README.md
pub fn discover_unknown_files(fixture: &Fixture) -> Vec<String> {
    let fixture_dir = &fixture.path;
    let input_ext = fixture.input_type().extension();
    let mut unknown = Vec::new();

    let Ok(entries) = fs::read_dir(fixture_dir) else {
        return unknown;
    };

    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let os_filename = entry.file_name();
        let Some(filename) = os_filename.to_str() else {
            continue;
        };
        if !is_known_fixture_file(filename, input_ext) {
            unknown.push(filename.to_string());
        }
    }

    unknown.sort();
    unknown
}

/// Check if a filename is a known fixture file pattern
fn is_known_fixture_file(filename: &str, input_ext: &str) -> bool {
    // Static files (input, expected, output_prettier, README, audit_signature)
    if matches!(
        filename,
        "input.svelte"
            | "input.svelte.ts"
            | "input.ts"
            | "input.css"
            | "expected.json"
            | "expected_ours.json"
            | "expected_svelte.json"
            | "output_prettier.svelte"
            | "output_prettier.svelte.ts"
            | "output_prettier.ts"
            | "output_prettier.css"
            | "README.md"
            | AUDIT_SIGNATURE_FILENAME
    ) {
        return true;
    }

    // Variant files must have correct extension matching input type
    // unformatted_*.{ext} (but not unformatted_ours_*)
    if filename.starts_with("unformatted_")
        && !filename.starts_with("unformatted_ours_")
        && filename.ends_with(input_ext)
    {
        return true;
    }

    // unformatted_ours_*.{ext}
    if filename.starts_with("unformatted_ours_") && filename.ends_with(input_ext) {
        return true;
    }

    // unformatted_prettier_*.{ext}
    if filename.starts_with("unformatted_prettier_") && filename.ends_with(input_ext) {
        return true;
    }

    // prettier_variant_*.{ext}
    if filename.starts_with("prettier_variant_") && filename.ends_with(input_ext) {
        return true;
    }

    // variant_*.{ext}
    if filename.starts_with("variant_") && filename.ends_with(input_ext) {
        return true;
    }

    // prettier_intermediate_*.{ext}
    if filename.starts_with("prettier_intermediate_") && filename.ends_with(input_ext) {
        return true;
    }

    // input_invalid_*.{ext}
    if filename.starts_with("input_invalid_") && filename.ends_with(input_ext) {
        return true;
    }

    false
}
