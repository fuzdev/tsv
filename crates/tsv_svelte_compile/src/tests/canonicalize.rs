//! The JS canonicalizer: idempotence, intent erasure, and comment losslessness.

use super::support::*;
use crate::*;

#[test]
fn multiline_but_fitting_object_collapses() {
    // A short object authored expanded and the same object authored inline
    // must reach the SAME canonical form (expansion intent erased).
    let expanded = canonicalize_js("const x = {\n\ta: 1,\n\tb: 2\n};\n").unwrap();
    let inline = canonicalize_js("const x = {a: 1, b: 2};\n").unwrap();
    assert_eq!(
        expanded, inline,
        "multiline-but-fitting object must collapse"
    );
    assert!(
        !expanded.contains("a: 1,\n"),
        "should be single-line: {expanded:?}"
    );
}

#[test]
fn blank_lines_are_dropped() {
    let with_blanks = canonicalize_js("const a = 1;\n\n\nconst b = 2;\n").unwrap();
    let without = canonicalize_js("const a = 1;\nconst b = 2;\n").unwrap();
    assert_eq!(with_blanks, without, "blank lines must be erased");
    assert!(
        !with_blanks.contains("\n\n"),
        "no blank line survives: {with_blanks:?}"
    );
}

#[test]
fn over_width_construct_still_breaks() {
    // An object whose inline form exceeds the 100-col print width must break,
    // and both authorings (inline vs expanded) canonicalize identically.
    let long = "const config = {alpha: 1, bravo: 2, charlie: 3, delta: 4, echo: 5, \
                     foxtrot: 6, golf: 7, hotel: 8};\n";
    let inline = canonicalize_js(long).unwrap();
    assert!(
        inline.contains('\n'),
        "over-width object must break across lines"
    );
    // Same content, authored expanded, reaches the same canonical form.
    let expanded = canonicalize_js(
        "const config = {\n\talpha: 1,\n\tbravo: 2,\n\tcharlie: 3,\n\tdelta: 4,\n\techo: 5,\n\
             \tfoxtrot: 6,\n\tgolf: 7,\n\thotel: 8\n};\n",
    )
    .unwrap();
    assert_eq!(
        inline, expanded,
        "width-broken forms must be authoring-independent"
    );
}

#[test]
fn trailing_comment_survives() {
    let out = canonicalize_js("const x = 1; // keep me\n").unwrap();
    assert!(out.contains("// keep me"), "trailing comment lost: {out:?}");
}

#[test]
fn leading_comment_survives() {
    let out = canonicalize_js("// heading\nconst x = 1;\n").unwrap();
    assert!(out.contains("// heading"), "leading comment lost: {out:?}");
}

#[test]
fn consecutive_line_comments_do_not_merge() {
    // The losslessness edge case: two own-line line comments must stay on two
    // lines (never merge onto one, which would swallow the second `//`).
    let out = canonicalize_js("// first\n// second\nconst x = 1;\n").unwrap();
    assert!(out.contains("// first"), "first comment lost: {out:?}");
    assert!(out.contains("// second"), "second comment lost: {out:?}");
    // "// first // second" on one line would be the merge bug.
    assert!(
        !out.contains("// first // second"),
        "comments merged: {out:?}"
    );
}

#[test]
fn template_interpolation_chain_trailing_comment_stays_valid() {
    // D1: a `+` chain inside a template interpolation with an operand-trailing
    // `//` comment. Collapsing would trail the comment inside `${...}` and
    // swallow the closer (`${x + y // c})z`), making the output unparseable —
    // the chain must stay broken so the comment ends at a real line end.
    let out = assert_comments_lossless("const r = `(${x + // c\n\ty})z`;\n", &["// c"]);
    // The output must reparse (canonicalize_js validates this itself, but pin
    // the invariant explicitly at the test level too).
    canonicalize_js(&out).expect("D1 output must reparse");
}

#[test]
fn binary_chain_multiple_trailing_comments_do_not_merge() {
    // D2 (`+` chain): two operand-trailing comments must not merge onto one
    // trailing line (which also reorders them: `a + b + c; // two // one`).
    assert_comments_lossless(
        "const q = a + // one\n\tb + // two\n\tc;\n",
        &["// one", "// two"],
    );
}

#[test]
fn logical_chain_multiple_trailing_comments_do_not_merge() {
    // D2 (`||` chain): same class through the logical-expression path.
    assert_comments_lossless(
        "const ok = first || // one\n\tsecond || // two\n\tthird;\n",
        &["// one", "// two"],
    );
}

#[test]
fn chain_with_trailing_comments_as_call_arg_stays_lossless() {
    // Not-statement-final variant: the commented chain is a call argument, so
    // there is no statement end for a trailing comment to legally land on.
    assert_comments_lossless("f(a + // one\n\tb + // two\n\tc);\n", &["// one", "// two"]);
}

#[test]
fn chain_with_trailing_comments_as_array_element_stays_lossless() {
    // Not-statement-final variant: the commented chain is an array element
    // followed by another element — trailing past the `,` must not swallow it.
    assert_comments_lossless(
        "const xs = [a + // one\n\tb, // two\n\tc];\n",
        &["// one", "// two"],
    );
}

#[test]
fn block_comment_survives() {
    let out = canonicalize_js("const x = /* inline */ 1;\n").unwrap();
    assert!(out.contains("/* inline */"), "block comment lost: {out:?}");
}

#[test]
fn idempotent_on_samples() {
    assert_idempotent("const x = {\n\ta: 1\n};\n");
    assert_idempotent("const a = 1;\n\nconst b = 2;\n");
    assert_idempotent("// lead\nexport function f(x) {\n\treturn x + 1;\n}\n");
    assert_idempotent("import {a, b} from 'mod';\nconst t = `line\nbreak`;\n");
    assert_idempotent("const x = 1; // trailing\n// own line\nconst y = 2;\n");
}

#[test]
fn template_literal_newline_is_content_not_intent() {
    // A real newline inside a template literal is content, not layout intent —
    // it must survive canonicalization verbatim.
    let out = canonicalize_js("const t = `a\nb`;\n").unwrap();
    assert!(
        out.contains("`a\nb`"),
        "template literal newline not preserved: {out:?}"
    );
}

#[test]
fn blank_between_call_args_dropped() {
    // A blank line between call arguments is authoring intent — the canonical
    // reprint must erase it, exactly as it does between statements. Guards the
    // canonical gate on `is_next_line_empty` (a raw-`source` blank detector that,
    // ungated, would still see the authored blank on the canonical pass and force
    // expansion — the exact break the main-merge resolution of #534 introduced).
    let with_blank = canonicalize_js("f(a,\n\nb);\n").unwrap();
    let without = canonicalize_js("f(a, b);\n").unwrap();
    assert_eq!(
        with_blank, without,
        "blank between call args must be erased"
    );
    assert!(
        !with_blank.contains("\n\n"),
        "no blank line survives: {with_blank:?}"
    );
}
