// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The postfix array/indexed-access type `[` is a `[no LineTerminator here]`
//! position (acorn's `tsParseArrayTypeOrHigher`: `while (!hasPrecedingLineBreak()
//! && eat('['))`): a newline before `[` ends the type via ASI, so `T⏎[K]` parses
//! as `T` then a fresh `[K]` statement, never an indexed-access type `T[K]`. In a
//! class body this surfaces as Gap B (the `[e2]` starts a new member); at
//! statement level a parser that ignores the line break is a silent AST
//! divergence — it emits `TSIndexedAccessType` where acorn splits. These pin the
//! split (and the same-line control that stays an indexed access) directly on the
//! wire AST.
//!
//! The `asi_after_type_annotation` fixture guards the same split via formatter
//! normalization; this asserts the AST shape itself, independent of the printer.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source))
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
/// statement. The line-break check is therefore a loop condition (every bracket),
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

/// The type-argument LOOKAHEAD reads the same rule, or its verdict hands the type parser
/// a list it cannot finish: `a <⏎B⏎[c] >⏎d` — the layout the formatter emits for a comment
/// between the operand and its index — is the comparison chain `a < B[c] > d`, as the
/// same bytes on one line are (tsc `parseTypeArgumentsInExpression` fails on the broken
/// index and falls back; acorn's `tsParseArrayTypeOrHigher` gate says the same). Read as
/// type arguments it was "Expected '>', found '['".
#[test]
fn type_args_lookahead_newline_bracket_stays_comparison() {
    for source in [
        "a <\nB\n[c] >\nd;",
        "a < B // c\n[c] > d;",
        "a < B\n[c] > d;",
    ] {
        let json = parse_json(source);
        assert_eq!(
            json.pointer("/body/0/expression/type")
                .and_then(Value::as_str),
            Some("BinaryExpression"),
            "a `[` past a line terminator is no index, so the `<` is a comparison: {source:?} → {json}"
        );
        assert_eq!(
            json.pointer("/body/0/expression/left/right/type")
                .and_then(Value::as_str),
            Some("MemberExpression"),
            "`B[c]` is the comparison's member operand: {source:?} → {json}"
        );
    }
}

/// Control: the same index on the operand's line is an indexed-access type, and the
/// call's type arguments — including an index that OPENS with `|`/`&` (the union
/// printer's own leading-pipe layout), bare or in a paren shell, which no expression can.
#[test]
fn type_args_lookahead_same_line_and_leading_operator_index() {
    for source in [
        "fn<A[B]>()",
        "fn<A[| B | C]>()",
        "fn<A[& B & C]>()",
        "fn<A[(| B | C)[]]>()",
        "fn<A[(B)]>()",
        "fn<A[\n| B // c\n| C]>()",
    ] {
        let json = parse_json(source);
        assert_eq!(
            json.pointer("/body/0/expression/type")
                .and_then(Value::as_str),
            Some("CallExpression"),
            "{source:?} is an instantiation call: {json}"
        );
        assert!(
            json.pointer("/body/0/expression/typeArguments/params/0/type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "TSIndexedAccessType"),
            "the type argument is the indexed access: {source:?} → {json}"
        );
    }
}
