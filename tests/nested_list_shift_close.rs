//! A nested type-argument list that closes INTO a shift token (`A<…>>`, `A<…>>>`), behind a
//! name in a `<` region: graded as a list of its own, so a body that is no type leaves the
//! source the comparison chain tsc and acorn-typescript read (`type_arg_region_walk`'s `<`
//! arm behind a name).
//!
//! The readings are a fixture claim
//! (`expressions/binary/relational_chain_nested_list_shift_close`).
//! Pinned here: the scope the grade keeps to — a nested list opened by the second `<` of a
//! `<<` keeps the claim the region makes, and the type parse rejects the line, as tsc does —
//! that the grade nests without running past the depth limit, and the bracket it keeps out of.

fn accepts(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::__private::parse(source, &arena).is_ok()
}

/// tsc reads a `<<` inside a type-argument region as two `<`, and rejects these: the claim
/// stands, so tsv rejects them too. Their acorn readings (shifts and comparisons) are the
/// ones a graded `<<` list would have taken.
#[test]
fn a_list_opened_by_a_shift_keeps_the_claim() {
    for source in [
        "x = a < b << c >>> `t`;",
        "fn(a < b << c, d >>> (await e));",
        "fn(a < b << c >>> (e));",
        "x = a < b << c, d >>> `t`;",
    ] {
        assert!(!accepts(source), "{source}");
    }
}

/// A list closed by a single `<` is graded, and a body that is no type reads as the
/// comparisons and the shift: the call, the arithmetic pair and the unary each refuse it,
/// at each level of a nest.
#[test]
fn a_list_closed_into_a_shift_is_graded() {
    for source in [
        "x = f < A < g(C) >> `t`;",
        "x = f < A < g(C) >> (t, u);",
        "x = f < A < (C * D) >> `t`;",
        "x = f < A < -C >> `t`;",
        "x = f < A < B < g(C) >>> `t`;",
        "x = f < A < c.d() >> (t);",
    ] {
        assert!(accepts(source), "{source}");
    }
    // a type body keeps the list
    for source in ["f<A<C>>`t`;", "f<A<B<C>>>(x);", "f<A<C | D>>(x);"] {
        assert!(accepts(source), "{source}");
    }
}

/// A nest of shift-closed lists far past the depth limit is graded level by level up to the
/// limit, past which the claim stands and the type parse refuses the depth: deep enough here
/// that a grade with no bound would run out of stack first.
#[test]
fn a_deep_nest_of_shift_closed_lists_ends_at_the_depth_limit() {
    let depth = 64 * tsv_lang::NESTING_LIMIT as usize;
    let source = format!("f<{}C{}>(x);", "A<".repeat(depth), ">".repeat(depth));
    assert!(!accepts(&source));
    // a comparison-shaped nest: the claim past the limit is what the depth error reports
    let depth = tsv_lang::NESTING_LIMIT as usize + 44;
    let source = format!(
        "x = f < {}g(C) {} `t`;",
        "A < ".repeat(depth),
        ">".repeat(depth + 1)
    );
    let arena = bumpalo::Bump::new();
    let error = tsv_ts::__private::parse(&source, &arena).expect_err("too deep");
    assert!(
        error.to_string().contains("depth limit"),
        "refused by the depth limit: {error}"
    );
}

/// The scope the grade keeps to inside the region: a list inside a bracket of it is the
/// bracketed-head family, whose body the type parse grades, and the printer's reading of the
/// pair around the chain keeps to that scope too.
#[test]
fn a_list_inside_a_bracket_keeps_the_region_reading() {
    let source = "x = f < A[B < C[!d] >>> (t, u)] > (u);";
    let printed = "x = (f < A[B < C[!d] >>> (t, u)]) > u;\n";
    assert_eq!(tsv_ts::format(source).ok().as_deref(), Some(printed));
}
