// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A multi-line, non-`*`-aligned block comment leading a `for(…)` init clause is
//! preserved **verbatim** — its interior lines keep their authored columns, with no
//! context indent re-applied (matching prettier's non-indentable-block-comment
//! handling). When the header breaks, the comment rides with the init onto its own
//! line (`(` alone) — exactly how tsv lays out a leading multi-line block comment in
//! any `(`/`[`-delimited list (a call, an array), and what prettier emits here too.
//!
//! The hazard this guards: a printer that strips the comment's *start-line*
//! indentation and re-applies *context* indent per continuation line grows the
//! interior a tab **every** format pass — an F1 fixed-point violation. Not a fixture:
//! the case only reproduces at a non-zero base indent with a broken header.
//!
//! The comment is OWNED (every glued block comment is), so it rides with the init
//! exactly as in the `(`/`[`-list layout above, and tsv matches prettier here.

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

/// The stable form formats to itself — the interior compounds no tab per pass (the
/// fixed-point invariant).
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
