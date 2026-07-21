// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Parser coverage for the bare parenthesized this-type: `(this)` at a
//! full-type position parses as `TSParenthesizedType` wrapping `TSThisType`
//! (acorn's shape), not a `TSTypeReference` named `this`. The `(` first parses
//! as a possible function-type parameter list — `this` is a valid parameter
//! name (`(this: T) => U`) — so the single-bare-identifier reinterpretation in
//! `parse_parenthesized_or_function_type` must special-case `this`. At an
//! operand position (`keyof (this)`) the paren short-circuits to a
//! parenthesized type and reaches the this-type directly; both paths must
//! agree.
//!
//! This can't be a fixture: both prettier and tsv strip the redundant parens
//! (`type A = (this);` → `type A = this;`), so the input is not idempotent and
//! the AST shape only surfaces on unformatted source (the fixture pipeline
//! requires F1 idempotency). Each test also asserts the non-idempotent premise
//! (output ≠ input), so a formatter change that starts preserving the parens
//! flags the case for promotion into a real fixture.

/// Parses `source`, asserts the type at `pointer` is a `TSParenthesizedType`
/// wrapping a `TSThisType`, then asserts tsv formats it to `expected_output`
/// (≠ `source`) and that the output is a stable fixed point.
fn assert_paren_this(source: &str, pointer: &str, expected_output: &str) {
    assert_ne!(
        source, expected_output,
        "premise: these inputs are non-idempotent (tsv strips the redundant \
         parens) — if they become idempotent, promote to a fixture"
    );

    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    let program = tsv_ts::parse(source, &arena, &mut interner).expect("parse failed");
    let json = tsv_ts::convert_ast_json(&program, source, &interner);

    let paren = json.pointer(pointer).expect("parenthesized type");
    assert_eq!(
        paren.pointer("/type").and_then(|v| v.as_str()),
        Some("TSParenthesizedType"),
        "outer node is the paren: {paren}"
    );
    assert_eq!(
        paren
            .pointer("/typeAnnotation/type")
            .and_then(|v| v.as_str()),
        Some("TSThisType"),
        "inner is the this-type, not a type reference named `this`: {paren}"
    );

    let output = tsv_ts::format(&program, source, &interner);
    assert_eq!(output, expected_output, "paren-strip output");

    // The stripped form is tsv's fixed point.
    let arena_out = bumpalo::Bump::new();
    let mut interner_out = tsv_ts::Interner::new();
    let reparsed = tsv_ts::parse(&output, &arena_out, &mut interner_out).expect("reparse failed");
    assert_eq!(
        tsv_ts::format(&reparsed, &output, &interner_out),
        output,
        "output should be stable"
    );
}

/// Full-type position: the `(` speculatively parses function-type params, so
/// the bare `this` arrives via the single-identifier reinterpretation.
#[test]
fn paren_this_full_type_position() {
    assert_paren_this(
        "type A = (this);",
        "/body/0/typeAnnotation",
        "type A = this;\n",
    );
}

/// Operand position: `fn_type_disallowed` short-circuits the `(` to a
/// parenthesized type, reaching the this-type through the primary type parse.
#[test]
fn paren_this_operand_position() {
    assert_paren_this(
        "type B = keyof (this);",
        "/body/0/typeAnnotation/typeAnnotation",
        "type B = keyof this;\n",
    );
}
