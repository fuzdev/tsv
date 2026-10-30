// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! Stability pins for a unary operand whose chain head prints a pair of its own around a
//! comment: the head pair — a sequence's, a binary's — prints first and holds the comment, so
//! the operand takes no comment-holder pair (the holder pair a comment in an erased head shell
//! earns is the fixture claim
//! `expressions/unary/operand_left_spine_shell_block_comment_prettier_divergence`). Each case
//! is a fixed point and prints as written.
//!
//! Prettier answers this family differently (`typeof (/* c */ (a + b).m())`), a difference
//! older than these pins; they guard tsv's own output, not a sanctioned divergence.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

/// Each pair is (authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    // A sequence head's pair prints first and holds the comment.
    (
        "typeof (/* c */ a, b).m();\n",
        "typeof (/* c */ a, b).m();\n",
    ),
    // So does a binary head's pair, which the printer re-emits.
    (
        "typeof (/* c */ a + b).m();\n",
        "typeof (/* c */ a + b).m();\n",
    ),
];

#[test]
fn a_head_pair_holds_its_comment_with_no_holder_pair() {
    for &(source, expected) in CASES {
        let once = format(source);
        assert_eq!(once, expected, "{source:?}");
        assert_eq!(format(&once), once, "not a fixed point: {source:?}");
    }
}
