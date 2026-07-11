// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Precedence of TypeScript `as` / `satisfies` relative to the binary operators.
//!
//! `as` and `satisfies` bind at **relational** precedence — tsc's
//! `getBinaryOperatorPrecedence` returns `OperatorPrecedence.Relational` for both
//! (the same tier as `<` / `>` / `instanceof` / `in`), and acorn-typescript gates
//! them on `tt._in.binop` (the relational binding power) in its `parseExprOp`
//! override. They are left-associative, and the right-hand side is a *type*
//! (consumed by `parseType`), not an expression.
//!
//! So `x === y as Foo` is `x === (y as Foo)` (the cast binds tighter than
//! equality), while `a + b as T` stays `(a + b) as T` (additive binds tighter
//! than the cast). tsv previously parsed `as` / `satisfies` *below every* binary
//! operator (a two-phase infix loop that finished the whole binary expression
//! first), so it grouped `(x === y) as Foo` — diverging from the AST-shape oracle
//! (acorn) itself.
//!
//! These bare forms escape `corpus:compare:parse` because real code always
//! parenthesizes the cast (`x === (y as Foo)`), so the corpus is no safety net —
//! the acorn differential is the oracle. And the bare divergent forms are not
//! format-stable (prettier adds clarifying parens) yet acorn *accepts* them, so
//! they fit neither a plain fixture (input must format to itself) nor an
//! `input_invalid_*` (acorn accepts). Hence this `.rs` test, pinning both the
//! acorn-matching parse shape and prettier's canonical format output — the same
//! vehicle as `tests/nullish_logical_precedence.rs`.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// Reduce an AST node to a compact, fully-parenthesized grouping signature. Mirrors
/// the differential sweep's oracle reduction so the expected strings below are the
/// verbatim acorn output for each probe.
fn sig(n: &Value) -> String {
    let Some(t) = n.get("type").and_then(Value::as_str) else {
        return "null".to_string();
    };
    let op = |n: &Value| {
        n.get("operator")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string()
    };
    match t {
        "ExpressionStatement" => sig(&n["expression"]),
        "BinaryExpression" | "LogicalExpression" => {
            format!("({} {} {})", sig(&n["left"]), op(n), sig(&n["right"]))
        }
        "AssignmentExpression" => {
            format!("({} {} {})", sig(&n["left"]), op(n), sig(&n["right"]))
        }
        "TSAsExpression" => format!(
            "({} as {})",
            sig(&n["expression"]),
            sig(&n["typeAnnotation"])
        ),
        "TSSatisfiesExpression" => {
            format!(
                "({} satisfies {})",
                sig(&n["expression"]),
                sig(&n["typeAnnotation"])
            )
        }
        "Identifier" => n
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("id")
            .to_string(),
        "TSTypeReference" => n
            .get("typeName")
            .and_then(|tn| tn.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("T")
            .to_string(),
        "TSUnionType" => n
            .get("types")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(sig).collect::<Vec<_>>().join("|"))
            .unwrap_or_default(),
        other => other.to_string(),
    }
}

/// The parse-shape signature of the first statement's expression.
fn expr_sig(source: &str) -> String {
    let json = parse_json(source);
    sig(json.pointer("/body/0/expression").expect("no expression"))
}

/// `as` / `satisfies` bind tighter than every operator below the relational tier
/// (equality, bitwise, logical, `??`), so the cast attaches to the right operand:
/// `x === y as Foo` is `x === (y as Foo)`, not `(x === y) as Foo`. This is the
/// core divergence — tsv previously grouped the whole binary first.
#[test]
fn binds_tighter_than_sub_relational_operators() {
    for (src, expected) in [
        ("x === y as Foo;", "(x === (y as Foo))"),
        ("a == b as T;", "(a == (b as T))"),
        ("a != b as T;", "(a != (b as T))"),
        ("a !== b as T;", "(a !== (b as T))"),
        ("a & b as T;", "(a & (b as T))"),
        ("a ^ b as T;", "(a ^ (b as T))"),
        ("a | b as T;", "(a | (b as T))"),
        ("flag && v as T;", "(flag && (v as T))"),
        ("a || b as T;", "(a || (b as T))"),
        ("a ?? b as T;", "(a ?? (b as T))"),
    ] {
        assert_eq!(expr_sig(src), expected, "shape of {src:?}");
    }
}

/// `satisfies` sits at the same relational tier as `as` — identical grouping.
#[test]
fn satisfies_matches_as_precedence() {
    for (src, expected) in [
        ("x === y satisfies Foo;", "(x === (y satisfies Foo))"),
        ("flag && v satisfies T;", "(flag && (v satisfies T))"),
        ("a | b satisfies T;", "(a | (b satisfies T))"),
    ] {
        assert_eq!(expr_sig(src), expected, "shape of {src:?}");
    }
}

/// Multi-operator chains: each sub-relational operator's right operand absorbs the
/// cast, while the operators keep their own precedence and left-associativity.
#[test]
fn multi_operator_chains() {
    for (src, expected) in [
        ("a === b === c as T;", "((a === b) === (c as T))"),
        ("x === y as Foo === z;", "((x === (y as Foo)) === z)"),
        ("a + b === c as T;", "((a + b) === (c as T))"),
        ("a && b || c as T;", "((a && b) || (c as T))"),
        ("w as T === x as U;", "((w as T) === (x as U))"),
    ] {
        assert_eq!(expr_sig(src), expected, "shape of {src:?}");
    }
}

