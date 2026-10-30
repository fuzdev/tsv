//! Plumbing for the sibling tsv crates — the bindings' shared format path (`tsv_arena`)
//! and the dev tooling. Not public API: nothing here is covered by semver, and any item
//! may change or go away in any release.

use super::*;

/// Parse a document the format path folded (`tsv_lang::printing::normalize_carriage_returns`)
/// — the parse every Svelte format entry point runs: the CLI, the bindings' `parse_format!`
/// and [`format_str`]. A parse error is reported in the caller's own coordinates
/// (`FoldedSource::parse_with`).
///
/// **The one place the fold could change meaning, refused.** Svelte's template reader ends
/// an in-tag `//` comment (`<div // c⏎class="x">`) at the next `\n` alone, so a lone `<CR>`
/// inside one is comment text — and folding it to `<LF>` would end the comment there, turning
/// the rest of its line into markup (a real `class` attribute above, or a parse of a document
/// whose own bytes do not parse). A `<CR>` with only whitespace after it in the comment (a
/// doubled `<CR><CR><LF>` ending) ends it where the fold does, and is let through. Everywhere else the fold keeps meaning (acorn ends a line at
/// a `<CR>` too; HTML normalizes it in text and attribute values; a block comment only changes
/// its own bytes). So when the fold rewrote a lone `<CR>`
/// (`FoldedSource::folded_lone_cr`), the author's own bytes are parsed first: their parse
/// error is this one (exactly what `parse` reports for them), and an in-tag `//` comment
/// holding a lone `<CR>` is refused ([`Error::refusal`], at the `<CR>` but no syntax
/// error — the document is valid Svelte). A `<CR><LF>` is one line break in both readings,
/// and a document the fold rewrote no lone `<CR>` in pays nothing.
pub fn parse_folded<'arena>(
    folded: &tsv_lang::printing::FoldedSource<'_>,
    arena: &'arena bumpalo::Bump,
) -> Result<Root<'arena>> {
    if folded.folded_lone_cr() {
        refuse_lone_cr_in_tag_line_comment(folded.original(), arena)?;
    }
    folded.parse_with(|text| parse(text, arena))
}

/// [`format_in`] over a document the caller folded ahead of the parse
/// (`tsv_lang::printing::normalize_carriage_returns`) — the format entry points that fold
/// (the CLI, the bindings, [`format_str`]). Identical output; the document's line verdict
/// comes from the fold's own pass instead of a second walk of the source.
pub fn format_folded_in(
    root: &Root<'_>,
    folded: &tsv_lang::printing::FoldedSource<'_>,
    arena: &tsv_lang::doc::arena::DocArena,
) -> String {
    printer::format_svelte_folded_in(root, folded, arena)
}
