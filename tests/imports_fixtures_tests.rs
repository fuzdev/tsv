//! The import-record fixture gate (`tests/fixtures_imports/`) — pure Rust, so it runs in
//! every `cargo test --workspace`.
//!
//! Per fixture, `tsv_ts::parse_imports` (an `input.ts`) or `tsv_svelte::parse_imports` (an
//! `input.svelte`), written by the bindings' JSON writer, must produce the committed
//! `expected.json`; and the tree must keep its shape (every directory a fixture or a
//! container; a fixture its one input, `expected.json` and `README.md`, at most an
//! `oracle.txt`). No ratchet: every failing fixture fails the test.
//!
//! The committed files' agreement with the oracle (the canonical parsers' trees) is
//! `deno task imports:fixtures:validate`'s, which needs the Deno sidecar and so stays out
//! of this test.

use std::path::Path;
use tsv_debug::diff::{DiffOptions, diff_to_string};
use tsv_debug::imports_fixtures::{
    IMPORTS_FIXTURES_DIR, ours_json, render_expected, walk_imports_fixtures,
};

#[test]
fn test_all_import_fixtures() {
    let fixtures = match walk_imports_fixtures(Path::new(IMPORTS_FIXTURES_DIR)) {
        Ok(fixtures) => fixtures,
        Err(e) => panic!("the import fixture tree is malformed: {e}"),
    };
    assert!(
        !fixtures.is_empty(),
        "no import fixtures under {IMPORTS_FIXTURES_DIR}"
    );
    let mut failures = Vec::new();
    for fixture in &fixtures {
        let name = &fixture.relative_path;
        for problem in fixture.structure_problems() {
            failures.push(format!("{name}: {problem}"));
        }
        let committed = match std::fs::read_to_string(fixture.expected_path()) {
            Ok(text) => text,
            Err(e) => {
                failures.push(format!("{name}: reading expected.json: {e}"));
                continue;
            }
        };
        let expected: serde_json::Value = match tsv_debug::json::from_str(&committed) {
            Ok(value) => value,
            Err(e) => {
                failures.push(format!("{name}: expected.json does not parse: {e}"));
                continue;
            }
        };
        let ours = fixture
            .read_input()
            .and_then(|source| ours_json(fixture.language, &source));
        match ours {
            Ok(ours) if ours == expected => {}
            Ok(ours) => {
                let rendered = render_expected(&ours).unwrap_or_default();
                failures.push(format!(
                    "{name}: records differ\n{}",
                    diff_to_string(&committed, &rendered, &DiffOptions::freshness())
                ));
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failure(s) over {} import fixture(s):\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n")
    );
}