/// Regression guard: operators *tighter* than relational (additive, shift,
/// multiplicative) still bind before the cast, so the cast wraps the whole binary
/// — `a + b as T` is `(a + b) as T`, unchanged. And an `as` on the left is a
/// complete operand: `b as T + c` is `(b as T) + c`.
#[test]
fn tighter_than_relational_operators_unaffected() {
    for (src, expected) in [
        ("a + b as T;", "((a + b) as T)"),
        ("a - b as T;", "((a - b) as T)"),
        ("a * b as T;", "((a * b) as T)"),
        ("a / b as T;", "((a / b) as T)"),
        ("a << b as T;", "((a << b) as T)"),
        ("a >> b as T;", "((a >> b) as T)"),
        ("b as T + c;", "((b as T) + c)"),
        ("b as T * c;", "((b as T) * c)"),
    ] {
        assert_eq!(expr_sig(src), expected, "shape of {src:?}");
    }
}

/// Same-tier relational operators are left-associative with the cast: a cast after
/// a relational operator wraps the whole relational (`a < b as T` → `(a < b) as
/// T`), and a relational after a cast takes the cast as its left operand
/// (`a as T instanceof B` → `(a as T) instanceof B`). Casts after sub-relational
/// operators keep their left-operand form too (`a as T && b` → `(a as T) && b`).
#[test]
fn relational_tier_left_associative() {
    for (src, expected) in [
        ("a < b as T;", "((a < b) as T)"),
        ("a > b as T;", "((a > b) as T)"),
        ("a instanceof B as T;", "((a instanceof B) as T)"),
        ("a as T instanceof B;", "((a as T) instanceof B)"),
        ("a as T && b;", "((a as T) && b)"),
        ("a as T ?? b;", "((a as T) ?? b)"),
        ("a as T === b;", "((a as T) === b)"),
    ] {
        assert_eq!(expr_sig(src), expected, "shape of {src:?}");
    }
}

/// `parse_type` consumes a trailing type union, so the `| b` after `a as T` is a
/// *type* union, not a bitwise-or on an expression: `a as T | b` is `a as (T | b)`.
/// Unchanged by the precedence fix. Chained casts are left-associative.
#[test]
fn union_rhs_and_chained_casts() {
    for (src, expected) in [
        ("a as T | b;", "(a as T|b)"),
        ("a as T as U;", "((a as T) as U)"),
        ("a as T satisfies U;", "((a as T) satisfies U)"),
        ("a satisfies T as U;", "((a satisfies T) as U)"),
    ] {
        assert_eq!(expr_sig(src), expected, "shape of {src:?}");
    }
}

/// A parenthesized cast is the one accepted cast assignment target; the `as T` is
/// stripped to the bare identifier during the assignment-target conversion (both
/// tsv and acorn), and `x = y as T` keeps the cast on the right. Unchanged.
#[test]
fn assignment_target_preserved() {
    assert_eq!(expr_sig("(x as T) = v;"), "(x = v)");
    assert_eq!(expr_sig("x = y as T;"), "(x = (y as T))");
}

/// The parse groupings above render to prettier's canonical output: the formatter
/// adds clarifying parens around every `as` / `satisfies` binary operand and
/// around a binary/logical cast operand, so both directions read unambiguously.
/// Each form is idempotent (formats to itself on the second pass).
#[test]
fn formats_to_prettier_canonical() {
    for (input, canonical) in [
        // sub-relational: cast on the right
        ("x === y as Foo;", "x === (y as Foo);\n"),
        ("a == b as T;", "a == (b as T);\n"),
        ("a !== b as T;", "a !== (b as T);\n"),
        ("a & b as T;", "a & (b as T);\n"),
        ("a ^ b as T;", "a ^ (b as T);\n"),
        ("a | b as T;", "a | (b as T);\n"),
        ("flag && v as T;", "flag && (v as T);\n"),
        ("a || b as T;", "a || (b as T);\n"),
        ("a ?? b as T;", "a ?? (b as T);\n"),
        ("x === y satisfies Foo;", "x === (y satisfies Foo);\n"),
        // chains
        ("x === y as Foo === z;", "(x === (y as Foo)) === z;\n"),
        ("w as T === x as U;", "(w as T) === (x as U);\n"),
        ("a && b || c as T;", "(a && b) || (c as T);\n"),
        // tighter-than-relational: cast wraps the whole binary
        ("a + b as T;", "(a + b) as T;\n"),
        ("b as T + c;", "(b as T) + c;\n"),
        ("a * b as T;", "(a * b) as T;\n"),
        ("a << b as T;", "(a << b) as T;\n"),
        // same-tier relational
        ("a < b as T;", "(a < b) as T;\n"),
        ("a as T instanceof B;", "(a as T) instanceof B;\n"),
        ("a instanceof B as T;", "(a instanceof B) as T;\n"),
        ("a as T && b;", "(a as T) && b;\n"),
        ("a as T ?? b;", "(a as T) ?? b;\n"),
        // union rhs + chained casts (no parens)
        ("a as T | b;", "a as T | b;\n"),
        ("a as T as U;", "a as T as U;\n"),
        ("a as T satisfies U;", "a as T satisfies U;\n"),
        ("a satisfies T as U;", "a satisfies T as U;\n"),
        // assignment target
        ("(x as T) = v;", "(x as T) = v;\n"),
        ("x = y as T;", "x = y as T;\n"),
    ] {
        assert_eq!(format(input), canonical, "format {input:?}");
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}
