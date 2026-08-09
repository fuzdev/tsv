// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Parens that carry a *statement's reading* are not redundant — the printer must keep
//! them.
//!
//! Two ECMAScript lookahead restrictions make a leading token change what the whole
//! statement IS, so a paren the author wrote around that token is load-bearing:
//!
//! - `ExpressionStatement : [lookahead ∉ { `{`, `function`, `async function`, `class`,
//!   `let [` }] Expression ;` — so `let[a] = 1;` is a **`VariableDeclaration`** (an
//!   array binding pattern), while `(let)[a] = 1;` is an assignment to the member
//!   `let[a]`. Same for a `for` init and a for-in/of left.
//! - `ForInOfStatement`'s `[lookahead ∉ { `let` }]` on the `of` form — so
//!   `for (let of foo);` is a syntax error and only `for ((let) of foo);` says what the
//!   author meant.
//!
//! tsv's own reader is the strictest witness available: dropping these parens produces
//! text tsv either **rejects** or reparses as a **different node**. The second half is
//! why this file asserts node types and not just round-trips — the re-meaning cases are
//! valid, idempotent and comment-clean, so every reparse-shaped audit is blind to them.
//!
//! A third, smaller case is the same principle without a lookahead rule: a
//! strict-mode-reserved word heading an `as` / `satisfies` expression statement
//! (`(interface) as never;`). Bare, tsv's parser commits to a declaration reading and
//! errors, so the parens must survive there too.
//!
//! This can't be a fixture: acorn-typescript enforces the strict-mode early error that
//! bars `let` / `interface` as names and **rejects every input below**, so no
//! `expected.json` oracle exists (same reason as
//! [`strict_reserved_word_as_name`](./strict_reserved_word_as_name.rs)). The expected
//! strings are prettier's own output — these shapes are prettier's
//! `tests/format/js/identifier/{parentheses,for-of}/` suite, which
//! `corpus:compare:format` grades from the other side.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::convert_ast_json(&program, source)
}

