// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Where a value wraps around a run whose operators the author left **glued**
//! (`1px+2px`, `fn(a)-webkit-x`, `100%-var(…)`, `1px+-(2px)`), and that every form it
//! wraps to is a fixed point.
//!
//! The rule (`build_value_member_parts`): a glued run is one fill item and travels whole,
//! and a function or parenthesized group in it that no line holds breaks inside its own
//! parens. No glued gap is a wrap point, because a break in one changes what the run is:
//! `1px+2px` is a dimension and a signed dimension, `1px-2px` is a single dimension whose
//! unit is `px-2px`, split by whitespace on either side of the `-`, `fn(a)-webkit-x` holds
//! a `-` an ident takes as its own first code point once whitespace stands ahead of it,
//! `-var(` is a single `<function-token>` that a break turns into a `-` and a real
//! `var()`, and a sign parted from its group is a bare operator ending a line — none of
//! them reads back as the same tokens once whitespace stands in the glue.
//!
//! The grid half pins the two emitters a `grid` / `grid-template*` value has — the fill
//! a one-line value takes and the row emitter a value with an authored newline takes —
//! answering a glued gap the same way, with an authored row break winning over glue the
//! rule would introduce.

fn format_css(source: &str) -> String {
    tsv_css::format_str(source).expect("format failed")
}

/// `source` formats to `expected` in one pass, and `expected` is a fixed point.
fn assert_converges(label: &str, source: &str, expected: &str) {
    assert_eq!(format_css(source), expected, "case `{label}`");
    assert_eq!(
        format_css(expected),
        expected,
        "case `{label}`: the expected form must be a fixed point"
    );
}

/// A glued run of plain operands, behind a pad sized so the declaration is one column
/// past the print width: the run moves to the next line whole.
#[test]
fn a_glued_run_of_plain_operands_travels_whole() {
    for run in [
        "1px+2px",
        "1px-2px",
        "fn(a)-webkit-x",
        "50%-2px",
        "(1)+1px",
        "var(--a)-10px",
        "1/-1",
        "12px/1.5",
        "16/9",
        "1/2/3/4",
        "10px/20px",
    ] {
        // `\tmargin: <pad> <run>;` at 101 columns.
        let pad = "p".repeat(101 - 2 - "margin: ".len() - 1 - run.len() - 1);
        let source = format!("a {{\n\tmargin: {pad} {run};\n}}\n");
        let expected = format!("a {{\n\tmargin: {pad}\n\t\t{run};\n}}\n");
        assert_converges(run, &source, &expected);
    }
}

/// The same runs across every pad that walks them over the print width, in a plain
/// property, a custom property and a `calc()` (which spaces a `/` and a `*` itself, so only
/// the fixed point is asked there): whatever the layout, it is a fixed point and the run
/// is never split.
#[test]
fn a_glued_run_of_plain_operands_is_never_split() {
    for run in ["1px+2px", "1px-2px", "fn(a)-webkit-x", "12px/1.5", "16/9"] {
        for pad in 60..=100 {
            let pad = "p".repeat(pad);
            for (source, keeps_run) in [
                (format!("a {{ margin: {pad} {run}; }}\n"), true),
                (format!("a {{ --x: {pad} {run}; }}\n"), true),
                (format!("a {{ width: calc({pad} {run}); }}\n"), false),
            ] {
                let formatted = format_css(&source);
                assert!(
                    !keeps_run || formatted.contains(run),
                    "`{run}` was split:\n{formatted}"
                );
                assert_eq!(
                    format_css(&formatted),
                    formatted,
                    "not a fixed point from:\n{source}"
                );
            }
        }
    }
}

/// A `-` glued to a named function is that function token's own first code point, so the
/// run moves to the next line whole — whatever stands ahead of the `-`.
#[test]
fn a_hyphen_glued_to_a_named_function_is_never_parted_from_it() {
    for run in [
        "100%-var(--y)",
        "10px-var(--y)",
        "(1)-var(--y)",
        "fn(a)-var(--y)",
    ] {
        // `\tmargin: <pad> <run>;` at 101 columns.
        let pad = "p".repeat(101 - 2 - "margin: ".len() - 1 - run.len() - 1);
        let source = format!("a {{\n\tmargin: {pad} {run};\n}}\n");
        let expected = format!("a {{\n\tmargin: {pad}\n\t\t{run};\n}}\n");
        assert_converges(run, &source, &expected);
    }
}

/// A function no line holds breaks inside its own parens — glued to the operator before
/// it where the author glued it (`100%-var(⏎…`), on a line of its own where the author
/// spaced it (`100%-⏎var(⏎…`) — each in one pass.
#[test]
fn a_function_in_a_glued_run_breaks_inside_its_own_parens() {
    let name = "j".repeat(88);
    assert_converges(
        "a space after the operator",
        &format!("a {{ width: calc(100%- var(--{name}) * 2); }}\n"),
        &format!(
            "a {{\n\twidth: calc(\n\t\t100%-\n\t\t\tvar(\n\t\t\t\t--{name}\n\t\t\t)\n\t\t\t* 2\n\t);\n}}\n"
        ),
    );
    assert_converges(
        "every operator glued",
        &format!("a {{ width: calc(100%-var(--{name})*2+1px); }}\n"),
        &format!(
            "a {{\n\twidth: calc(\n\t\t100%-var(\n\t\t\t\t--{name}\n\t\t\t)\n\t\t\t* 2+1px\n\t);\n}}\n"
        ),
    );
    // Two groups in one run: the one that has to break is the one that does.
    assert_converges(
        "the last group breaks, the first stays flat",
        &format!("a {{ margin: pppp var(--y)-fn(--{name}); }}\n"),
        &format!("a {{\n\tmargin: pppp\n\t\tvar(--y)-fn(\n\t\t\t--{name}\n\t\t);\n}}\n"),
    );
}

