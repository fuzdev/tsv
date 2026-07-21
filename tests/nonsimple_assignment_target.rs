// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A *non-simple* assignment target — a call (`foo() = bar`), a literal
//! (`1 >>= 2`), `this` (`this = x`), or any other non-`Reference` left — is not a
//! valid `LeftHandSideExpression` for assignment. But "the left-hand side is not a
//! valid assignment target" is a **static-semantic early-error**, not a syntax
//! error: the assignment grammar (`LeftHandSideExpression = AssignmentExpression`)
//! parses these fine, and the "is it assignable?" refinement is an early-error the
//! spec layers on top. Per tsv's permissive stance (see `crates/tsv_ts/CLAUDE.md`
//! §Sources of truth), the parser **defers** that early-error to the diagnostics
//! layer so the formatter keeps formatting well-formed input — prettier formats all
//! of these, so tsv must parse them.
//!
//! This can't be a fixture: acorn-typescript *rejects* these ("Assigning to
//! rvalue"), so no `expected.json` oracle exists — the shape is pinned here against
//! tsv itself and prettier's formatting.
//!
//! Contrast: a no-declaration `for`-in/of head is an
//! `AssignmentTargetType`/`LeftHandSideExpression` position that is NOT an
//! assignment context, so a non-simple head there stays a parse error (prettier
//! rejects it too — `for_head_*` guards below).

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    let program = tsv_ts::parse(source, &arena, &mut interner).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source, &interner)
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    let program = tsv_ts::parse(source, &arena, &mut interner).expect("parse failed");
    tsv_ts::format(&program, source, &interner)
}

fn rejects(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    tsv_ts::parse(source, &arena, &mut interner).is_err()
}

/// `foo() = bar;` parses as an `AssignmentExpression` whose left is the
/// `CallExpression` (kept as-is — the invalid-target check is deferred).
#[test]
fn call_assignment_target_parses() {
    let json = parse_json("foo() = bar;");
    let e = "/body/0/expression";
    assert_eq!(
        json.pointer(&format!("{e}/type")).and_then(Value::as_str),
        Some("AssignmentExpression"),
        "non-simple `=` left still yields an AssignmentExpression: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{e}/operator"))
            .and_then(Value::as_str),
        Some("="),
    );
    assert_eq!(
        json.pointer(&format!("{e}/left/type"))
            .and_then(Value::as_str),
        Some("CallExpression"),
        "the call is kept as the assignment left: {json}"
    );
}

/// `foo() += 1;` — a compound assignment operator over a non-simple (call) target
/// also parses, with the operator preserved.
#[test]
fn call_compound_assignment_target_parses() {
    let json = parse_json("foo() += 1;");
    let e = "/body/0/expression";
    assert_eq!(
        json.pointer(&format!("{e}/type")).and_then(Value::as_str),
        Some("AssignmentExpression"),
    );
    assert_eq!(
        json.pointer(&format!("{e}/operator"))
            .and_then(Value::as_str),
        Some("+="),
    );
    assert_eq!(
        json.pointer(&format!("{e}/left/type"))
            .and_then(Value::as_str),
        Some("CallExpression"),
    );
}

/// `1 >>= 2;` — a literal left with a compound operator. acorn rejects it; tsv
/// defers, keeping the `Literal` as the left.
#[test]
fn literal_compound_assignment_target_parses() {
    let json = parse_json("1 >>= 2;");
    let e = "/body/0/expression";
    assert_eq!(
        json.pointer(&format!("{e}/type")).and_then(Value::as_str),
        Some("AssignmentExpression"),
    );
    assert_eq!(
        json.pointer(&format!("{e}/operator"))
            .and_then(Value::as_str),
        Some(">>="),
    );
    assert_eq!(
        json.pointer(&format!("{e}/left/type"))
            .and_then(Value::as_str),
        Some("Literal"),
        "the literal is kept as the assignment left: {json}"
    );
}

/// `this = x;` — `this` is a non-simple target (a `ThisExpression`, not a
/// `Reference`). Parses with the `ThisExpression` left.
#[test]
fn this_assignment_target_parses() {
    let json = parse_json("this = x;");
    assert_eq!(
        json.pointer("/body/0/expression/left/type")
            .and_then(Value::as_str),
        Some("ThisExpression"),
        "`this` is kept as the assignment left: {json}"
    );
}

