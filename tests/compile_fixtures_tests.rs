//! Pure-Rust compile fixture validation — the sidecar-free slice of the compile
//! fixture contract, so it runs in every `cargo test --workspace`. This is the
//! offline parity gate: `compile()` needs no Deno.
//!
//! Per fixture in `tests/fixtures_compile/`:
//!
//! - `input.svelte` parses with tsv's Svelte parser (the compiler's front end).
//! - `expected_server.js` exists, is non-empty, and is a `canonicalize_js` fixed
//!   point (idempotent; it reparses by construction — the canonicalizer
//!   self-validates its output).
//! - `expected.css`, when present, is non-empty.
//! - **Ours-vs-expected parity**: `tsv_svelte_compile::compile` succeeds, its
//!   canonicalized JS equals the committed `expected_server.js`, and its CSS
//!   matches `expected.css`.
//!
//! Oracle freshness (does the canonical Svelte compiler still produce these
//! expectations?) is sidecar-dependent and lives in the
//! `compile_fixtures_validate` command instead.

use std::path::Path;
use tsv_debug::compile_fixtures::{
    COMPILE_FIXTURES_DIR, walk_compile_fixtures, with_trailing_newline,
};
use tsv_svelte_compile::{CompileOptions, canonicalize_js, compare_canonical, compile};

#[test]
fn test_all_compile_fixtures() {
    let root = Path::new(COMPILE_FIXTURES_DIR);
    assert!(
        root.exists(),
        "Compile fixtures directory not found: {COMPILE_FIXTURES_DIR}"
    );

    let fixtures = walk_compile_fixtures(root).expect("Failed to walk compile fixtures");
    assert!(
        !fixtures.is_empty(),
        "No compile fixtures found in {COMPILE_FIXTURES_DIR}"
    );

    let mut failures: Vec<String> = Vec::new();
    for fixture in &fixtures {
        let name = &fixture.relative_path;

        // input.svelte must parse (the compiler front end accepts it).
        let input = match std::fs::read_to_string(fixture.input_path()) {
            Ok(input) => {
                let arena = bumpalo::Bump::new();
                if let Err(e) = tsv_svelte::parse(&input, &arena) {
                    failures.push(format!("{name}: input.svelte fails to parse: {e}"));
                }
                Some(input)
            }
            Err(e) => {
                failures.push(format!("{name}: cannot read input.svelte: {e}"));
                None
            }
        };

        // expected_server.js must be a canonicalize fixed point.
        let expected_js = match std::fs::read_to_string(fixture.expected_server_js_path()) {
            Ok(expected) => {
                if expected.is_empty() {
                    failures.push(format!("{name}: expected_server.js is empty"));
                } else {
                    match canonicalize_js(&expected) {
                        Ok(again) if again == expected => {}
                        Ok(_) => failures.push(format!(
                            "{name}: expected_server.js is not a canonicalize fixed point — \
                             regenerate via compile_fixture_init"
                        )),
                        Err(e) => failures.push(format!(
                            "{name}: expected_server.js fails to canonicalize: {e}"
                        )),
                    }
                }
                Some(expected)
            }
            Err(e) => {
                failures.push(format!("{name}: cannot read expected_server.js: {e}"));
                None
            }
        };

        // expected.css, when present, must be non-empty.
        let css_path = fixture.expected_css_path();
        let expected_css = if css_path.exists() {
            match std::fs::read_to_string(&css_path) {
                Ok(css) if css.trim().is_empty() => {
                    failures.push(format!("{name}: expected.css is empty"));
                    None
                }
                Ok(css) => Some(css),
                Err(e) => {
                    failures.push(format!("{name}: cannot read expected.css: {e}"));
                    None
                }
            }
        } else {
            None
        };

        // Ours-vs-expected parity — the offline compiler gate.
        if let (Some(input), Some(expected_js)) = (&input, &expected_js) {
            match compile(input, &CompileOptions::default()) {
                Ok(ours) => {
                    // Parity tolerates a comment-POSITION difference (tsv's comment
                    // placement vs the oracle's, which `expected_server.js` records) —
                    // same code, same comment sequence, no bundler annotation.
                    match canonicalize_js(&ours.js) {
                        Ok(canonical) if compare_canonical(&canonical, expected_js).is_parity() => {
                        }
                        Ok(canonical) => failures.push(format!(
                            "{name}: compiled js differs from expected_server.js\n\
                             --- ours ---\n{canonical}--- expected ---\n{expected_js}"
                        )),
                        Err(e) => {
                            failures
                                .push(format!("{name}: compiled js fails to canonicalize: {e}"));
                        }
                    }
                    let ours_css = ours.css.as_deref().map(with_trailing_newline);
                    if ours_css != expected_css {
                        failures.push(format!(
                            "{name}: compiled css differs from expected.css\n\
                             --- ours ---\n{ours_css:?}\n--- expected ---\n{expected_css:?}"
                        ));
                    }
                }
                Err(e) => failures.push(format!("{name}: compile failed: {e}")),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} compile fixture failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
