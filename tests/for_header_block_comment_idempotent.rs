// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A multi-line, non-`*`-aligned block comment leading a `for(…)` init clause is
//! preserved **verbatim** — its interior lines keep their authored columns, with no
//! context indent re-applied (matching prettier's non-indentable-block-comment
//! handling).
//!
//! Regression: the printer used to strip the comment's *start-line* indentation and
//! re-apply *context* indent per continuation line. When the `for(…)` header breaks,
//! the init (and its leading comment) render one level deeper than the `for`
//! keyword's line, so the stripped amount and the re-applied indent differed and the
//! interior grew a tab **every** format pass — an F1 fixed-point violation. Not a
//! fixture: the case only reproduces at a non-zero base indent with a broken header,
//! and it is a comment-position divergence from prettier (which relocates the
//! comment to its own line), so it is pinned here against tsv itself.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// The tsv-stable form: the `for` header breaks (init on line 1, then `;`-separated
/// clauses), and the block comment's continuation line `clause */` keeps its single
/// authored tab — it is NOT re-indented to the init's depth.
const STABLE: &str = "function f() {
\tfor (/* first
\tclause */ aaaaaaaaaaaaaaaaaaaa;
\t\tbbbbbbbbbbbbbbbbbb;
\t\tcccccccccccccccc
\t) {
\t\td();
\t}
}
";

/// The stable form formats to itself — the interior no longer compounds a tab per
/// pass (the fixed-point invariant the regression violated).
#[test]
fn for_header_block_comment_is_idempotent() {
    assert_eq!(format(STABLE), STABLE, "for-header block comment must be a fixed point");
    // Second pass, to be explicit that it does not drift after the first.
    assert_eq!(format(&format(STABLE)), STABLE, "still stable on the second pass");
}

/// The continuation line is preserved verbatim at a single leading tab — proof the
/// interior is not context-indented (which is what compounded before).
#[test]
fn for_header_block_comment_continuation_preserved_verbatim() {
    let out = format(STABLE);
    assert!(
        out.contains("\n\tclause */ "),
        "continuation keeps its single authored tab (no context indent): {out:?}"
    );
    assert!(
        !out.contains("\n\t\tclause */"),
        "continuation must NOT gain a second tab (the compounding regression): {out:?}"
    );
}
