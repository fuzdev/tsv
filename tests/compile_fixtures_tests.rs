//! Pure-Rust compile fixture validation — the sidecar-free slice of the compile
//! fixture contract, so it runs in every `cargo test --workspace`.
//!
//! Per fixture in `tests/fixtures_compile/`:
//!
//! - `input.svelte` parses with tsv's Svelte parser (the compiler's front end).
//! - `expected_server.js` exists, is non-empty, and is a `canonicalize_js` fixed
//!   point (idempotent; it reparses by construction — the canonicalizer
//!   self-validates its output).
//! - `expected.css`, when present, is non-empty.
//!
//! Oracle freshness (does the canonical Svelte compiler still produce these
//! expectations?) is sidecar-dependent and lives in the
//! `compile_fixtures_validate` command instead.

use std::path::Path;
use tsv_debug::compile_fixtures::{COMPILE_FIXTURES_DIR, walk_compile_fixtures};
use tsv_svelte_compile::canonicalize_js;

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
        match std::fs::read_to_string(fixture.input_path()) {
            Ok(input) => {
                let arena = bumpalo::Bump::new();
                if let Err(e) = tsv_svelte::parse(&input, &arena) {
                    failures.push(format!("{name}: input.svelte fails to parse: {e}"));
                }
            }
            Err(e) => failures.push(format!("{name}: cannot read input.svelte: {e}")),
        }

        // expected_server.js must be a canonicalize fixed point.
        match std::fs::read_to_string(fixture.expected_server_js_path()) {
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
            }
            Err(e) => failures.push(format!("{name}: cannot read expected_server.js: {e}")),
        }

        // expected.css, when present, must be non-empty.
        let css_path = fixture.expected_css_path();
        if css_path.exists() {
            match std::fs::read_to_string(&css_path) {
                Ok(css) if css.trim().is_empty() => {
                    failures.push(format!("{name}: expected.css is empty"));
                }
                Ok(_) => {}
                Err(e) => failures.push(format!("{name}: cannot read expected.css: {e}")),
            }
        }

        // TODO(M1): once codegen lands, assert here that
        // `canonicalize_js(tsv_svelte_compile::compile(input).js) == expected` —
        // the pure-Rust ours-vs-expected parity gate.
    }

    assert!(
        failures.is_empty(),
        "{} compile fixture failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
