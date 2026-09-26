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
//! The prettier-canonical shapes ARE a fixture —
//! `typescript/expressions/assignment/nonsimple_target_svelte_divergence`. An
//! acorn *rejection* is representable (`expected_ours.json` plus an
//! `expected_svelte.json` holding `{"error": "failed to parse"}`), and it is the
//! only form that pins the prettier side against a live oracle, so the formatting
//! claim cannot go stale the way a hand-written expected string can. What stays
//! here is what a fixture cannot assert: the **node types** below (an
//! over-permissive parser can accept a widened form while building the wrong node
//! for it), the `for`-head rejections, and the cast arms whose wire shape has no
//! oracle at all.
//!
//! Contrast: a no-declaration `for`-in/of head is an
//! `AssignmentTargetType`/`LeftHandSideExpression` position that is NOT an
//! assignment context, so a non-simple head there stays a parse error (prettier
//! rejects it too — `for_head_*` guards below). What the for-head shares with the
//! `=` left is the *simple* target under its wrappers — an assertion or a JSDoc cast
//! over an identifier or member is a target in both (tsc's `checkReferenceExpression`
//! reads the two positions with one rule; the fixture
//! `typescript/expressions/assignment/cast_target_for_head_svelte_divergence` pins
//! the family against prettier) — and the two for-head shapes no fixture can hold sit
//! below: the JSDoc cast over a nested assertion, whose parens prettier's TypeScript
//! parser strips in a for head, and the sealed optional chain.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source))
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn rejects(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_err()
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

/// A no-declaration for-head reads its target through a JSDoc cast the way an `=`
/// left does — the cast's parens are transparent grouping, and the assertion under
/// them still wraps a simple target — as a pattern child too. acorn rejects the
/// nested form ("Unexpected type cast in parameter position"), and prettier's
/// TypeScript parser strips a JSDoc cast's parens in a for head, so no `input.*` can
/// hold this as a fixed point: pinned here, to tsv's own reprint, which keeps the cast.
#[test]
fn for_head_nested_jsdoc_cast_over_assertion_parses() {
    let json = parse_json("for ([/** @type {T} */ (a as U)] of xs) {}");
    assert_eq!(
        json.pointer("/body/0/left/elements/0/type")
            .and_then(Value::as_str),
        Some("TSAsExpression"),
        "the JSDoc cast unwraps at convert; the assertion is the element: {json}"
    );
    let canonical = "for ([/** @type {T} */ (a as U)] of xs) {\n}\n";
    assert_eq!(format(canonical), canonical, "idempotent");
}

/// `for await` reads its head through the same conversion.
#[test]
fn for_await_head_assertion_target_parses() {
    assert_eq!(
        parse_json("async function f() { for await (a! of xs) {} }")
            .pointer("/body/0/body/body/0/left/type")
            .and_then(Value::as_str),
        Some("TSNonNullExpression"),
    );
}

/// An optional chain is no for-head target, however it is wrapped: ecma262 gives an
/// `OptionalExpression` the invalid `AssignmentTargetType`, tsc's checker rejects it
/// (TS2780 / TS2781) and acorn has no `left` that could carry its `ChainExpression`. The
/// non-null and JSDoc-cast spellings are the same chain one node up, and a pattern
/// child is graded the same way.
#[test]
fn for_head_optional_chain_target_rejected() {
    for source in [
        "for (a?.b of xs) {}",
        "for (a?.b in o) {}",
        "for (a?.b! of xs) {}",
        "for (a?.[i].c of xs) {}",
        "for (/** @type {T} */ (a?.b) of xs) {}",
        "for ([a?.b] of xs) {}",
        "for ({ k: a?.b! } of xs) {}",
    ] {
        assert!(
            rejects(source),
            "an optional-chain for-head target rejects: {source}"
        );
    }
}

/// Parens seal a chain: `(a?.b).c` is an ordinary member target, in a for-head as
/// everywhere (tsc: no `OptionalChain` flag on the outer access; acorn: no
/// `ChainExpression` around it).
#[test]
fn for_head_sealed_optional_chain_target_parses() {
    let json = parse_json("for ((a?.b).c of xs) {}");
    assert_eq!(
        json.pointer("/body/0/left/type").and_then(Value::as_str),
        Some("MemberExpression"),
        "the sealed chain's outer access is the target: {json}"
    );
    let canonical = "for ((a?.b).c of xs) {\n}\n";
    assert_eq!(format(canonical), canonical, "idempotent");
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
// `tests/fixtures/typescript/expressions/assignment/cast_target*`, and no longer
// belong in an `input_invalid_*` slot now that they parse. Unlike the plain targets
// above they stay Rust tests on their own merit: acorn rejects both, so the wire shape
// they pin has no oracle reading — a fixture would pin tsv's own output as if it were
// one.

/// `([a, b] as T) = c;` — a type-assertion wrapping a destructuring array. The cast
/// arm only accepts a *simple* inner, so this falls through to the deferral, and the
/// `TSAsExpression` is what the `=` left carries on the wire as well as internally
/// (the formatter reproduces the `as T`). It formats to prettier's canonical form and
/// is idempotent. (acorn rejects, so there's no oracle for the wire shape — pinned to
/// tsv's own behavior; the assertion-preserving half of it is the shape acorn *does*
/// produce for the simple targets it accepts.)
#[test]
fn cast_wrapping_destructure_target_parses_and_formats() {
    assert_eq!(
        parse_json("([a, b] as T) = c;")
            .pointer("/body/0/expression/left/type")
            .and_then(Value::as_str),
        Some("TSAsExpression"),
        "the `=` left keeps its assertion wrapper",
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
    assert!(
        tsv_ts::parse("({ a: (b as T) } = obj);", &arena).is_ok(),
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

/// The deferral stops at the two shapes that have no faithful reprint. A compound
/// operator cannot cover a default (`[a += b] = xs`): the pattern node has no slot
/// for the operator, so converting would DELETE it — rejected in every assignable
/// context (acorn: "Only '=' operator can be used for specifying default value").
#[test]
fn compound_default_rejected_in_every_context() {
    for source in [
        "[a += b] = xs;",
        "({ x: a += b } = o);",
        "(a += b) = 1;",
        "for ([a += b] of xs) {}",
        "function fn([a += b]) {}",
        "([a += b]) => 1;",
    ] {
        assert!(
            rejects(source),
            "a compound default is a syntax error: {source:?}"
        );
    }
    // The plain `=` default and a compound assignment as a VALUE are untouched.
    for canonical in ["[a = b] = xs;\n", "x = a += b;\n", "([a = b]) => 1;\n"] {
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// A parenthesized assignment as the WHOLE target — `(a = b) = 1`,
/// `for ((a = b) of xs)` — would put an `AssignmentPattern` where acorn's grammar has
/// none (`AssignmentExpression.left`, the for-head's `left`): target conversion turns
/// the parenthesized `=` into that pattern node, so no assignment survives for a kept
/// pair to wrap. Rejected ahead of the deferral; the unparenthesized `a = b = 1` is
/// right-associative and unaffected. A parenthesized conditional, arrow or `yield` is
/// never converted and survives as itself — `tests/operator_assignment_target.rs`.
#[test]
fn whole_target_default_pattern_rejected() {
    for source in [
        "(a = b) = 1;",
        "(a = b) += 1;",
        "for ((a = b) of xs) {}",
        "for ((a = b) in obj) {}",
    ] {
        assert!(
            rejects(source),
            "an assignment pattern as the whole target is a syntax error: {source:?}"
        );
    }
    let canonical = "a = b = 1;\n";
    assert_eq!(format(canonical), canonical, "idempotent");
    assert_eq!(
        parse_json(canonical)
            .pointer("/body/0/expression/right/type")
            .and_then(Value::as_str),
        Some("AssignmentExpression"),
        "the unparenthesized chain nests on the RIGHT",
    );
}

/// An instantiation expression is a non-simple target like any other, and `+=` / `-=`
/// must reach it exactly as `*=` does. They are the two compound operators that share a
/// first byte with an EXPRESSION start (`+c`, `-c`), so a follower test that reads the
/// byte instead of the token takes the `+` of `+=` for a unary operand and reads the
/// would-be `<…>` as a comparison chain — which then has no right operand. tsc's
/// `canFollowTypeArgumentsInExpression` refuses the list ahead of `+` / `-` but not ahead
/// of `+=` / `-=`, which are different tokens; acorn-typescript's `tt.assign` is not
/// `startsExpr` either, so both read the instantiation (acorn then rejects the target,
/// the early error tsv defers). Each spelling must build the `*=` tree with only the
/// operator changed, glued (`f<T>+= c`), behind a comment, or past a `>>` close
/// (`f<A<B>>`) alike. The function-type head (`f<(a: T) => U>`) is the one that commits
/// without the lookahead and asks the follower afterwards; `new f<T>` is the one no
/// fixture can hold bare, since prettier and tsv both re-spell it `new f<T>()`, whose `(`
/// commits the list ahead of any follower; and `x = f<T>` puts the target on the right.
#[test]
fn instantiation_target_takes_plus_minus_assign_like_star_assign() {
    let expr = "/body/0/expression";
    let nested = "/body/0/expression/right";
    // (template with `OP` for the operator, the assignment's pointer, its `left` type)
    for (template, at, left) in [
        ("f<T> OP c;", expr, "TSInstantiationExpression"),
        ("f<T>OP c;", expr, "TSInstantiationExpression"),
        ("f<T> /* x */ OP c;", expr, "TSInstantiationExpression"),
        ("f<A<B>> OP c;", expr, "TSInstantiationExpression"),
        ("a.b<T> OP c;", expr, "TSInstantiationExpression"),
        ("new f<T> OP c;", expr, "NewExpression"),
        ("f<(a: T) => U> OP c;", expr, "TSInstantiationExpression"),
        ("x = f<T> OP c;", nested, "TSInstantiationExpression"),
    ] {
        let baseline = template.replace("OP", "*=");
        let sibling = parse_json(&baseline);
        assert_eq!(
            sibling
                .pointer(&format!("{at}/type"))
                .and_then(Value::as_str),
            Some("AssignmentExpression"),
            "the `*=` baseline is an assignment: {baseline:?} {sibling}"
        );
        assert_eq!(
            sibling
                .pointer(&format!("{at}/left/type"))
                .and_then(Value::as_str),
            Some(left),
            "the `*=` baseline's target: {baseline:?} {sibling}"
        );
        let sibling = sibling.to_string();
        for op in ["+=", "-="] {
            let source = template.replace("OP", op);
            assert!(!rejects(&source), "must parse: {source:?}");
            let json = parse_json(&source);
            assert_eq!(
                json.pointer(&format!("{at}/operator"))
                    .and_then(Value::as_str),
                Some(op),
                "the operator: {source:?} {json}"
            );
            assert_eq!(
                json.to_string(),
                sibling.replace("\"*=\"", &format!("\"{op}\"")),
                "`{op}` must build the `*=` tree: {source:?}"
            );
        }
    }
}

/// The CONTRAST that bounds the rule: a bare `+` / `-` and a prefix `++` / `--` DO start an
/// expression, so ahead of them the `<…>` stays a comparison chain on both oracles
/// (`x = f<T> + c` is `x = (f < T) > +c`), and a line break before the `+=` changes
/// nothing, the instantiation already holding there.
#[test]
fn a_unary_follower_still_reads_as_a_comparison_chain() {
    for (source, operand) in [
        ("x = f<T> + c;", "UnaryExpression"),
        ("x = f<T> - c;", "UnaryExpression"),
        ("x = f<T> ++c;", "UpdateExpression"),
        ("x = f<T> --c;", "UpdateExpression"),
    ] {
        let json = parse_json(source);
        let right = "/body/0/expression/right";
        assert_eq!(
            json.pointer(&format!("{right}/operator"))
                .and_then(Value::as_str),
            Some(">"),
            "the chain's outer operator: {source:?} {json}"
        );
        assert_eq!(
            json.pointer(&format!("{right}/left/operator"))
                .and_then(Value::as_str),
            Some("<"),
            "the chain's inner operator: {source:?} {json}"
        );
        assert_eq!(
            json.pointer(&format!("{right}/right/type"))
                .and_then(Value::as_str),
            Some(operand),
            "the follower is the chain's operand: {source:?} {json}"
        );
    }
    let broken = parse_json("f<T>\n-= c;");
    assert_eq!(
        broken
            .pointer("/body/0/expression/left/type")
            .and_then(Value::as_str),
        Some("TSInstantiationExpression"),
        "across a line break the instantiation already holds: {broken}"
    );
}

/// The far edge of the instantiation-target deferral: a `>`-LED assignment operator.
/// tsc refuses a type argument list ahead of any token its scanner starts with `>`
/// (`canFollowTypeArgumentsInExpression` — the close and the `>` would be ambiguous with
/// a re-scanned `>>`), and `>>=` / `>>>=` are two such tokens, so `f<T> >>= c` has no
/// instantiation to assign to: the `<…>` falls back to a comparison with no operand
/// (TS1109) — a GRAMMAR refusal. A close GLUED to the operator is no close at all: the
/// longest punctuator at that position swallows it (`f<T>>= c` is `f < T >>= c`,
/// `f<A<B>>=c` is `f < A < B >>= c`), a comparison assigned to, which no production
/// derives. acorn rejects every row here, but for the SPACED rows its reason is its own
/// reading — an instantiation target, refused by the assignment-target early error — not
/// its grammar; inside a paren or argument list, where it skips that check, it accepts
/// them (`a_greater_than_led_assignment_rejects_where_acorn_skips_the_target_check`). The
/// fixture path carries the headline spellings as the `input_invalid_*` files of
/// `typescript_specific/generics/instantiation_operator_follow`; these are the heads,
/// depths, trivia, positions and line breaks beside them.
#[test]
fn instantiation_target_refuses_a_greater_than_led_assignment() {
    for source in [
        // spaced: tsc's follower refusal
        "f<T> >>= c;",
        "f<T> >>>= c;",
        "f<A<B>> >>= c;",
        "a.b<T> >>= c;",
        "f()<T> >>= c;",
        "a?.b<T> >>= c;",
        "new f<T> >>= c;",
        "x = f<T> >>= c;",
        "() => f<T> >>= c;",
        "f<T> /* x */ >>= c;",
        "f<T>/**/>>= c;",
        "f<T>\n>>= c;",
        "a < b > >>= c;",
        // glued: the longest punctuator swallows the close
        "f<T>>= c;",
        "f<T>>>= c;",
        "f<T>>>=c;",
        "f<A<B>>=c;",
        "f<A<B>>>=c;",
        "f<A<B<C>>>=c;",
        "f<A<B<C>>>>=c;",
        "f<A<B<C>>>>>=c;",
        "a.b<T>>= c;",
        "f()<T>>>=c;",
        "a?.b<T>>= c;",
        "new f<T>>= c;",
        "x = a + f<T>>= c;",
        "class C { p = f<T>>= c; }",
        // a spaced `<`: the first `>` of the operator is no close either
        "a < b >>= c;",
        "a < b >>>= c;",
        "a<b>>=c;",
        "[a < b >>= c];",
        "`${a < b >>= c}`;",
    ] {
        assert!(rejects(source), "must reject: {source:?}");
    }
}

/// The acorn-kept side of the same seam: a SPACED `>=` — one `>` and an `=` — is the
/// follower acorn-typescript reads as `(f<T>) >= c` where tsc refuses the list, and
/// rejecting it would refuse a component Svelte compiles, so tsv accepts it and the
/// formatter repairs it to the pair every parser reads alike. A close glued
/// to `=` alone is `>=` itself, a comparison every parser reads alike (`f<T>= c` is
/// `f < T >= c`), and a `>`-led run with no `=` stays the shift it always was
/// (`f<T>>c` is `f < (T >> c)`). A `<<=` is no `>`-led token, so the instantiation
/// target it assigns to is the deferred class again; and a call's `)` ends the target
/// on no `>` at all, so `f<T>(x) >>= c` is the deferred call target (`foo() >>= c`).
#[test]
fn a_spaced_greater_equal_follower_keeps_the_acorn_reading() {
    for (source, printed) in [
        ("f<T> >= c;", "(f<T>) >= c;\n"),
        ("f<T>\n>= c;", "(f<T>) >= c;\n"),
        ("f<T> /* x */ >= c;", "(f<T>) /* x */ >= c;\n"),
        ("f<A<B>> >= c;", "(f<A<B>>) >= c;\n"),
        ("f<A<B<C>>> >= c;", "(f<A<B<C>>>) >= c;\n"),
        ("a.b<T> >= c;", "(a.b<T>) >= c;\n"),
        ("a?.b<T> >= c;", "(a?.b<T>) >= c;\n"),
        ("new f<T> >= c;", "new f<T>() >= c;\n"),
        ("a < b > >= c;", "(a<b>) >= c;\n"),
        ("x = a + f<T> >= c;", "x = (a + f<T>) >= c;\n"),
        ("f<T>= c;", "f < T >= c;\n"),
        ("a < b >= c;", "a < b >= c;\n"),
        ("f<T>>c;", "f < T >> c;\n"),
        ("f<T>>>c;", "f < T >>> c;\n"),
        ("f<A<B>>>c;", "f < A < B >>> c;\n"),
        ("f<T> <<= c;", "f<T> <<= c;\n"),
        ("f<T>(x) >>= c;", "f<T>(x) >>= c;\n"),
        ("f<T>(x) >>>= c;", "f<T>(x) >>>= c;\n"),
        // A `<` that never closes: the `>>=` is an ordinary operator on the last operand.
        ("f<T, U>>=c;", "(f < T, (U >>= c));\n"),
        ("f<T, U>>>=c;", "(f < T, (U >>>= c));\n"),
        ("g(c < d, x >>= 1);", "g(c < d, (x >>= 1));\n"),
    ] {
        assert!(!rejects(source), "must parse: {source:?}");
        assert_eq!(format(source), printed, "{source:?}");
    }
}

/// A parenthesized instantiation stays a target of `>>=` / `>>>=` (tsc's parser accepts
/// it; acorn's `Assigning to rvalue` is the deferred early error) and keeps its pair,
/// since stripping it leaves the bare spelling above that tsc's grammar refuses. The
/// fixture `typescript/expressions/assignment/instantiation_target_paren_svelte_prettier_divergence`
/// pins the script-block spellings against prettier; this adds the glued and the
/// optional-chain heads, and a Svelte template, where the pair must survive inside the
/// assignment's own.
#[test]
fn a_parenthesized_instantiation_keeps_its_pair_before_a_shift_assignment() {
    for source in [
        "(f<T>) >>= c;\n",
        "(f<T>) >>>= c;\n",
        "(a?.b<T>) >>= c;\n",
        "(f()<T>) >>= c;\n",
        "(f<T>) >>= c >>= d;\n",
        "(f<T>) /= c;\n",
    ] {
        assert_eq!(format(source), source, "the pair must stay: {source:?}");
    }
    assert_eq!(format("(f<T>)>>=c;"), "(f<T>) >>= c;\n");
    // tsc takes the list past a line break; joined, the pair must appear
    assert_eq!(format("f<T>\n/= c;"), "(f<T>) /= c;\n");
    assert_eq!(format("f<T>\n/= c/g;"), "(f<T>) /= c / g;\n");
    assert_eq!(format("(f<T>) /= c/g;"), "(f<T>) /= c / g;\n");

    let template = "<script lang=\"ts\"></script>\n\n{((f<T>) >>= c)}\n";
    assert_eq!(
        tsv_svelte::format_str("<script lang=\"ts\"></script>\n\n{(f<T>) >>= c}\n")
            .expect("a parenthesized target parses in a template"),
        template,
        "keeps the target pair inside the assignment's own"
    );
    assert_eq!(
        tsv_svelte::format_str(template).expect("the output parses"),
        template,
        "idempotent"
    );
}

/// Inside a paren or a call argument list acorn-typescript does not know yet whether the list
/// is an arrow's parameters (`maybeInArrowParameters`), so it skips the target check an
/// assignment operator runs, and never runs it once the list turns out not to be one —
/// the window reaches everything nested in the list (an object value, an array element, a
/// template hole, a spread, an arrow-parameter default) until a function body resets it.
/// There acorn accepts both classes above as it reads them: a `>`-led operator assigning
/// to an instantiation (`g(f<T> >>= c)`), and any compound operator assigning to a
/// comparison or other binary (`g(a < b >>= c)`, `g(f<T>>= c)` — the glued close is no
/// close — and `g(a + b >>= c)`, which tsv already rejects). tsc refuses every row
/// (TS1109 / TS1005): the first is its follower refusal, the second its
/// `LeftHandSideExpression` production, and neither is the deferred early error. The
/// fixture path pins one of each against Svelte's own parse (`tsv_rejects.txt`):
/// `typescript/expressions/assignment/operator_target_compound_paren_list/*` and
/// `typescript/typescript_specific/generics/instantiation_shift_assign_call_arg_svelte_divergence`.
#[test]
fn a_greater_than_led_assignment_rejects_where_acorn_skips_the_target_check() {
    for source in [
        "g(f<T> >>= c);",
        "(f<T> >>= c);",
        "({ k: f<T> >>= c });",
        "g(new f<T> >>= c);",
        "g(f<T>>>= c);",
        "g(a < b >>= c);",
        "g(f<T>>= c);",
        "g(f<A<B>>=c);",
        "(a < b >>= c);",
        "x = (f<A<B>>=c);",
        "({ k: a < b >>= c });",
        "(x = a < b >>= c) => 0;",
        "g(a + b >>= c);",
    ] {
        assert!(rejects(source), "must reject: {source:?}");
    }
    assert!(
        tsv_svelte::format_str("<script lang=\"ts\"></script>\n\n{g(f<T> >>= c)}\n").is_err(),
        "a template expression reads the same window"
    );
    let unclosed = "<script lang=\"ts\"></script>\n\n{g(c < d, (x >>= 1))}\n";
    assert_eq!(
        tsv_svelte::format_str("<script lang=\"ts\"></script>\n\n{g(c < d, x >>= 1)}\n")
            .expect("a `<` that never closes leaves an ordinary `>>=`"),
        unclosed
    );
}

/// A close GLUED to an `=` is no close either: tsc's re-scan joins it to the `=` as `>=`
/// (`reScanGreaterToken`), so `f<T>==c` is `f < T >= = c` and `f<() => U>=c` has nothing
/// to read `() => U` as — every row is a syntax error to tsc and to acorn-typescript,
/// inside a call argument list too. The spaced spellings are the instantiation compared
/// (`f<T> == c`), which both take.
#[test]
fn a_close_glued_to_an_equals_is_no_close() {
    for source in [
        "f<T>==c;",
        "f<T>===c;",
        "f<A<B>>==c;",
        "a < b >== c;",
        "x = a.b<A<B>>==c;",
        "g(f<T>==c);",
        // a function-type head commits its list before the follower is read
        "f<() => U>=c;",
        "f<(a: A<B>) => C<D>>=c;",
        "g(f<() => U>=c);",
        "f<() => U>==c;",
        "f<A<B>>===c;",
        // a `new` head too: none of these is `new f<…>()` compared or assigned to
        "new f<() => U>=c;",
    ] {
        assert!(rejects(source), "must reject: {source:?}");
    }
    assert_eq!(format("f<T> == c;"), "f<T> == c;\n");
}

/// A same-line `/=` refuses the list as a `>`-led operator does, one rule over: tsc takes a
/// list only before a token that cannot START an expression, and `/=` can — as the head
/// of a regular expression literal. So `f<T> /= c` is `f < T >` followed by an
/// unterminated literal (TS1161), and `x = f<T> /= c/g` is the comparison with the regular
/// expression `/= c/g` as its right operand, which tsc accepts and tsv must read the same
/// way rather than as the instantiation assigned to acorn builds. Past a line break tsc
/// takes the list (`f<T>⏎/= c`).
#[test]
fn a_same_line_divide_assign_refuses_the_list() {
    for source in [
        "f<T> /= c;",
        "f<T>/= c;",
        "x = f<T> /= c;",
        "g(f<T> /= c);",
        "a.b<T> /= c;",
        "new f<T> /= c;",
        // a comment with no line terminator in it is no line break (TS1161)
        "f<T> /**/ /= c;",
    ] {
        assert!(rejects(source), "must reject: {source:?}");
    }
    let json = parse_json("x = f<T> /= c/g;");
    let right = "/body/0/expression/right";
    assert_eq!(
        json.pointer(&format!("{right}/operator"))
            .and_then(Value::as_str),
        Some(">"),
        "the comparison chain: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{right}/right/regex/pattern"))
            .and_then(Value::as_str),
        Some("= c"),
        "the regular expression operand: {json}"
    );
    // Inside a call argument list acorn-typescript reads the instantiation assigned to
    // (its target check is skipped there); tsc — and tsv — read the same comparison.
    let json = parse_json("g(f<T> /= c/g);");
    let arg = "/body/0/expression/arguments/0";
    assert_eq!(
        json.pointer(&format!("{arg}/operator"))
            .and_then(Value::as_str),
        Some(">"),
        "the comparison chain in the argument: {json}"
    );
    assert_eq!(
        json.pointer(&format!("{arg}/right/regex/pattern"))
            .and_then(Value::as_str),
        Some("= c"),
        "the regular expression operand in the argument: {json}"
    );
    assert_eq!(format("g(f<T> /= c/g);"), "g((f < T) > /= c/g);\n");
    // Across a line break — a comment holding one counts, as it does for tsc — the list
    // holds, and whatever the printer does with the break it keeps that reading.
    for source in ["f<T>\n/= c;", "f<T> /*\n*/ /= c;", "f<T> //x\n/= c;"] {
        for (pass, text) in [("source", source.to_string()), ("printed", format(source))] {
            let json = parse_json(&text);
            assert_eq!(
                json.pointer("/body/0/expression/left/type")
                    .and_then(Value::as_str),
                Some("TSInstantiationExpression"),
                "{pass} of {source:?} keeps the instantiation target: {text:?} {json}"
            );
        }
    }
}
