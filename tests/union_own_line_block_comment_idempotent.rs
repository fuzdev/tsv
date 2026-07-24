// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]
// the TS object-type literals (`{x:1}`) are source-under-test, not format placeholders
#![allow(clippy::literal_string_with_formatting_args)]

//! An own-line block comment glued forward to a union member (`a |⏎/* c */ {x:1} | b`)
//! must reach the stable own-line broken form in ONE pass — the comment stays own-line
//! above the member's `| ` (prettier's fixed point, already stable under tsv), never
//! relocated to trail the pipe.
//!
//! Regression this guards: the union's forced-multiline routing keys on the one-sided
//! own-line test (`is_own_line_comment`), but the emission used to filter the comment out
//! of the own-line bucket via the opposite-sided `comment_hugs_next`, emitting it glued
//! after `| ` — pass 2 then saw no own-line comment, took the width-decided path, and
//! collapsed the union flat: a 2-pass non-idempotency (the `|⟨⟩␣` blank-audit shape).
//!
//! Not a fixture: the authored form's comment sits after the `|` while the normalized
//! form's sits before it — a token move the `unformatted_*` rules exclude — so the
//! transition is pinned here directly (the stable forms themselves are pinned by the
//! `types/comments/union_member_own_line_block_comment` fixture).

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

const EXPECTED: &str = "type A =\n\t| a\n\t/* c */\n\t| { x: 1 }\n\t| b;\n";

/// After-pipe own-line authoring (plain newline) → the own-line broken form, one pass.
#[test]
fn own_line_glued_forward_normalizes_in_one_pass() {
    let source = "type A = a |\n/* c */ {x:1} | b;\n";
    let pass1 = format(source);
    assert_eq!(
        pass1, EXPECTED,
        "comment must stay own-line above the member"
    );
    assert_eq!(
        format(&pass1),
        pass1,
        "the own-line broken form must be a fixed point"
    );
}

/// The blank-line authoring keeps the author's blank before the comment (prettier's
/// blank-preserving fixed point) and is equally one-pass stable.
#[test]
fn own_line_glued_forward_with_blank_preserves_blank() {
    let source = "type A = a |\n\n/* c */ {x:1} | b;\n";
    let expected = "type A =\n\t| a\n\n\t/* c */\n\t| { x: 1 }\n\t| b;\n";
    let pass1 = format(source);
    assert_eq!(
        pass1, expected,
        "the authored blank line before the comment is preserved"
    );
    assert_eq!(
        format(&pass1),
        pass1,
        "the blank form must be a fixed point"
    );
}
