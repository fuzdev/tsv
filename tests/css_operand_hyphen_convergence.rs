// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Every authoring of a `-` that stands where an **operand** is expected reaches one
//! form in one pass — the F1 invariant, which the whitespace around such a `-` must not
//! decide.
//!
//! The hazard this guards: a run splitter that answers "is this `-` the operator?" with
//! postcss's WORD notion, under which `#`, `.`, `!`, `%`, `[`, `]`, `~`, `^` and the rest
//! of the fifteen bytes the CSS catalog's "Hyphen word extent at an operand position"
//! entry enumerates are word content.
//! Run-final the `-` is the operator on any reading, so `b: +- [a]` splits into two
//! operators and the head rule glues the `-` onto the block (`+-[a]`); read back, that
//! same text is `+` plus the single word `-[a]`, and a head `+` before a word keeps its
//! gap (`+ -[a]`). Two passes, two forms, and neither pass is wrong on its own terms.
//! The splitter instead asks css-syntax-3 of the `-` itself (§4.3.9 "would start an
//! identifier", §4.3.10's number — the lexer's `hyphen_starts_own_token`, the one reading
//! the printer's glue refusal takes too), so the `-` is the operator at every authoring.
//!
//! Prettier's own fixed point is `+ -[a]`; what it lacks is idempotence on the form it
//! writes from an **operator-glued** authoring — from `+- [a]` or `+ - [a]` its first
//! pass writes pass 1's `+-[a]`, and its second pass writes `+ -[a]`. tsv takes pass 1's
//! form, a cataloged divergence —
//! `tests/fixtures/css/values/operators/hyphen_word_extent_prettier_divergence` pins it
//! against the live oracle. That fixture also carries the one spacing prettier holds
//! (`+ -[a]`) as its `unformatted_ours_spaces`; what it cannot carry, and what this file
//! pins, is the two operator-glued spacings above: prettier normalizes each of them to
//! *tsv's* form, so neither is an `unformatted_ours_*` (prettier must not reach `input`)
//! and a divergence fixture may hold no plain `unformatted_*`.

/// `(label, authorings, expected)` — every authoring formats to `expected` in one pass,
/// and `expected` is a fixed point. The authorings differ only in the two gaps the three
/// members leave: the one between the operators and the one before the member.
const CASES: &[(&str, &[&str], &str)] = &[
    (
        "a `[…]` block after the `-`",
        &[
            "a{b: +-[a]}",
            "a{b: +- [a]}",
            "a{b: + -[a]}",
            "a{b: + - [a]}",
        ],
        "a {\n\tb: +-[a];\n}\n",
    ),
    (
        "a `#` after the `-`",
        &["a{b: +-#a}", "a{b: +- #a}", "a{b: + -#a}", "a{b: + - #a}"],
        "a {\n\tb: +-#a;\n}\n",
    ),
    (
        "a `.` no digit follows",
        &["a{b: +-.a}", "a{b: +- .a}", "a{b: + -.a}", "a{b: + - .a}"],
        "a {\n\tb: +-.a;\n}\n",
    ),
    (
        "a `!` after the `-`",
        &["a{b: +-!a}", "a{b: +- !a}", "a{b: + -!a}", "a{b: + - !a}"],
        "a {\n\tb: +-!a;\n}\n",
    ),
    (
        "a `~` after the `-` — the rule is the byte question, not the six-byte list",
        &["a{b: +-~a}", "a{b: +- ~a}", "a{b: + -~a}", "a{b: + - ~a}"],
        "a {\n\tb: +-~a;\n}\n",
    ),
    (
        "one operator further in, where the `/` has an operand on its left",
        &["a{b: 1.50 / /-[a]}", "a{b: 1.50 / / - [a]}"],
        "a {\n\tb: 1.5 / /-[a];\n}\n",
    ),
    // The bound: a byte that DOES open an ident or a number for the `-` keeps it inside
    // that token, so the `+` before it is a head `+` in front of a word and the gap
    // stands — the authorings converge the other way.
    (
        "the bound — an ident after the `-`",
        &["a{b: +-a}", "a{b: + -a}", "a{b: +  -a}"],
        "a {\n\tb: + -a;\n}\n",
    ),
    (
        "the bound — a number the `-` signs",
        &["a{b: +-1.50}", "a{b: + -1.50}"],
        "a {\n\tb: +-1.5;\n}\n",
    ),
];

fn format_css(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let stylesheet = tsv_css::parse(source, &arena).expect("parse failed");
    tsv_css::format(&stylesheet, source)
}

/// Each authoring formats to its case's expected form in one pass, and that form is a
/// fixed point. The one-pass assertion is what the word reading failed on the
/// operator-glued authorings (`+- [a]`); the fixed-point assertion is the F1 invariant
/// its output then violated.
#[test]
fn an_operand_position_hyphen_converges_in_one_pass() {
    for &(label, authorings, expected) in CASES {
        for authoring in authorings {
            let out = format_css(authoring);
            assert_eq!(
                out, expected,
                "case `{label}`: `{authoring}` should format to the expected form in one pass"
            );
        }
        let out_twice = format_css(expected);
        assert_eq!(
            out_twice, expected,
            "case `{label}`: the expected form must be a fixed point (idempotent)"
        );
    }
}
