// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The paren pair around an UPDATE operator's operand, on the inputs no fixture can
//! carry: `(a * b)++` keeps it, `(f())++` strips it, the instantiation rule is
//! postfix-only (`(f<T>)++` keeps, `++(f<T>)` strips), and `++(-b)` keeps a pair it
//! does not need.
//!
//! An update operator binds on its operand exactly as a non-null `!` does, so every
//! operand looser than a member access needs the pair: bare, `a * b++` is
//! `a * (b++)` and `-b++` is `-(b++)` — a different tree. Where the operand ends on
//! an instantiation's closing `>` the bare spelling does not parse at all, since a
//! `++` starts an expression and so cannot follow a type argument list (tsc's
//! `canFollowTypeArgumentsInExpression`); a prefix `++f<T>` has nothing after the
//! list, so that spelling is fine.
//!
//! These cases cannot be fixtures, because acorn-typescript — the parser that
//! generates `expected.json` — rejects every input below, so there is no
//! `expected.json` to generate. Most are invalid update targets (`Assigning to
//! rvalue`, a check tsc's parser leaves to its checker), a couple are optional-chain
//! targets, and the bare postfix spellings die on the re-lex itself. tsv's parser
//! defers all of that as an early error and so has to print the tree. The
//! oracle-backed half of the same rule — the operands acorn DOES accept — lives in
//! the `typescript/expressions/unary/update_operand_paren` fixture, and the follower
//! class's fixturable half in
//! `typescript_specific/generics/instantiation_paren_follow_prettier_divergence`.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn parses(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

/// Each source is its own fixed point AND its output reparses.
fn assert_stable(sources: &[&str], what: &str) {
    for source in sources {
        let formatted = format(source);
        assert_eq!(&formatted, source, "{what}");
        assert!(
            parses(&formatted),
            "{what}: output must reparse: {formatted}"
        );
    }
}

#[test]
fn postfix_update_keeps_the_pair_below_a_member_access() {
    // The PRECEDENCE clause. The postfix operand's grammar is a
    // `LeftHandSideExpression`; every one of these sits below that, so bare the
    // operator would capture the rightmost operand.
    assert_stable(
        &[
            "(a * b)++;\n",
            "(a ?? b)++;\n",
            "(a = b)++;\n",
            "(a ? b : c)++;\n",
            "(-b)++;\n",
            "(void a)++;\n",
            "(await x)++;\n",
            "(x++)++;\n",
            "(++x)++;\n",
            "((a) => a)++;\n",
            "(a * b)--;\n",
        ],
        "postfix update below a member access",
    );
}

#[test]
fn prefix_update_keeps_the_pair_below_a_unary() {
    // The PRECEDENCE clause again. The prefix operand's grammar is a
    // `UnaryExpression`. These sit below it, so bare `++a * b` is `(++a) * b` — the
    // operator binds the left operand alone.
    assert_stable(
        &[
            "++(a * b);\n",
            "++(a ?? b);\n",
            "++(a = b);\n",
            "++(a ? b : c);\n",
            "++((a) => a);\n",
            // The control for the postfix-only instantiation half below: the pair
            // survives the prefix spelling because the BINARY is below a unary, not
            // because anything follows the type argument list.
            "++(a * f<T>);\n",
            "--(a * b);\n",
        ],
        "prefix update below a unary",
    );
}

#[test]
fn prefix_update_keeps_a_redundant_pair_at_the_unary_level() {
    // Not a correctness fix — the ACKNOWLEDGED COST of one uniform rule. Each of
    // these operands is already a `UnaryExpression`, so the bare spelling parses to
    // the same tree (`++-b` IS `++(-b)`) and the pair carries nothing. The prefix arm
    // keeps it anyway rather than carve a shape list out of the precedence clause, so
    // this group pins the output a future narrowing would silently change.
    assert_stable(
        &[
            "++(-b);\n",
            "++(x++);\n",
            "++(await x);\n",
            "++(void a);\n",
            "--(-b);\n",
        ],
        "prefix update at the unary level",
    );
}

#[test]
fn postfix_update_keeps_the_pair_on_a_bare_instantiation() {
    // The INSTANTIATION clause, which only a bare `TSInstantiationExpression` ever
    // reaches — every composite operand is already parenthesized by precedence. A
    // `++` starts an expression, so it cannot follow a type argument list and the
    // bare spelling does not parse at all.
    assert_stable(
        &[
            "(f<T>)++;\n",
            "(f<T>)--;\n",
            "(obj.f<T>)++;\n",
            "(a?.f<T>)++;\n",
        ],
        "postfix update on a bare instantiation",
    );
    assert!(!parses("f<T>++;\n"), "bare postfix update must not parse");
}

#[test]
fn postfix_update_keeps_a_composite_instantiation_pair_by_precedence() {
    // These end on an instantiation's `>` too, but the instantiation clause never
    // sees them: each is kept by the precedence clause, exactly as its
    // instantiation-free twin is (`(a * b)++`, `(-b)++`, `(await x)++`,
    // `(a ? b : c)++`, `(<T>x)++`). The re-lex below is a second, independent reason
    // the bare spelling could not have been authored.
    assert_stable(
        &[
            "(a * f<T>)++;\n",
            "(-f<T>)++;\n",
            "(await f<T>)++;\n",
            "(a ? b : f<T>)++;\n",
            "(<T>f<U>)++;\n",
        ],
        "postfix update on a composite ending in an instantiation",
    );
    for bare in ["a * f<T>++;\n", "-f<T>++;\n", "await f<T>++;\n"] {
        assert!(!parses(bare), "bare postfix update must not parse: {bare}");
    }
}

#[test]
fn prefix_update_strips_the_instantiation_pair() {
    // Nothing follows the type argument list, so there is nothing to re-lex.
    assert_eq!(format("++(f<T>);\n"), "++f<T>;\n");
    assert_eq!(format("--(f<T>);\n"), "--f<T>;\n");
}

#[test]
fn update_operand_strips_a_redundant_pair() {
    // A call, an optional chain and a template are all `LeftHandSideExpression`s, so
    // the pair carries nothing.
    for (source, expected) in [
        ("(a?.b)++;\n", "a?.b++;\n"),
        ("(f())++;\n", "f()++;\n"),
        ("(`x`)++;\n", "`x`++;\n"),
    ] {
        assert_eq!(format(source), expected);
    }
}
