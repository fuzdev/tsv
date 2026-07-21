// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! An update expression whose operand is a **type assertion** must keep its
//! parens: bare `a as T++` binds `++` to the type (`a as (T++)`), a different —
//! and here invalid — expression. `(a as T)++` is fixture-covered
//! (`typescript/expressions/unary/update_operand_paren`), but the `satisfies`
//! and angle-bracket (`<T>a`) forms can't be: acorn-typescript rejects the
//! *input* ("Assigning to rvalue"), so no `expected.json` oracle exists. They're
//! pinned here against tsv's own formatting (prettier keeps the parens too). See
//! `crates/tsv_ts/CLAUDE.md` §Sources of truth.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_ts::Interner::new();
    let program = tsv_ts::parse(source, &arena, &mut interner).expect("parse failed");
    tsv_ts::format(&program, source, &interner)
}

/// `(a satisfies T)++` — the `satisfies` cast keeps its parens (postfix).
#[test]
fn satisfies_operand_of_postfix_update_keeps_parens() {
    let canonical = "(a satisfies T)++;\n";
    assert_eq!(format(canonical), canonical, "idempotent");
}

/// `(<T>a)++` — an angle-bracket assertion keeps its parens (postfix).
#[test]
fn angle_bracket_operand_of_postfix_update_keeps_parens() {
    let canonical = "(<T>a)++;\n";
    assert_eq!(format(canonical), canonical, "idempotent");
}

/// Prefix updates over a cast keep the parens too.
#[test]
fn cast_operand_of_prefix_update_keeps_parens() {
    for canonical in ["--(a satisfies T);\n", "++(<T>a);\n"] {
        assert_eq!(format(canonical), canonical, "idempotent: {canonical:?}");
    }
}

/// Boundary: a plain-reference operand keeps no parens (redundant stripped),
/// so the new rule fires only for the assertion forms.
#[test]
fn plain_reference_update_strips_redundant_parens() {
    assert_eq!(format("(a.b)++;"), "a.b++;\n");
}
