// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Format-stability pins for the `<T>` type-assertion vs. generic-arrow boundary
//! in standalone TS.
//!
//! The tsv-rejects/acorn-accepts *rejections* — `<T>x => x` and `<T,>(() => {})`
//! — moved to the fixture pipeline, which can now express them: an
//! `input_invalid_*` fixture requires BOTH parsers to reject, but here acorn
//! accepts, so they are `tsv_rejects.txt` fixtures under
//! `tests/fixtures/typescript/expressions/type_assertion_arrow/` (`operand` +
//! `type_params`) that verify both halves and self-heal. What stays here are the
//! *accept* boundaries: standalone-TS forms that round-trip stably, contrasting
//! the rejected forms. Cataloged in `docs/conformance_svelte.md` §Type assertion
//! vs. generic arrow.

/// tsv parses + formats standalone-TS `input` to itself, then re-formats
/// stably (idempotent).
fn assert_ours_stable(input: &str) {
    let arena = bumpalo::Bump::new();
    let ast = tsv_ts::parse(input, &arena).expect("parse failed");
    let output = tsv_ts::format(&ast, input);
    assert_eq!(output, input, "printer should keep the form stable");

    let arena_twice = bumpalo::Bump::new();
    let ast_twice = tsv_ts::parse(&output, &arena_twice).expect("reparse failed");
    let output_twice = tsv_ts::format(&ast_twice, &output);
    assert_eq!(output, output_twice, "printer should be idempotent");
}

/// An assertion over a *parenthesized* arrow keeps the assertion reading and
/// round-trips stably (the AST shape itself is fixture-pinned by
/// `type_assertion_paren_arrow_svelte_divergence`).
#[test]
fn type_assertion_over_paren_arrow_stable() {
    assert_ours_stable("<any>(() => {});\n");
    assert_ours_stable("<T>(() => {});\n");
}

/// Boundary: the ordinary generic-arrow forms stay generic arrows, and an
/// assertion whose type can't parse as type parameters stays an assertion.
#[test]
fn generic_arrow_and_assertion_boundaries_stable() {
    assert_ours_stable("const fn = <T>(x: T) => x;\n");
    assert_ours_stable("<any[]>(() => {});\n");
    assert_ours_stable("<any>x;\n");
}
