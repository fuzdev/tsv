// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Parser coverage for redundantly-parenthesized types at operand positions
//! inside an arrow function's return-type annotation — `(): A & ([b]) => x` and
//! siblings. At a union/intersection constituent the grammar has no function
//! types, so the `(…)` is a parenthesized type and the trailing `=>` belongs to
//! the enclosing arrow (acorn's shape; `fn_type_disallowed` in `tsv_ts`'s
//! parser).
//!
//! These can't be fixtures: acorn accepts them but tsc/prettier reject
//! ("Function type notation must be parenthesized when used in an intersection
//! type"), and tsv strips the redundant parens (`A & ([b])` → `A & [b]`), so
//! the input is not idempotent — the fixture pipeline requires both (F1/F3).
//! The operand mechanism itself is fixture-covered
//! (`types/function_type_operand`, `expressions/arrow/return_type_nested_function_type`,
//! `expressions/arrow/return_type_intersection_paren_union`); these tests pin
//! the redundant-paren arrow-position accepts those can't hold. Each test also
//! asserts the non-idempotent premise (output ≠ input), so a formatter change
//! that starts preserving the parens flags these cases for promotion into real
//! fixtures.

/// Parses `source`, asserts the arrow shape (body is the identifier `x`, the
/// return type is an intersection whose second member is a parenthesized
/// type), then asserts tsv formats it to `expected_output` (≠ `source`) and
/// that the output is a stable fixed point preserving the same arrow shape.
fn assert_operand_paren_arrow(source: &str, expected_output: &str) {
    assert_ne!(
        source, expected_output,
        "premise: these inputs are non-idempotent (tsv strips the redundant \
         parens) — if they become idempotent, promote to a fixture"
    );

    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    let json = tsv_ts::convert_ast_json(&program, source);

    let init = json
        .pointer("/body/0/declarations/0/init")
        .expect("arrow init");
    assert_eq!(
        init.pointer("/body/name").and_then(|v| v.as_str()),
        Some("x"),
        "the trailing `=>` belongs to the enclosing arrow, so its body is `x`: {init}"
    );
    let return_type = init
        .pointer("/returnType/typeAnnotation")
        .expect("return type");
    assert_eq!(
        return_type.pointer("/type").and_then(|v| v.as_str()),
        Some("TSIntersectionType"),
        "return type is the whole intersection: {return_type}"
    );
    assert_eq!(
        return_type
            .pointer("/types/1/type")
            .and_then(|v| v.as_str()),
        Some("TSParenthesizedType"),
        "the `(…)` operand is a parenthesized type, not function params: {return_type}"
    );

    let output = tsv_ts::format(&program, source);
    assert_eq!(output, expected_output, "paren-strip output");

    // The stripped form is tsv's fixed point and keeps the same arrow shape.
    let arena_out = bumpalo::Bump::new();
    let reparsed = tsv_ts::parse(&output, &arena_out).expect("reparse failed");
    assert_eq!(
        tsv_ts::format(&reparsed, &output),
        output,
        "output should be stable"
    );
    let json_out = tsv_ts::convert_ast_json(&reparsed, &output);
    assert_eq!(
        json_out
            .pointer("/body/0/declarations/0/init/body/name")
            .and_then(|v| v.as_str()),
        Some("x"),
        "stripped form keeps the arrow body"
    );
}

#[test]
fn intersection_paren_pattern_operand() {
    assert_operand_paren_arrow(
        "const f = (): A & ([b]) => x;",
        "const f = (): A & [b] => x;\n",
    );
}

#[test]
fn intersection_paren_identifier_operand() {
    assert_operand_paren_arrow("const f = (): A & (B) => x;", "const f = (): A & B => x;\n");
}

#[test]
fn intersection_paren_keyword_operand() {
    assert_operand_paren_arrow(
        "const f = (): A & (number) => x;",
        "const f = (): A & number => x;\n",
    );
}
