// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The postfix array/indexed-access type `[` is a `[no LineTerminator here]`
//! position (acorn's `tsParseArrayTypeOrHigher`: `while (!hasPrecedingLineBreak()
//! && eat('['))`): a newline before `[` ends the type via ASI, so `T⏎[K]` parses
//! as `T` then a fresh `[K]` statement, never an indexed-access type `T[K]`. In a
//! class body this surfaces as Gap B (the `[e2]` starts a new member); at
//! statement level it was a silent AST divergence — tsv used to emit
//! `TSIndexedAccessType` where acorn splits. These pin the split (and the
//! same-line control that stays an indexed access) directly on the wire AST.
//!
//! The `asi_after_type_annotation` fixture guards the same split via formatter
//! normalization; this asserts the AST shape itself, independent of the printer.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    let program = tsv_ts::parse(source, &arena, &mut interner).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source, &interner)
}

/// `let a: number⏎[0]` → `let a: number;` (the type is `number`, not `number[0]`)
/// plus a fresh `[0]` array-expression statement.
#[test]
fn var_annotation_newline_bracket_splits() {
    let json = parse_json("let a: number\n[0]");
    assert_eq!(
        json.pointer("/body/0/type").and_then(Value::as_str),
        Some("VariableDeclaration")
    );
    assert_eq!(
        json.pointer("/body/0/declarations/0/id/typeAnnotation/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSNumberKeyword"),
        "the type is `number`, not an indexed access `number[0]`: {json}"
    );
    assert_eq!(
        json.pointer("/body/1/type").and_then(Value::as_str),
        Some("ExpressionStatement"),
        "the `[0]` split off into its own statement: {json}"
    );
    assert_eq!(
        json.pointer("/body/1/expression/type")
            .and_then(Value::as_str),
        Some("ArrayExpression")
    );
}

/// `type T = number⏎[e2]` splits the same way: a `number` alias, then `[e2]`.
#[test]
fn type_alias_newline_bracket_splits() {
    let json = parse_json("type T = number\n[e2]");
    assert_eq!(
        json.pointer("/body/0/type").and_then(Value::as_str),
        Some("TSTypeAliasDeclaration")
    );
    assert_eq!(
        json.pointer("/body/0/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSNumberKeyword"),
        "the alias is `number`, not `number[e2]`: {json}"
    );
    assert_eq!(
        json.pointer("/body/1/type").and_then(Value::as_str),
        Some("ExpressionStatement")
    );
}

/// Control: same-line `number[0]` (no newline) stays an indexed-access type.
#[test]
fn same_line_bracket_is_indexed_access() {
    let json = parse_json("let b: number[0]");
    assert_eq!(
        json.pointer("/body/0/declarations/0/id/typeAnnotation/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSIndexedAccessType"),
        "same-line `number[0]` is an indexed access, not split: {json}"
    );
}

/// Per-bracket check: `A[]⏎[K]` stops after the FIRST `[]` (the second `[` has a
/// line break), so the type is the array `A[]` and `[K]` splits into its own
/// statement. This is what makes the fix a loop-condition check (every bracket),
/// not a first-bracket-only one.
#[test]
fn chained_bracket_newline_stops_per_bracket() {
    let json = parse_json("type T = A[]\n[K]");
    assert_eq!(
        json.pointer("/body/0/type").and_then(Value::as_str),
        Some("TSTypeAliasDeclaration")
    );
    assert_eq!(
        json.pointer("/body/0/typeAnnotation/type")
            .and_then(Value::as_str),
        Some("TSArrayType"),
        "the type is the array `A[]`, not `A[][K]`: {json}"
    );
    assert_eq!(
        json.pointer("/body/1/type").and_then(Value::as_str),
        Some("ExpressionStatement")
    );
}
