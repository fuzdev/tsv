//! A byte-adjacent pair of indentable block comments is ONE comment, at **every** format
//! entry point.
//!
//! The rule is prettier's parse-time splice (`mergeNestledJsdocComments`), which tsv states
//! in the printer's comment VIEW instead: its own `parse` is a drop-in for acorn / Svelte,
//! which emit two `Block` comments, so merging in the wire would move every fixture's
//! `expected.json` and both parse gates. The view is built where a format hands a printer
//! its comment list — and there are **three** such places, because a `<script>` island and
//! a template island build their environments from different arrays:
//!
//! - `tsv_ts::format_document_in` — a standalone `.ts` / `.js` document;
//! - `tsv_ts::build_program_doc` — a Svelte `<script>` body, which takes the island's own
//!   `Program.comments` rather than the host printer's array;
//! - `tsv_svelte`'s `format_root` — the host printer and every template `{expr}` island it
//!   constructs (`ts_inputs`), which share `Root.comments`.
//!
//! Two of the three are pinned by fixtures — `typescript/syntax/comments/nestled_block_run`
//! reaches the `<script>` one and `svelte/syntax/comments/expr_nestled_block_run` the
//! template one. The **standalone document** has no fixture: an `input.ts` would be
//! convertible (the construct formats identically inside `<script>`), so the seam it
//! covers, not the output, is the reason it would exist. Asserting the three agree states
//! that directly, and a seam someone forgets to merge shows up here as a disagreement
//! rather than as nothing at all.

/// The separator tsv must never emit inside a nestled pair — a space at the glued sites, a
/// hardline at the dangling ones, and neither belongs between two halves of one comment.
const SPLIT: &str = "*/ /**";

#[test]
fn standalone_document_welds_a_nestled_pair() {
    let out = tsv_ts::format_str("/** a\n *//** b\n */\nconst a = 1;\n").unwrap();
    assert_eq!(out, "/** a\n *//** b\n */\nconst a = 1;\n");
    assert!(
        !out.contains(SPLIT),
        "standalone document split the pair: {out:?}"
    );
}

/// A document whose ONLY content is a nestled pair — the one shape that reaches
/// `push_program_trailing_comments` with `has_output == false`, where the cursor is handed
/// back as 0 and the run scans `[0, source.len())`. A merged entry has to survive that arm
/// like any other, and no fixture reaches it: every fixture's trailing pair follows a
/// statement.
#[test]
fn comment_only_document_welds_a_nestled_pair() {
    let out = tsv_ts::format_str("/** a\n *//** b\n */\n").unwrap();
    assert_eq!(out, "/** a\n *//** b\n */\n");
}

#[test]
fn svelte_script_island_welds_a_nestled_pair() {
    let out =
        tsv_svelte::format_str("<script>\n\t/** a\n\t *//** b\n\t */\n\tconst a = 1;\n</script>\n")
            .unwrap();
    assert!(
        out.contains("/** a\n\t *//** b\n\t */"),
        "script island: {out:?}"
    );
    assert!(
        !out.contains(SPLIT),
        "script island split the pair: {out:?}"
    );
}

#[test]
fn svelte_template_island_welds_a_nestled_pair() {
    let out = tsv_svelte::format_str("{/** a\n *//** b\n */ expr}\n").unwrap();
    assert_eq!(out, "{/** a\n *//** b\n */ expr}\n");
    assert!(
        !out.contains(SPLIT),
        "template island split the pair: {out:?}"
    );
}

/// The scope is prettier's: the merge is a JS-parse postprocess, so a `<style>` sheet —
/// whose comments `tsv_css` collects into an array of its own — keeps its two comments.
/// prettier's CSS printer has no such splice, and a corpus `compare` agrees.
#[test]
fn css_does_not_weld_a_nestled_pair() {
    let out = tsv_svelte::format_str(
        "<style>\n\t/** a\n\t *//** b\n\t */\n\t.class {\n\t\tcolor: red;\n\t}\n</style>\n",
    )
    .unwrap();
    assert!(
        out.contains(SPLIT),
        "css welded a pair prettier leaves split: {out:?}"
    );
}

/// The `parse` wire is untouched: it is a drop-in for acorn / Svelte, which report two
/// `Block` comments for a nestled pair. Only the format path merges.
#[test]
fn parse_still_reports_two_comments() {
    let source = "/** a\n *//** b\n */\nconst a = 1;\n";
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).unwrap();
    assert_eq!(
        program.comments.len(),
        2,
        "the wire must keep both comments"
    );
}
