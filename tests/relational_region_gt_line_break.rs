// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Where an open `<` region BEGINS and ENDS — the lifetime half of the rule that a lone `>`
//! which may close a type-argument region never ends a line
//! (`BinaryExpression::may_close_type_arguments`, `docs/conformance_prettier_ts.md`
//! §Relational chain type-argument parens).
//!
//! The width boundary and the sweep of what carries tsc's recovery to the `>` are fixture
//! claims (`expressions/binary/relational_region_inner_gt_break_long_prettier_divergence`).
//! What is pinned here is the parser's scoping, one shape per rule, which a `long` fixture
//! would carry only as a pile of near-identical 100-column cells: each case formats a line
//! that must break at its `>` and asks which side of the `>` the break landed on.
//!
//! The two directions are not symmetric, and only one is a claim about tsc. Every case that
//! ENDS a region was graded against tsc with the break after the `>` and reads as the
//! comparison — ending a region early is the unsound direction, so that is the half that
//! needs an oracle. The cases that keep one OPEN are the superset's: some are tsc's rejects
//! in the printed form (`x < (a >⏎b)`, the comma siblings, an arrow's block body once its
//! `return` takes the parens a broken argument gets), others tsc happens to abandon
//! (`function`, `class`, a call between the siblings) and cost only a line break ahead of
//! the `>` where one after it would have done.

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// Which side of its `>` the `A > B` comparison in `template` broke on.
#[derive(Debug, PartialEq, Eq)]
enum Break {
    /// `aaa⏎> bbb` — the region may still be open.
    Ahead,
    /// `aaa >⏎bbb` — the ordinary layout.
    After,
}

fn break_side(template: &str) -> Break {
    let source = template.replace('A', A).replace('B', B);
    let output = tsv_ts::format_str(&source).expect("the case parses");
    assert_eq!(
        tsv_ts::format_str(&output).expect("the output reparses"),
        output,
        "not a fixed point: {template}"
    );
    let line_of = |needle: &str| {
        output
            .lines()
            .find(|line| line.contains(needle))
            .expect("each operand prints on a line")
            .trim()
            .to_string()
    };
    let (ahead, after) = (line_of(B).starts_with("> "), line_of(A).ends_with(" >"));
    assert!(
        ahead != after,
        "`A > B` did not break at its operator: {output}"
    );
    if ahead { Break::Ahead } else { Break::After }
}

#[test]
fn a_region_is_open_behind_a_less_than() {
    for template in [
        "x < (A > B);",
        "x << (A > B);",
        // No shell: the comma is the type-argument separator.
        "fn(x < q, A > B);",
        "[x < q, A > B];",
        // A grouping paren is the one delimiter the printer may strip, so its `)` closes
        // nothing: this prints as the comma list above.
        "fn((x < q), A > B);",
        // The earliest region outlives a later, deeper one.
        "fn(x < q, g(y < r), A > B);",
    ] {
        assert_eq!(break_side(template), Break::Ahead, "{template}");
    }
}

#[test]
fn a_body_inherits_the_region_it_was_written_in() {
    for template in [
        "x < (() => { return A > B; });",
        "x < (() => { q; return A > B; });",
        "x < function () { return A > B; };",
        "x < class { p = A > B; };",
    ] {
        assert_eq!(break_side(template), Break::Ahead, "{template}");
    }
}

#[test]
fn a_region_ends_at_the_closer_of_the_delimiter_it_opened_in() {
    for template in [
        "fn(x < q)(A > B);",
        "[x < q][A > B];",
        "fn({ k: x < q }, A > B);",
        "fn(`${x < q}`, A > B);",
        // A body's own regions die with it.
        "fn(function () { return x < q; }, A > B);",
    ] {
        assert_eq!(break_side(template), Break::After, "{template}");
    }
}

#[test]
fn a_region_ends_at_a_statement_or_member_boundary() {
    for template in [
        "x < q; fn(A > B);",
        "if (x < q) fn(A > B);",
        "if (x < q) { fn(A > B); }",
        "for (let i = 0; i < n; i++) fn(A > B);",
        "while (x < q) fn(A > B);",
        "class C { p = x < q; r = A > B; }",
        // … including inside a body that inherited none.
        "fn(() => { if (x < q) { y; } return A > B; });",
    ] {
        assert_eq!(break_side(template), Break::After, "{template}");
    }
}

#[test]
fn a_less_than_in_the_greater_thans_own_left_operand_is_the_pair_rules() {
    // `(x < q) > …` is the chain `relexes_as_type_arguments` answers with a pair, which ends
    // the region on a `)`; the `>` there ends its line.
    let output = tsv_ts::format_str(&format!("const k = ({A} < q) > {B};")).expect("parses");
    assert!(output.contains(" >\n"), "{output}");
    // … and a `<` inside its RIGHT operand does not stand ahead of it at all.
    assert_eq!(break_side("fn(A > B, x < q);"), Break::After);
}

#[test]
fn only_a_lone_greater_than_closes_a_region() {
    for operator in [">=", ">>", ">>>"] {
        let output = tsv_ts::format_str(&format!("x < ({A} {operator} {B});")).expect("parses");
        assert!(output.contains(&format!(" {operator}\n")), "{output}");
    }
}