/// A `-` that follows another operator is a sign, and stays on its group: the run
/// `1px+-(2px)` is one item at every pad that walks it across the print width, in the
/// fill, in a `calc()` and in a grid value (where the line it wraps to is a row on the
/// next pass).
#[test]
fn a_sign_behind_an_operator_stays_on_its_group() {
    for pad in 80..=100 {
        let pad = "p".repeat(pad);
        for (source, run) in [
            (format!("a {{ grid: {pad} 1px+-(2px); }}\n"), "1px+-(2px)"),
            (format!("a {{ margin: {pad} 1px+-(2px); }}\n"), "1px+-(2px)"),
            (
                format!("a {{ width: calc({pad} -(2px) *-(2px)); }}\n"),
                "*-(2px)",
            ),
        ] {
            let formatted = format_css(&source);
            assert!(formatted.contains(run), "`{run}` was split:\n{formatted}");
            assert_eq!(
                format_css(&formatted),
                formatted,
                "not a fixed point from:\n{source}"
            );
        }
    }
}

/// A sign pair too wide for its line in a grid value: the fill moves it to a line of its
/// own as one item, that line is a row on the next pass, and the row emitter keeps the
/// sign on its group.
#[test]
fn a_sign_pair_in_a_grid_value_is_a_fixed_point() {
    let k = "k".repeat(50);
    let m = "m".repeat(50);
    assert_converges(
        "the pair opens a row",
        &format!("a {{ grid-template-columns: ppp aaa -({k} + {m}); }}\n"),
        &format!(
            "a {{\n\tgrid-template-columns:\n\t\tppp aaa\n\t\t-(\n\t\t\t{k} +\n\t\t\t\t{m}\n\t\t);\n}}\n"
        ),
    );
    for tail in [
        format!("-({k} + {m})"),
        format!("+({k} + {m})"),
        format!("/({k} + {m})"),
        format!("aaa -({k} + {m})"),
        format!("aaa -({k} + {m}) bbb"),
    ] {
        for property in ["grid", "grid-template-columns", "grid-template"] {
            for pad in 40..=100 {
                let source = format!("a {{ {property}: {} {tail}; }}\n", "p".repeat(pad));
                let formatted = format_css(&source);
                assert_eq!(
                    format_css(&formatted),
                    formatted,
                    "not a fixed point from:\n{source}"
                );
            }
        }
    }
}

/// A grid value with an authored row break: each gap inside a row is the value rule's,
/// so a glued run stays glued and an authored space stays a space.
#[test]
fn a_grid_row_keeps_its_glued_gaps() {
    assert_converges(
        "glued runs in a row",
        "a { grid-template: 'a a' 40px\n 'b c' 40px/1fr 1fr; }\n",
        "a {\n\tgrid-template:\n\t\t'a a' 40px\n\t\t'b c' 40px/1fr 1fr;\n}\n",
    );
    assert_converges(
        "a sign pair, a glued run and a spaced operator",
        "a { grid-template-columns: ppp\n aaa -(1px) 1px+2px fn(a)-webkit-x 1fr / auto; }\n",
        "a {\n\tgrid-template-columns:\n\t\tppp\n\t\taaa -(1px) 1px+2px fn(a)-webkit-x 1fr / auto;\n}\n",
    );
}

/// An authored row break wins over glue the value rule would introduce — a head `/` is
/// glued to the member after it on one line, and stays a row of its own across a newline
/// — so every row the plan counted is a row in the output, in one pass.
#[test]
fn an_authored_grid_row_break_wins_over_introduced_glue() {
    assert_converges(
        "a head `/` on a row of its own",
        "a { grid:\n /\n 1fr; }\n",
        "a {\n\tgrid:\n\t\t/\n\t\t1fr;\n}\n",
    );
    assert_converges(
        "an operator ahead of a group on the next row",
        "a { grid: 1fr -\n (2px); }\n",
        "a {\n\tgrid:\n\t\t1fr -\n\t\t(2px);\n}\n",
    );
}

/// A newline INSIDE a member is no row break at a glued gap: the run behind a function
/// the author broke across lines stays one run, and the value one line.
#[test]
fn a_glued_gap_behind_a_multiline_function_opens_no_grid_row() {
    assert_converges(
        "a glued run behind a function holding a newline",
        "a { grid: calc(1px +\n 2px)-a; }\n",
        "a {\n\tgrid: calc(1px + 2px)-a;\n}\n",
    );
}

/// A multi-line comment still ends the row it opens on, whatever is glued behind it.
#[test]
fn a_multiline_comment_ends_its_grid_row_at_a_glued_gap() {
    assert_converges(
        "an operator glued to a multi-line comment",
        "a { grid-template-areas: 50% /* c\n d */- [x]; }\n",
        "a {\n\tgrid-template-areas:\n\t\t50% /* c\n d */\n\t\t- [x];\n}\n",
    );
}
