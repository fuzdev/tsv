// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Formatter coverage for the Stage-3 import-phase proposals — source-phase
//! imports (`import source …` / `import.source(…)`) and import defer
//! (`import defer * as ns …` / `import.defer(…)`).
//!
//! The *parser* is graded by the test262 suite (see `docs/conformance_test262.md`);
//! these tests cover the *printer*, which test262 never exercises. acorn rejects
//! this syntax, so there is no fixture parse oracle, and prettier either drops the
//! phase (`import defer`) or throws (`import source`) — so these can't be fixtures
//! either. Each test asserts a single-pass stable round-trip plus idempotency.
//!
//! The prettier divergences are cataloged in `docs/conformance_prettier.md` and
//! `docs/conformance_svelte.md`. The `import source` form (prettier throws) is
//! also live-pinned in `tests/prettier_error_bugs.rs`; the `import defer` form
//! (prettier silently drops the phase) is documented-only — a live "prettier
//! succeeds with wrong output" assertion would gate the suite on a sidecar call
//! under load, which is needless fragility for a niche Stage-3 divergence.

/// tsv parses + formats `input` to itself, then re-formats stably (idempotent).
fn assert_ours_stable(input: &str) {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(input, &arena).expect("parse failed");
    let output = tsv_svelte::format(&ast, input);
    assert_eq!(
        output, input,
        "printer should keep import-phase syntax stable"
    );

    let arena_twice = bumpalo::Bump::new();
    let ast_twice = tsv_svelte::parse(&output, &arena_twice).expect("reparse failed");
    let output_twice = tsv_svelte::format(&ast_twice, &output);
    assert_eq!(output, output_twice, "printer should be idempotent");
}

#[test]
fn dynamic_import_source_stable() {
    assert_ours_stable("<script lang=\"ts\">\n\timport.source('x');\n</script>\n");
}

#[test]
fn dynamic_import_defer_stable() {
    assert_ours_stable("<script lang=\"ts\">\n\timport.defer('x');\n</script>\n");
}

#[test]
fn static_import_defer_namespace_stable() {
    assert_ours_stable("<script lang=\"ts\">\n\timport defer * as ns from 'x';\n</script>\n");
}

#[test]
fn static_import_source_binding_stable() {
    assert_ours_stable("<script lang=\"ts\">\n\timport source x from 'x';\n</script>\n");
}
