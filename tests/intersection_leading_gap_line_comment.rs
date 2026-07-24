// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]
// the TS object-type literals (`{x:1}`) are source-under-test, not format placeholders
#![allow(clippy::literal_string_with_formatting_args)]

//! A line comment in an intersection's leading-`&` gap (`=⏎& ⏎// c⏎{x:1} & b`) must be
//! PRESERVED — the intersection's leading-gap comment extraction is block-only, and no
//! routing predicate scans `[span.start, first_type_start)`, so every line comment there
//! (a plain comment or a `prettier-ignore` directive) was silently dropped: comment loss
//! (hazard-1 class), and for a directive a phantom one-pass freeze (member frozen on pass
//! 1, directive gone, freeze lost on pass 2).
//!
//! Targets are the hosts' own fixed points, because pass 2 re-reads these comments
//! OUTSIDE the intersection span (the bare leading `&` never survives formatting):
//!
//! - a **plain line comment** converges in one pass to the trailing-prefix form the
//!   paren-hoist path already produces (`type T = // c⏎⇥{ x: 1 } & b;`) — probe-verified
//!   stable at the alias AND annotation hosts (the own-line form is only stable at the
//!   alias, so it cannot be the universal target);
//! - a **directive** (alone on its line) stays own-line and freezes the first member
//!   (Rule A) — the own-line directive form is the fixed point both hosts keep.
//!
//! Not a fixture: the authored form carries a bare leading `&` that canonical output
//! drops — a token move the `unformatted_*` rules exclude — so the transitions are pinned
//! here directly (the target forms themselves are ordinary, fixture-covered shapes).

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// Assert `source` formats to `expected` in ONE pass and `expected` is a fixed point.
fn assert_one_pass(source: &str, expected: &str) {
    let pass1 = format(source);
    assert_eq!(pass1, expected, "one-pass convergence");
    assert_eq!(
        format(&pass1),
        pass1,
        "the target form must be a fixed point"
    );
}

/// Two-member alias: the leading-gap line comment survives, trailing the `=`.
#[test]
fn two_member_alias_line_comment() {
    assert_one_pass(
        "type T =\n\t&\n\t// c\n\t{x: 1} & b;\n",
        "type T = // c\n\t{ x: 1 } & b;\n",
    );
}

/// Two comments: the first trails the `=`, the rest stay own-line — none are dropped,
/// none merge.
#[test]
fn two_member_alias_two_line_comments() {
    assert_one_pass(
        "type T =\n\t&\n\t// c1\n\t// c2\n\t{x: 1} & b;\n",
        "type T = // c1\n\t// c2\n\t{ x: 1 } & b;\n",
    );
}

/// A directive alone on its line in the leading gap: emitted own-line (never relocated —
/// trailing placements are inert, so relocation would lose the freeze on pass 2) and the
/// first member freezes verbatim.
#[test]
fn two_member_alias_directive_freezes_first_member() {
    assert_one_pass(
        "type T =\n\t&\n\t// prettier-ignore\n\t{x: 1} & b;\n",
        "type T =\n\t// prettier-ignore\n\t{x: 1} & b;\n",
    );
}

/// Annotation host (delegates to the shared bare printer): same preservation, same
/// trailing-`:` target.
#[test]
fn two_member_annotation_line_comment() {
    assert_one_pass(
        "let v:\n\t&\n\t// c\n\t{x: 1} & b;\n",
        "let v: // c\n\t{ x: 1 } & b;\n",
    );
}

/// Annotation host with a directive: own-line + first-member freeze.
#[test]
fn two_member_annotation_directive_freezes_first_member() {
    assert_one_pass(
        "let v:\n\t&\n\t// prettier-ignore\n\t{x: 1} & b;\n",
        "let v:\n\t// prettier-ignore\n\t{x: 1} & b;\n",
    );
}

/// Leading-gap comment PLUS a between-member comment (the forced-multiline layout): both
/// survive; the body keeps the line-comment layout's continuation indent.
#[test]
fn compound_leading_and_mid_comment() {
    assert_one_pass(
        "type T =\n\t&\n\t// lead\n\t{x: 1} &\n\t// mid\n\tb;\n",
        "type T = // lead\n\t{ x: 1 } &\n\t\t// mid\n\t\tb;\n",
    );
}

/// Single-member collapse: the member's `&` drops, the comment survives.
#[test]
fn single_member_alias_line_comment() {
    assert_one_pass(
        "type T =\n\t&\n\t// c\n\t{x: 1};\n",
        "type T = // c\n\t{ x: 1 };\n",
    );
}

/// Single-member annotation: the comment already survived here, but the body rendered at
/// column 0 on pass 1 and re-indented on pass 2 — a 2-pass transient. One pass now.
#[test]
fn single_member_annotation_line_comment_indent_stable() {
    assert_one_pass(
        "let v:\n\t&\n\t// c\n\t{x: 1};\n",
        "let v: // c\n\t{ x: 1 };\n",
    );
}

/// No-regression control: an own-line BLOCK comment in the leading gap keeps its current
/// glued-inline emission (block comments were never dropped here).
#[test]
fn block_comment_control_unchanged() {
    assert_one_pass(
        "type T =\n\t&\n\t/* c */\n\t{x: 1} & b;\n",
        "type T = /* c */ { x: 1 } & b;\n",
    );
}
