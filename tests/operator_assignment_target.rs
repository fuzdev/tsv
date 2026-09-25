// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! An assignment's left side must be a `LeftHandSideExpression`
//! (ecma262 §13.15: `AssignmentExpression : LeftHandSideExpression = AssignmentExpression`,
//! and the same left for every `AssignmentOperator` and the three logical assignments).
//! A bare operator expression is not one — a unary (`-a`, `typeof a`, `!a`), an update
//! (`++a`, `a++`), an `await` (`await a`), a binary or logical expression (`a + b`,
//! `a ?? b`) — so `-a = 1` is a GRAMMAR error: no derivation reaches it. That is a
//! different class from the deferred `AssignmentTargetType` early error of
//! `tests/nonsimple_assignment_target.rs` (`foo() = bar`), whose left IS a
//! `LeftHandSideExpression` and only fails the refinement layered on top. tsc's parser
//! rejects every bare spelling here (TS1005 `';' expected.`), and so do acorn
//! (`Assigning to rvalue`) and prettier.
//!
//! **Parenthesized, the same operand is the deferred class again.** `(-a) = 1` is a
//! `ParenthesizedExpression`, a `PrimaryExpression`, so the production derives it and only
//! the early error refuses it: tsc's parser accepts it, and tsv defers it as it does
//! `foo() = bar`. The pair is then load-bearing — the bare reprint is the grammar error
//! above, or for a conditional, an arrow or a `yield` — alternatives of
//! `AssignmentExpression` itself — a DIFFERENT program (`a ? b : c = 1` is
//! `a ? b : (c = 1)`) — so the formatter keeps it.
//!
//! The fixtures pin the family: `typescript/expressions/assignment/operator_target` (the
//! bare spellings, as `input_invalid_*` files, beside the valid controls),
//! `operator_target_paren_svelte_prettier_divergence` and
//! `assignment_tier_target_paren_svelte_prettier_divergence` (the kept pairs), and
//! `cast_target`'s `input_invalid_*` files (a bare assertion over a non-simple operand).
//! What stays here is what a fixture cannot hold: every assignment operator over one
//! operand, the line-break spellings a formatted input never carries, the nested
//! positions, the `Script`-goal reading of `await`, and the format fallback.

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

/// Every assignment operator ecma262 names — `=`, the arithmetic / bitwise / shift
/// compounds, and the three logical assignments.
const ASSIGNMENT_OPERATORS: [&str; 16] = [
    "=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=",
    "??=",
];

/// A bare operator expression is no assignment target under any assignment operator.
#[test]
fn bare_operator_target_rejects_under_every_operator() {
    for left in [
        "-a",
        "+a",
        "!a",
        "~a",
        "typeof a",
        "void a",
        "delete a.b",
        "++a",
        "--a",
        "a++",
        "a--",
        "await a",
        "a + b",
        "a * b",
        "a || b",
        "a ?? b",
        "a in b",
        "a instanceof b",
    ] {
        for op in ASSIGNMENT_OPERATORS {
            let source = format!("{left} {op} 1;");
            assert!(
                rejects(&source),
                "a bare operator target rejects: {source:?}"
            );
        }
    }
}

