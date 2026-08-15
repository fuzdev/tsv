// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! An **own-line** block comment in a gap whose comment run is pinned to the anchor's
//! line — the empty-statement body gap (`if (a)⏎/* c */⏎;`) and a labeled statement's
//! `:`→body gap (`l:⏎/* c */⏎fn();`) — converges in **one** pass.
//!
//! Regression this guards: the run's separators come from the shared leading-run
//! emitter, whose own-line arm forces a `hardline`. At these two sites the run is
//! emitted after a bare `" "`, never a `line`, so the comment always lands on the
//! anchor's line — which **erases the newline the author wrote before it**. Reading
//! `is_own_line_comment` there forces a break that the next pass removes (by then the
//! comment is no longer own-line), so the output is not a fixed point. That is exactly
//! why prettier needs two passes on these inputs; tsv answers the question `false` for
//! the run's FIRST comment (`LeadingGlue::AdjacentAnchorLine`) and lands directly.
//!
//! Not a fixture: the own-line form is a fixed point of *neither* formatter, so it
//! cannot be an `input` (F1) and cannot be an `unformatted_*` (which requires prettier
//! to normalize it in one pass too). The fixed points themselves are pinned by
//! `statements/if/empty_body_comment_run`, `statements/if/empty_body_own_line_block_two_pass_prettier_divergence`
//! (which pins prettier's intermediate) and `statements/labeled/comment_prettier_divergence`.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// Each pair is (own-line authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    ("if (a)\n/* c */\n;\n", "if (a) /* c */ ;\n"),
    (
        "if (a) fn();\nelse\n/* c */\n;\n",
        "if (a) fn();\nelse /* c */ ;\n",
    ),
    ("while (a)\n/* c */\n;\n", "while (a) /* c */ ;\n"),
    (
        "for (const k of o)\n/* c */\n;\n",
        "for (const k of o) /* c */ ;\n",
    ),
    ("l:\n/* c */\nfn();\n", "l: /* c */ fn();\n"),
];

#[test]
fn anchor_line_run_own_line_block_converges_in_one_pass() {
    for (authored, fixed_point) in CASES {
        let once = format(authored);
        assert_eq!(
            once, *fixed_point,
            "one pass must reach the fixed point for {authored:?}"
        );
        assert_eq!(
            format(&once),
            *fixed_point,
            "and stay there on the second pass for {authored:?}"
        );
    }
}

/// The carve-out is scoped to the run's FIRST comment: a later own-line comment keeps
/// its own line, because the separator that put it there reproduces the reading.
#[test]
fn anchor_line_run_later_comments_keep_their_lines() {
    let out = format("if (a)\n/* c1 */\n/* c2 */\n;\n");
    assert_eq!(
        out, "if (a) /* c1 */\n/* c2 */\n;\n",
        "the second comment keeps the line the author gave it"
    );
    assert_eq!(format(&out), out, "and that form is a fixed point");
}
