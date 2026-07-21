// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Precedence of `??` (nullish coalescing) relative to `||` / `&&`.
//!
//! Mixing `??` with `||`/`&&` without parentheses is a syntax error in the
//! ECMAScript grammar (`CoalesceExpressionHead` can't be a `LogicalORExpression`),
//! so acorn-typescript rejects these — no `expected.json` oracle exists, and the
//! shape is pinned here against tsv itself and prettier's formatting. But tsc's
//! parser is error-tolerant: it accepts the mix (emitting a diagnostic) and
//! **groups `??` at the SAME precedence as `||`, left-associative** (`Coalesce =
//! LogicalOR` in tsc's `OperatorPrecedence`), with `&&` one tier tighter. prettier
//! formats all of these (adding clarifying parens), so per tsv's permissive stance
//! (see `crates/tsv_ts/CLAUDE.md` §Sources of truth) tsv accepts them and must
//! reproduce tsc's grouping — otherwise the formatter emits parens implying the
//! opposite semantics.
//!
//! The divergence only surfaces when `??` is leftmost (`a ?? b || c`): tsv
//! previously gave `??` a precedence *below* `||`, grouping `a ?? (b || c)` where
//! tsc groups `(a ?? b) || c`. When `||`/`&&` is leftmost the groupings coincide.

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

fn op_at(json: &Value, pointer: &str) -> Option<String> {
    json.pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// `a ?? b || c` groups as `(a ?? b) || c` — `||` at the top (same precedence as
/// `??`, left-associative), the `??` nested as its left. This is the leftmost-`??`
/// case that previously diverged.
#[test]
fn nullish_then_or_groups_left() {
    let json = parse_json("a ?? b || c;");
    let e = "/body/0/expression";
    assert_eq!(
        op_at(&json, &format!("{e}/type")).as_deref(),
        Some("LogicalExpression"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/operator")).as_deref(),
        Some("||"),
        "`||` is the top operator (same precedence as `??`, left-assoc): {json}"
    );
    assert_eq!(
        op_at(&json, &format!("{e}/left/operator")).as_deref(),
        Some("??"),
        "`a ?? b` is the left operand of `||`: {json}"
    );
    assert_eq!(
        op_at(&json, &format!("{e}/right/type")).as_deref(),
        Some("Identifier"),
    );
}

/// `a ?? b || c && d` groups as `(a ?? b) || (c && d)` — `&&` is tighter than the
/// `??`/`||` tier, so it binds `c && d` on the right.
#[test]
fn nullish_or_and_grouping() {
    let json = parse_json("a ?? b || c && d;");
    let e = "/body/0/expression";
    assert_eq!(
        op_at(&json, &format!("{e}/operator")).as_deref(),
        Some("||"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/left/operator")).as_deref(),
        Some("??"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/right/operator")).as_deref(),
        Some("&&"),
        "`c && d` binds on the right (`&&` is tighter): {json}"
    );
}

/// The leftmost-`||`/`&&` cases were already correct (same-precedence left-assoc
/// gives the same grouping whichever loose operator is first): `a || b ?? c` →
/// `(a || b) ?? c`, `a && b ?? c` → `(a && b) ?? c`.
#[test]
fn or_or_and_then_nullish_group_left() {
    let json = parse_json("a || b ?? c;");
    let e = "/body/0/expression";
    assert_eq!(
        op_at(&json, &format!("{e}/operator")).as_deref(),
        Some("??"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/left/operator")).as_deref(),
        Some("||"),
    );

    let json = parse_json("a && b ?? c;");
    assert_eq!(
        op_at(&json, &format!("{e}/operator")).as_deref(),
        Some("??"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/left/operator")).as_deref(),
        Some("&&"),
    );
}

/// `a || b && c ?? d` groups as `(a || (b && c)) ?? d` — `??`/`||` at the loose
/// tier, `&&` tighter.
#[test]
fn or_and_then_nullish_grouping() {
    let json = parse_json("a || b && c ?? d;");
    let e = "/body/0/expression";
    assert_eq!(
        op_at(&json, &format!("{e}/operator")).as_deref(),
        Some("??"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/left/operator")).as_deref(),
        Some("||"),
        "`a || (b && c)` is the left of `??`: {json}"
    );
    assert_eq!(
        op_at(&json, &format!("{e}/left/right/operator")).as_deref(),
        Some("&&"),
    );
}

/// Each mixed form formats to its prettier-canonical shape (clarifying parens the
/// grouping the AST above pins) and is idempotent — no data loss on the round-trip.
#[test]
fn mixed_forms_format_to_prettier_canonical() {
    for (input, canonical) in [
        ("a ?? b || c;", "(a ?? b) || c;\n"),
        ("a || b ?? c;", "(a || b) ?? c;\n"),
        ("a ?? b || c && d;", "(a ?? b) || (c && d);\n"),
        ("a || b && c ?? d;", "(a || (b && c)) ?? d;\n"),
    ] {
        assert_eq!(format(input), canonical, "format {input:?}");
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// Regression guards: pure same-operator chains are unaffected (no cross-operator
/// precedence question), and left-associativity is preserved.
#[test]
fn pure_chains_unaffected() {
    // `a ?? b ?? c` stays a left-nested `??` chain.
    let json = parse_json("a ?? b ?? c;");
    let e = "/body/0/expression";
    assert_eq!(
        op_at(&json, &format!("{e}/operator")).as_deref(),
        Some("??"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/left/operator")).as_deref(),
        Some("??"),
    );
    for stable in ["a ?? b ?? c;\n", "a || b || c;\n", "a && b && c;\n"] {
        assert_eq!(format(stable), stable, "pure chain stable: {stable:?}");
    }
}

/// Regression guard: `&&` still binds tighter than `||` in ordinary (non-nullish)
/// logical expressions — `a || b && c` is `a || (b && c)`.
#[test]
fn and_still_tighter_than_or() {
    let json = parse_json("a || b && c;");
    let e = "/body/0/expression";
    assert_eq!(
        op_at(&json, &format!("{e}/operator")).as_deref(),
        Some("||"),
    );
    assert_eq!(
        op_at(&json, &format!("{e}/right/operator")).as_deref(),
        Some("&&"),
        "`b && c` binds tighter: {json}"
    );
}

/// Regression guard: explicitly parenthesized mixes are grammatical and unaffected
/// — the parens are honored and the forms round-trip stably.
#[test]
fn parenthesized_mixes_stable() {
    for stable in ["(a ?? b) || c;\n", "a ?? (b || c);\n", "(a || b) ?? c;\n"] {
        assert_eq!(format(stable), stable, "parenthesized stable: {stable:?}");
    }
}