/// The original filing: `await using [a] = x` reads as `await (using[a]) = x` — an
/// `await` over a member access, not an `await using` declaration (that one takes a
/// binding identifier, never a pattern), so the `=` has an `AwaitExpression` on its left.
/// The spelling without the `=` is the valid member-access statement, and the plain
/// `using [a] = x` assigns to the member `using[a]`.
#[test]
fn await_using_array_target_rejects() {
    for source in [
        "await using [a] = x;",
        "await using[a] = x;",
        "await (using[a]) = x;",
        "async function f() { await using [a] = x; }",
    ] {
        assert!(rejects(source), "an `await` target rejects: {source:?}");
    }
    for canonical in ["await using[a];\n", "using[a] = x;\n", "await using[0];\n"] {
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// A line break between the operand and the operator changes nothing: no ASI applies
/// ahead of `=` (the next token continues the expression), and a postfix `++` still binds
/// to the line before it only when no break precedes it — `a⏎++⏎b = 1` is `a; ++b = 1`,
/// whose `++b` is the prefix-update target.
#[test]
fn line_broken_operator_target_rejects() {
    for source in [
        "-a\n= 1;",
        "a++\n= 1;",
        "a\n++\nb = 1;",
        "a +\nb = 1;",
        "typeof\na = 1;",
    ] {
        assert!(
            rejects(source),
            "a line-broken operator target rejects: {source:?}"
        );
    }
}

/// The same left is refused wherever an assignment appears: the right of another
/// assignment, a conditional branch, an argument, an element, a property value, a
/// template hole, an arrow's concise body, and under a wrapper the operand's own
/// operator binds tighter than (`-a! = 1` is `-(a!) = 1`, `-(a) = 1` is a unary over a
/// parenthesized primary).
#[test]
fn nested_operator_target_rejects() {
    for source in [
        "x = -a = 1;",
        "x = a++ = 1;",
        "a ? -b = 1 : c;",
        "a ? b : -c = 1;",
        "f(-a = 1);",
        "x = [-a = 1];",
        "({ x: -a = 1 });",
        "`${-a = 1}`;",
        "() => -a = 1;",
        "async () => await a = 1;",
        "-a! = 1;",
        "-(a) = 1;",
        "typeof await a = 1;",
    ] {
        assert!(
            rejects(source),
            "a nested operator target rejects: {source:?}"
        );
    }
}

/// A destructuring DEFAULT is an assignment before it is a pattern: the element
/// `-a = 1` in `[-a = 1] = x` parses as an `AssignmentExpression` inside an array literal,
/// ahead of any cover reparse, so its bare operator target is the same grammar error as at
/// statement level (tsc TS1005 `',' expected.`, acorn `Assigning to rvalue`) — in an array
/// or object pattern, a for-of head, and an arrow parameter list alike.
#[test]
fn pattern_default_operator_target_rejects() {
    for source in [
        "[-a = 1] = x;",
        "[a++ = 1] = x;",
        "[a + b = 1] = x;",
        "({ a: -b = 1 } = x);",
        "({ a: await b = 1 } = x);",
        "for ([-a = 1] of xs) {}",
        "([-a = 1]) => 1;",
    ] {
        assert!(
            rejects(source),
            "a bare operator default target rejects: {source:?}"
        );
    }
}

/// A bare type assertion is an assignment target only over a SIMPLE operand — acorn's
/// reading, which tsv follows though tsc's parser refuses the bare assertion outright (the
/// formatter repairs it to `(a as T) = 1`, a spelling all three parsers read alike). Over
/// a non-simple operand both oracles reject, and so does tsv; parenthesized, the
/// assertion is the deferred class again (`([a, b] as T) = c`, pinned in
/// `tests/nonsimple_assignment_target.rs`).
#[test]
fn bare_assertion_over_non_simple_operand_rejects() {
    for source in [
        "-a as T = 1;",
        "a++ as T = 1;",
        "a + b as T = 1;",
        "(-a) as T = 1;",
        "fn() as T = 1;",
        "[a, b] as T = c;",
        "-a satisfies T = 1;",
        "<T>-a = 1;",
        "<T>a++ = 1;",
        "<T>(-a) = 1;",
        "<T>fn() = 1;",
    ] {
        assert!(
            rejects(source),
            "a bare assertion over a non-simple operand rejects: {source:?}"
        );
    }
    // The simple-operand spellings keep parsing, repaired to the parenthesized form.
    for (source, canonical) in [
        ("a as T = 1;", "(a as T) = 1;\n"),
        ("a satisfies T = 1;", "(a satisfies T) = 1;\n"),
        ("<T>a = 1;", "(<T>a) = 1;\n"),
        ("a.b as T = 1;", "(a.b as T) = 1;\n"),
        ("(a) as T = 1;", "(a as T) = 1;\n"),
        ("a as T as U = 1;", "(a as T as U) = 1;\n"),
        ("a! as T = 1;", "(a! as T) = 1;\n"),
        ("a as T += 1;", "(a as T) += 1;\n"),
    ] {
        assert_eq!(format(source), canonical, "repaired: {source:?}");
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// The parenthesized operand is the deferred class: it parses, keeps its operand node as
/// the target (the parens are transparent grouping on the wire, as for `(foo()) = bar`),
/// and prints WITH its pair. Every left that is not a `LeftHandSideExpression` bare takes
/// it — the operator expressions, whose bare reprint does not parse, the assignment-tier
/// alternatives (conditional, arrow, `yield`), whose bare reprint parses as a different
/// program, and a function expression, whose body tsc's parser does not continue past
/// into an `=` (TS2809).
#[test]
fn parenthesized_target_parses_and_keeps_its_parens() {
    for (source, left) in [
        ("(-a) = 1;", "UnaryExpression"),
        ("(void a) = 1;", "UnaryExpression"),
        ("(a++) += 1;", "UpdateExpression"),
        ("(a + b) = 1;", "BinaryExpression"),
        ("(a < b) = 1;", "BinaryExpression"),
        ("(a in b) = 1;", "BinaryExpression"),
        ("(a ?? b) ??= 1;", "LogicalExpression"),
        ("(a ? b : c) = 1;", "ConditionalExpression"),
        ("(() => b) = 1;", "ArrowFunctionExpression"),
        ("(async () => b) = 1;", "ArrowFunctionExpression"),
    ] {
        assert_eq!(
            parse_json(source)
                .pointer("/body/0/expression/left/type")
                .and_then(Value::as_str),
            Some(left),
            "the operand is the target: {source:?}"
        );
        assert_eq!(
            format(source),
            format!("{source}\n"),
            "keeps its pair: {source:?}"
        );
    }
    for (source, canonical) in [
        ("((-a)) = 1;", "(-a) = 1;\n"),
        ("x = (-a) = 1;", "x = (-a) = 1;\n"),
        ("x = (a ? b : c) = 1;", "x = (a ? b : c) = 1;\n"),
        ("(-a)! = 1;", "(-a)! = 1;\n"),
        (
            "async function f() {\n\t(await a) = 1;\n}",
            "async function f() {\n\t(await a) = 1;\n}\n",
        ),
        (
            "function* g() {\n\t(yield) = 1;\n}",
            "function* g() {\n\t(yield) = 1;\n}\n",
        ),
        (
            "function* g() {\n\t(yield a) = 1;\n}",
            "function* g() {\n\t(yield a) = 1;\n}\n",
        ),
        // as a destructuring default's target
        ("[(-a) = 1] = x;", "[(-a) = 1] = x;\n"),
        ("({ a: (-b) = 1 } = x);", "({ a: (-b) = 1 } = x);\n"),
        ("[(a ? b : c) = 1] = x;", "[(a ? b : c) = 1] = x;\n"),
        // where the assignment itself takes a pair, and where it takes none
        ("f((-a) = 1);", "f(((-a) = 1));\n"),
        ("`${(-a) = 1}`;", "`${((-a) = 1)}`;\n"),
        ("() => (-a) = 1;", "() => ((-a) = 1);\n"),
        ("for ((-a) = 1;;) {}", "for ((-a) = 1; ;) {}\n"),
        // a function expression, off statement start
        ("x = (function(){}) = 1;", "x = (function () {}) = 1;\n"),
        (
            "x = (async function(){}) = 1;",
            "x = (async function () {}) = 1;\n",
        ),
        ("f((function(){}) = 1);", "f(((function () {}) = 1));\n"),
        ("(function(){}) = 1;", "(function () {}) = 1;\n"),
        // a class expression needs no pair: tsc reads `x = class {} = 1` as the same program
        ("x = (class {}) = 1;", "x = class {} = 1;\n"),
    ] {
        assert_eq!(format(source), canonical, "{source:?}");
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// A bare function-expression target is a `PrimaryExpression`, so both bare spellings
/// parse — tsc's parser refuses the `=` one (TS2809, it does not read past the body) and
/// reads the compound one — and tsv repairs both to the kept pair, the one form every
/// operator reads alike.
#[test]
fn bare_function_target_is_repaired_to_the_kept_pair() {
    for (source, canonical) in [
        ("x = function () {} = 1;", "x = (function () {}) = 1;\n"),
        ("x = function () {} += 1;", "x = (function () {}) += 1;\n"),
    ] {
        assert_eq!(format(source), canonical, "repaired: {source:?}");
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// The statement-start walk stops at a non-LHS target that brings its own pair
/// (`(function () {}) = 1` wraps once), but a CAST target's pair is prettier's, and
/// prettier still wraps the leftmost node inside it — so the walk descends through the
/// three cast kinds, and these print as prettier prints them.
#[test]
fn statement_start_cast_target_wraps_its_leftmost_node() {
    for canonical in [
        "(({}).x as T) = 1;\n",
        "(({ a }) as T) = c;\n",
        "(({}) as T) = 1;\n",
        "((function () {}).x as T) = 1;\n",
        "((class {}).x as T) = 1;\n",
        "((let)[0] as T) = 1;\n",
        "((function () {}) as T) = 1;\n",
        "(function () {}) = 1;\n",
    ] {
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
    for (source, canonical) in [
        ("({}.x as T) = 1;", "(({}).x as T) = 1;\n"),
        ("({a} as T) = c;", "(({ a }) as T) = c;\n"),
        ("(let[0] as T) = 1;", "((let)[0] as T) = 1;\n"),
    ] {
        assert_eq!(format(source), canonical, "{source:?}");
    }
}

/// "Parenthesized" is a fact about the SOURCE, not about where the operand's span starts
/// relative to the first token: a comment inside the pair, on either side of the operand,
/// or a line break inside it, leaves the pair in place; a comment AHEAD of a bare operand
/// is no pair at all; and a JSDoc cast's pair is a pair (the cast is the deferred class,
/// its operand the parenthesized unary).
#[test]
fn parenthesized_target_is_read_from_the_source() {
    for source in [
        "( /* c */ -a) = 1;",
        "(-a /* c */) = 1;",
        "(\n-a\n) = 1;",
        "/** @type {T} */ (-a) = 1;",
    ] {
        assert!(
            !rejects(source),
            "a parenthesized target parses: {source:?}"
        );
        let formatted = format(source);
        assert!(
            !rejects(&formatted),
            "its output re-parses: {source:?} -> {formatted:?}"
        );
        assert_eq!(format(&formatted), formatted, "idempotent: {formatted:?}");
        assert!(
            formatted.contains('(') && !formatted.starts_with('-'),
            "keeps a pair around the operand: {formatted:?}"
        );
    }
    for source in ["/* c */ -a = 1;", "/** @type {T} */ -a = 1;"] {
        assert!(rejects(source), "no pair, no target: {source:?}");
    }
}

/// A Svelte template expression parses through the same assignment arm, so the rule and
/// the kept pair reach it unchanged.
#[test]
fn svelte_template_operator_target() {
    assert!(
        tsv_svelte::format_str("{-a = 1}").is_err(),
        "a bare operator target rejects in a template"
    );
    let canonical = "{((-a) = 1)}\n";
    assert_eq!(
        tsv_svelte::format_str("{(-a) = 1}").expect("a parenthesized target parses"),
        canonical,
        "keeps the target pair inside the assignment's own"
    );
    assert_eq!(
        tsv_svelte::format_str(canonical).expect("the output parses"),
        canonical,
        "idempotent"
    );
}

/// A destructuring element's TARGET is the deferred class, not this one: ecma262 states
/// "LeftHandSideExpression is not covering an AssignmentPattern" as an early error, and
/// tsc's parser accepts every element spelling (its checker rejects). So the whole-target
/// rule must not reach into a pattern's elements. An element DEFAULT's target is another
/// matter — the element is an assignment before any reparse, so a bare operator there
/// rejects (`pattern_default_operator_target_rejects`).
#[test]
fn destructuring_element_operator_target_stays_deferred() {
    for canonical in [
        "[-a] = x;\n",
        "[a++] = x;\n",
        "[...-a] = x;\n",
        "({ a: -b } = x);\n",
        "({ ...-a } = x);\n",
    ] {
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// The controls the rule must not reach: a non-null assertion is a member-tier postfix
/// in TypeScript's grammar (`a! = 1` is a simple target), a `new` without arguments is a
/// `NewExpression` (a `LeftHandSideExpression`, so the deferred class), a `yield`'s
/// argument absorbs the assignment, and an operator over a parenthesized assignment is an
/// ordinary operand.
#[test]
fn left_hand_side_and_absorbed_controls_parse() {
    for (source, canonical) in [
        ("a! = 1;", "a! = 1;\n"),
        ("new a = 1;", "new a() = 1;\n"),
        (
            "function* g() { yield a = 1; }",
            "function* g() {\n\tyield (a = 1);\n}\n",
        ),
        ("-(a = 1);", "-(a = 1);\n"),
        ("await (a = 1);", "await (a = 1);\n"),
        ("(a = 1) + b;", "(a = 1) + b;\n"),
    ] {
        assert_eq!(format(source), canonical, "{source:?}");
    }
}

/// At the `Script` goal, `await` outside an async function is an identifier, so the
/// module grammar's `AwaitExpression` target does not exist there: `await = 1` assigns to
/// the name, `await(a) = 1` is a call target (the deferred class), and `await⏎a = 1` is two
/// statements by ASI. The format fallback reaches exactly those readings once the module
/// parse refuses the `await` target.
#[test]
fn script_goal_await_is_a_name() {
    for source in ["await = 1;", "await(a) = 1;", "await\na = 1;"] {
        let arena = bumpalo::Bump::new();
        assert!(
            tsv_ts::parse_with_goal(source, tsv_ts::Goal::Script, &arena).is_ok(),
            "parses at Script: {source:?}"
        );
    }
    assert!(
        rejects("await (a) = 1;"),
        "at Module the `await` is an operator"
    );
    assert!(
        rejects("await\na = 1;"),
        "at Module no ASI splits `await⏎a`"
    );
    assert_eq!(
        tsv_ts::format_str("await\na = 1;").expect("the fallback parses"),
        "await;\na = 1;\n",
        "the fallback's Script reading"
    );
    assert_eq!(
        tsv_ts::format_str("await (a) = 1;").expect("the fallback parses"),
        "await(a) = 1;\n",
        "the fallback's Script reading"
    );
    // A top-level `import` is a goal gate: the Script retry dies on it, so the fallback
    // reports the Module attempt's error — the operator target — not the Script's.
    let source = "import x from 'y';\nawait (a) = 1;";
    let arena = bumpalo::Bump::new();
    let module_error = tsv_ts::parse_with_goal(source, tsv_ts::Goal::Module, &arena)
        .expect_err("the module attempt rejects the `await` target");
    let fallback_error = tsv_ts::format_str(source).expect_err("the fallback rejects");
    assert_eq!(
        fallback_error.to_string(),
        module_error.to_string(),
        "the goal gate keeps the Module error"
    );
}