/// `(foo()) = bar;` — a *parenthesized* non-simple target parses too. prettier
/// strips the redundant grouping parens, so tsv formats it to `foo() = bar;` and is
/// idempotent from there.
#[test]
fn parenthesized_call_assignment_target_parses_and_formats() {
    let json = parse_json("(foo()) = bar;");
    assert_eq!(
        json.pointer("/body/0/expression/left/type")
            .and_then(Value::as_str),
        Some("CallExpression"),
        "the parens are transparent grouping; the call is the left: {json}"
    );
    let canonical = "foo() = bar;\n";
    assert_eq!(
        format("(foo()) = bar;"),
        canonical,
        "prettier strips the redundant grouping parens"
    );
    assert_eq!(format(canonical), canonical, "idempotent");
}

/// Each deferred-target form formats to its prettier-canonical shape and is
/// idempotent (no data loss on the round-trip).
#[test]
fn deferred_targets_format_to_prettier_canonical() {
    for canonical in [
        "foo() = bar;\n",
        "foo() += 1;\n",
        "1 >>= 2;\n",
        "this = x;\n",
    ] {
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// Regression guard: a no-declaration `for`-in head with a non-simple target stays
/// a parse error — it's a `LeftHandSideExpression`/`AssignmentTargetType` position,
/// not an assignment context, so the deferral does not reach it (prettier rejects
/// it too).
#[test]
fn for_head_call_target_still_rejected() {
    assert!(
        rejects("for (foo() in b) {}"),
        "a non-simple for-in head stays a parse error"
    );
}

/// Regression guard: the `for`-of counterpart with a `new` target also stays a
/// parse error.
#[test]
fn for_head_new_target_still_rejected() {
    assert!(
        rejects("for (new C() of xs) {}"),
        "a non-simple for-of head stays a parse error"
    );
}

/// Regression guard: the Binding-adjacent cover-grammar transforms are unaffected —
/// a normal array-destructuring assignment still converts its left to an
/// `ArrayPattern`.
#[test]
fn array_destructuring_assignment_unaffected() {
    assert_eq!(
        parse_json("[a, b] = c;")
            .pointer("/body/0/expression/left/type")
            .and_then(Value::as_str),
        Some("ArrayPattern"),
    );
}

/// Regression guard: object-destructuring assignment still converts its left to an
/// `ObjectPattern`.
#[test]
fn object_destructuring_assignment_unaffected() {
    assert_eq!(
        parse_json("({ a } = c);")
            .pointer("/body/0/expression/left/type")
            .and_then(Value::as_str),
        Some("ObjectPattern"),
    );
}

// -- Cast-wrapping targets: the same deferral, reached through the cast arms --
//
// A type-assertion wrapping a *non-simple* target (a destructuring pattern), and a
// nested *parenthesized* cast, are two more "invalid assignment target"
// early-errors acorn rejects ("Assigning to rvalue") but prettier formats — so they
// defer too. These previously lived as `input_invalid_*` variants under
// `tests/fixtures/typescript/expressions/assignment/cast_target*`; because acorn
// rejects them they can't be fixtures (no `expected.json` oracle), and now that they
// parse they no longer belong in an `input_invalid_*` slot, so their coverage moved
// here alongside the other deferred targets.

/// `([a, b] as T) = c;` — a type-assertion wrapping a destructuring array. The cast
/// arm only accepts a *simple* inner, so this falls through to the deferral; the
/// internal AST keeps the `TSAsExpression` (the formatter reproduces the `as T`),
/// while the convert layer unwraps the cast from the `=` left, so the *wire* left is
/// the inner `ArrayExpression`. It formats to prettier's canonical form and is
/// idempotent. (acorn rejects, so there's no oracle for the wire shape — pinned to
/// tsv's own unwrap behavior.)
#[test]
fn cast_wrapping_destructure_target_parses_and_formats() {
    assert_eq!(
        parse_json("([a, b] as T) = c;")
            .pointer("/body/0/expression/left/type")
            .and_then(Value::as_str),
        Some("ArrayExpression"),
        "convert unwraps the cast from the `=` left, exposing the inner array",
    );
    let canonical = "([a, b] as T) = c;\n";
    assert_eq!(format(canonical), canonical, "idempotent");
}

/// `({ a: (b as T) } = obj);` — a *parenthesized* nested cast (no default). The
/// nested-cast arm rejects the parenthesized form, so it falls through to the
/// deferral. prettier strips the redundant parens, so tsv formats it to
/// `({ a: b as T } = obj);` and is idempotent from there.
#[test]
fn nested_parenthesized_cast_target_parses_and_formats() {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    assert!(
        tsv_ts::parse("({ a: (b as T) } = obj);", &arena, &mut interner).is_ok(),
        "a nested parenthesized cast in a destructuring assignment parses",
    );
    let canonical = "({ a: b as T } = obj);\n";
    assert_eq!(
        format("({ a: (b as T) } = obj);"),
        canonical,
        "prettier strips the redundant parens around the nested cast",
    );
    assert_eq!(format(canonical), canonical, "idempotent");
}
