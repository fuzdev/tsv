// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A multi-line, non-`*`-aligned block comment leading a `for(…)` init clause is
//! preserved **verbatim** — its interior lines keep their authored columns, with no
//! context indent re-applied (matching prettier's non-indentable-block-comment
//! handling). When the header breaks, the comment rides with the init onto its own
//! line (`(` alone) — exactly how tsv lays out a leading multi-line block comment in
//! any `(`/`[`-delimited list (a call, an array), and what prettier emits here too.
//!
//! Regression this guards: the printer used to strip the comment's *start-line*
//! indentation and re-apply *context* indent per continuation line, so the interior
//! grew a tab **every** format pass — an F1 fixed-point violation. Not a fixture: the
//! case only reproduces at a non-zero base indent with a broken header.
//!
//! Note: tsv formerly kept the comment glued to `for (` (init hugging the `(` line) as
//! a deliberate divergence. Owning every glued block comment unified that with the
//! `(`/`[`-list layout above — the owned comment now rides with the init — so tsv
//! matches prettier here and the over-preservation divergence is closed. The F1
//! invariant this test guards (no tab compounding) is unaffected.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// The tsv-stable form: the `for` header breaks with `(` alone, then the init (and
/// its leading comment) on the next line at the clause indent, then the `;`-separated
/// clauses. The block comment's continuation line `clause */` keeps its single
/// authored tab — it is NOT re-indented to the init's depth.
const STABLE: &str = "function f() {
\tfor (
\t\t/* first
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
    assert_eq!(
        format(STABLE),
        STABLE,
        "for-header block comment must be a fixed point"
    );
    // Second pass, to be explicit that it does not drift after the first.
    assert_eq!(
        format(&format(STABLE)),
        STABLE,
        "still stable on the second pass"
    );
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