fn accepts(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// The `type` of the first top-level statement — the axis a dropped paren flips.
fn first_statement_type(source: &str) -> String {
    parse_json(source)
        .pointer("/body/0/type")
        .and_then(Value::as_str)
        .map_or_else(|| "<none>".to_owned(), str::to_owned)
}

fn check(cases: &[(&str, &str)]) {
    let mut failures = Vec::new();
    for (source, expected) in cases {
        let got = format(source);
        if got.trim_end() != *expected {
            failures.push(format!(
                "{source}\n    expected: {expected:?}\n    got:      {got:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} shapes printed wrong:\n  {}",
        failures.len(),
        cases.len(),
        failures.join("\n  ")
    );
}

/// `let` as the object of a **computed** member that leads an expression statement.
/// Prettier's `shouldAddParenthesesToIdentifier` clause 3, and the reason it exists:
/// without the parens the statement is a `let [a] = 1` declaration.
#[test]
fn computed_member_object_at_statement_start() {
    check(&[
        ("(let)[a] = 1;", "(let)[a] = 1;"),
        ("(let)[a].b.c.e = 1;", "(let)[a].b.c.e = 1;"),
        ("(let)[let[a]] = 1;", "(let)[let[a]] = 1;"),
        ("(let)[a] ??= 1;", "(let)[a] ??= 1;"),
        ("(let)[0] = 1;", "(let)[0] = 1;"),
        ("(let)['a'] = 1;", "(let)['a'] = 1;"),
        ("(let)[x].foo();", "(let)[x].foo();"),
        ("(let)[2];", "(let)[2];"),
        // The leftmost token is not `let`, so nothing is ambiguous and no paren is added.
        ("foo[let[a]] = 1;", "foo[let[a]] = 1;"),
        ("foo = let[a];", "foo = let[a];"),
        ("a = let[x].foo();", "a = let[x].foo();"),
        ("[let[a]] = 1;", "[let[a]] = 1;"),
        ("a[1] + (let[2] = 2);", "a[1] + (let[2] = 2);"),
        // A NON-computed member never needs the paren — only `let [` is restricted.
        ("let.a = 1;", "let.a = 1;"),
        ("let.a[0] = 1;", "let.a[0] = 1;"),
        ("let.let[x].foo();", "let.let[x].foo();"),
        // `let (` is likewise unrestricted, so a call callee stays bare.
        ("let()[a] = 1;", "let()[a] = 1;"),
        ("foo(let)[a] = 1;", "foo(let)[a] = 1;"),
    ]);
}

/// The same clause reached through an enclosing expression: what matters is the
/// **leftmost token of the statement**, so the paren goes on `let` however deep the
/// expression that starts with it.
#[test]
fn computed_member_object_leftmost_through_expressions() {
    check(&[
        ("((let)[0] = 1) || 2;", "((let)[0] = 1) || 2;"),
        ("((let)[0] = 1) ? a : b;", "((let)[0] = 1) ? a : b;"),
        (
            "((let)[0] = 1) instanceof a;",
            "((let)[0] = 1) instanceof a;",
        ),
        ("((let)[0] = 1)();", "((let)[0] = 1)();"),
        ("((let)[0] = 1)``;", "((let)[0] = 1)``;"),
        ("((let)[0] = 1).toString;", "((let)[0] = 1).toString;"),
        ("((let)[0] = 1)?.toString;", "((let)[0] = 1)?.toString;"),
        ("(((let)[0] = 1), 2);", "(((let)[0] = 1), 2);"),
        // A statement BODY is an expression statement of its own.
        ("while (true) (let)[0] = 1;", "while (true) (let)[0] = 1;"),
        // Not statement-initial: the enclosing construct already supplies a token.
        ("alert((let[0] = 1));", "alert((let[0] = 1));"),
        ("if ((let[0] = 1));", "if ((let[0] = 1));"),
        ("var a = (let[0] = 1);", "var a = (let[0] = 1);"),
        ("void (let[0] = 1);", "void (let[0] = 1);"),
        ("new (let[0] = 1)();", "new (let[0] = 1)();"),
        ("throw (let[0] = 1);", "throw (let[0] = 1);"),
        ("[...(let[0] = 1)];", "[...(let[0] = 1)];"),
    ]);
}

/// A `for` header is three more statement-start positions: the C-style init, the for-in
/// left, and the for-of left.
#[test]
fn for_header_positions() {
    check(&[
        ("for ((let)[0] = 1; ; );", "for ((let)[0] = 1; ;);"),
        ("for ((let)[0] in {});", "for ((let)[0] in {});"),
        ("for ((let)[0] of []);", "for ((let)[0] of []);"),
    ]);
}

/// The for-of / for-in LEFT carries a stronger rule than the computed-member one: the
/// restriction is on the head's leftmost token whatever its shape, so a plain `(let)`,
/// a member, and a call all keep the paren.
#[test]
fn for_of_and_for_in_left_leftmost_token() {
    check(&[
        ("for ((let) of foo);", "for ((let) of foo);"),
        ("for ((let).a of foo);", "for ((let).a of foo);"),
        ("for ((let)[a] of foo);", "for ((let)[a] of foo);"),
        ("for ((let)().a of foo);", "for ((let)().a of foo);"),
        ("for ((let).a in foo);", "for ((let).a in foo);"),
        ("for ((let)[a] in foo);", "for ((let)[a] in foo);"),
        // Prettier normalizes the paren onto the identifier rather than the member.
        ("for ((let.a) of foo);", "for ((let).a of foo);"),
        ("for ((let[a]) of foo);", "for ((let)[a] of foo);"),
        // The RIGHT side of `of` is an ordinary expression position.
        ("for (foo of let);", "for (foo of let);"),
        ("for (foo of let.a);", "for (foo of let.a);"),
        ("for (foo of let[a]);", "for (foo of let[a]);"),
        ("for (letFoo of foo);", "for (letFoo of foo);"),
        // A `let` DECLARATION binding a variable named `of` — no paren question at all.
        ("for (let of of let);", "for (let of of let);"),
    ]);
}

/// `for await` is the same left-hand position.
#[test]
fn for_await_of_left() {
    check(&[(
        "async function a() {\n\tfor await ((let) of foo);\n\tfor await ((let).a of foo);\n\tfor await ((let)[a] of foo);\n\tfor await ((let)()[a] of foo);\n}",
        "async function a() {\n\tfor await ((let) of foo);\n\tfor await ((let).a of foo);\n\tfor await ((let)[a] of foo);\n\tfor await ((let)()[a] of foo);\n}",
    )]);
}

/// A strict-mode-reserved word heading an `as` / `satisfies` expression statement: bare,
/// tsv's parser reads the word as a declaration starter and errors, so the paren is what
/// keeps the statement readable. `type` and `module` were already handled; `let` and
/// `interface` are the missing members of the same set.
#[test]
fn reserved_word_heading_a_cast_statement() {
    check(&[
        ("(type) as never;", "(type) as never;"),
        ("(module) as never;", "(module) as never;"),
        ("(interface) as never;", "(interface) as never;"),
        ("(let) as never;", "(let) as never;"),
        ("(let) satisfies never;", "(let) satisfies never;"),
    ]);
}

/// The invariant behind every case above, asserted directly: formatting must not change
/// what the statement IS. `(let[a] = 1);` is the sharpest — its bare form parses fine
/// and is idempotent, so only the node type exposes the loss.
#[test]
fn formatting_preserves_the_statement_kind() {
    let mut failures = Vec::new();
    for source in [
        "(let[a] = 1);",
        "(let)[a] = 1;",
        "(let)[0] = 1;",
        "(let)[2];",
        "(let)[x].foo();",
        "while (true) (let)[0] = 1;",
        "let[a] = 1;",
        "let.a = 1;",
    ] {
        let before = first_statement_type(source);
        let printed = format(source);
        if !accepts(&printed) {
            failures.push(format!(
                "{source}\n    -> output does not reparse: {printed:?}"
            ));
            continue;
        }
        let after = first_statement_type(&printed);
        if before != after {
            failures.push(format!(
                "{source}\n    -> {before} became {after} via {printed:?}"
            ));
        }
        let twice = format(&printed);
        if printed != twice {
            failures.push(format!(
                "{source}\n    -> not idempotent: {printed:?} != {twice:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} shapes lost their statement kind:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// The for-header counterpart of the invariant above: a for-of/for-in head must not
/// change what it binds, and must reparse.
#[test]
fn formatting_preserves_the_for_header() {
    let mut failures = Vec::new();
    for source in [
        "for ((let) of foo);",
        "for ((let).a of foo);",
        "for ((let)[a] of foo);",
        "for ((let)[0] in {});",
        "for ((let)[0] of []);",
        "for ((let)[0] = 1; ; );",
        "for (let of of let);",
    ] {
        let printed = format(source);
        if !accepts(&printed) {
            failures.push(format!(
                "{source}\n    -> output does not reparse: {printed:?}"
            ));
            continue;
        }
        let twice = format(&printed);
        if printed != twice {
            failures.push(format!(
                "{source}\n    -> not idempotent: {printed:?} != {twice:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} for-headers broke:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
