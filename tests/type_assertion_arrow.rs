// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Parser pins for the `<T>` type-assertion vs. generic-arrow disambiguation —
//! deliberate divergences from acorn-typescript the fixture path can't reach
//! (an `input_invalid_*` fixture requires BOTH parsers to reject; here the
//! canonical parser accepts). Cataloged in `docs/conformance_svelte.md`
//! §Type assertion vs. generic arrow.
//!
//! acorn-typescript tries the generic-arrow reading first, and its
//! Babel-ported "abort on a parenthesized arrow" check is dead code (acorn
//! never sets `extra.parenthesized`), so `<T>` followed by ANY arrow parses as
//! the arrow's type parameters. TypeScript instead reads a type assertion in
//! `.ts` (JSX-free) files. tsv follows TypeScript:
//!
//! - `<any>(() => {})` — an assertion over the parenthesized arrow (the
//!   `type_assertion_paren_arrow_svelte_divergence` fixture pins the AST).
//! - `<T>x => x` — rejected: an arrow is not a UnaryExpression, so it cannot
//!   be the assertion's operand (acorn: generic arrow; TypeScript: error).
//! - `<T,>(() => {})` — rejected: `T,` is not an assertion type, and tsv does
//!   not backtrack into the generic-arrow reading over a parenthesized arrow
//!   (acorn: generic arrow; TypeScript: error).
//!
//! Boundary checks keep the real generic-arrow forms parsing.

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

fn assert_ours_rejects(input: &str) {
    let arena = bumpalo::Bump::new();
    assert!(
        tsv_ts::parse(input, &arena).is_err(),
        "expected parse rejection (pinned divergence): {input}"
    );
}

/// An assertion over a *parenthesized* arrow keeps the assertion reading and
/// round-trips stably (the AST shape itself is fixture-pinned).
#[test]
fn type_assertion_over_paren_arrow_stable() {
    assert_ours_stable("<any>(() => {});\n");
    assert_ours_stable("<T>(() => {});\n");
}

/// An *unparenthesized* arrow is not a valid assertion operand.
#[test]
fn type_assertion_unparenthesized_arrow_operand_rejected() {
    assert_ours_rejects("<T>x => x;");
    assert_ours_rejects("<T>async x => x;");
}

/// `<T,>` cannot open a type assertion, and tsv doesn't re-read it as a
/// generic arrow's type parameters over a parenthesized arrow.
#[test]
fn trailing_comma_type_params_before_paren_arrow_rejected() {
    assert_ours_rejects("<T,>(() => {});");
}

/// Boundary: the ordinary generic-arrow forms stay generic arrows, and an
/// assertion whose type can't parse as type parameters stays an assertion.
#[test]
fn generic_arrow_and_assertion_boundaries_stable() {
    assert_ours_stable("const fn = <T>(x: T) => x;\n");
    assert_ours_stable("<any[]>(() => {});\n");
    assert_ours_stable("<any>x;\n");
}
