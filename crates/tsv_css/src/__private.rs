//! Plumbing for the sibling tsv crates — the Svelte embedding (`tsv_svelte`), the
//! bindings' shared format path (`tsv_arena`), and the dev tooling. Not public API:
//! nothing here is covered by semver, and any item may change or go away in any release.

use super::*;

/// [`parse`] over a document the format path folded
/// (`tsv_lang::printing::normalize_carriage_returns`) — the parse every CSS format entry
/// point runs (the CLI, the bindings' `parse_format!`, [`format_str`]), its error reported in
/// the caller's own coordinates (`FoldedSource::parse_with`). The fold keeps meaning
/// everywhere in CSS, whose syntax normalizes a `<CR>` to `<LF>` itself (css-syntax-3
/// §Preprocessing the input stream), so this is the folded parse alone;
/// `tsv_svelte::__private::parse_folded` is the sibling that is not.
pub fn parse_folded<'arena>(
    folded: &tsv_lang::printing::FoldedSource<'_>,
    arena: &'arena bumpalo::Bump,
) -> Result<CssStyleSheet<'arena>> {
    folded.parse_with(|text| parse(text, arena))
}

/// Parse embedded CSS source into internal AST
///
/// Use this when parsing CSS embedded in another language (e.g., Svelte `<style>` tags)
/// where span positions need to reflect the offset in the parent file.
///
/// # Arguments
/// * `source` - CSS source code
/// * `base_offset` - Offset in parent file (for error reporting and span calculation)
/// * `host_nesting` - The nesting the host document has spent around the stylesheet, in
///   wire levels: the parse counts its own on top of it against `tsv_lang::NESTING_LIMIT`
///
/// # Returns
/// * `Ok(CssStyleSheet)` - Parsed AST with nodes and value comments
/// * `Err(Error)` - Parse error positioned in the parent file
///
/// `arena` owns the returned AST's nodes (shared with the host document's arena
/// when CSS is embedded in a Svelte `<style>`).
///
/// # Errors
///
/// An error is positioned in the parent file (`base_offset` included) but carries no
/// point — [`Error::wire_point`] is `None` — until the host fills its context over
/// the whole document ([`Error::with_context`], as `tsv_svelte::parse` does): a
/// context taken over `source`, a slice, would read that position against the wrong text.
pub fn parse_embedded<'arena>(
    source: &str,
    base_offset: usize,
    host_nesting: u32,
    arena: &'arena bumpalo::Bump,
) -> Result<CssStyleSheet<'arena>> {
    parser::parse_css(source, base_offset, host_nesting, arena)
}

/// [`format_in`] over a document the caller folded ahead of the parse
/// (`tsv_lang::printing::normalize_carriage_returns`) — the format entry points that fold
/// (the CLI, the bindings, [`format_str`]). Identical output; the document's line verdict
/// comes from the fold's own pass instead of a second walk of the source.
pub fn format_folded_in(
    stylesheet: &CssStyleSheet<'_>,
    folded: &tsv_lang::printing::FoldedSource<'_>,
    arena: &tsv_lang::doc::arena::DocArena,
) -> String {
    printer::format_css_folded_in(stylesheet, folded, arena)
}

/// Format an embedded CSS stylesheet into a caller-provided doc arena.
///
/// `embed.base_indent_offset` seeds the indent so wrapped lines respect the host's
/// indentation, and `source` is the whole host document (spans are absolute), so
/// `line_table` is the host's whole-source table — never island-local.
///
/// The doc IR is built into `arena` rather than a freshly allocated one, so a Svelte host
/// formatting a `<style>` block shares its own document arena instead of allocating a
/// second whole-host-sized `DocArena` per block. Nothing borrowed from `arena` escapes (the
/// embedded CSS renders to an owned `String`), and the arena is **not** reset, so the
/// host's in-flight doc nodes stay valid across the call.
///
/// `host_scan` is [`HostBoundaryScan::of`] over the same `source`, taken once per host and
/// handed to every island in it.
///
/// There is no fresh-arena twin: an embedded stylesheet always has a host arena to lend,
/// and the Svelte host is the only caller.
pub fn format_embedded_in(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    line_table: tsv_lang::printing::LineTable<'_>,
    embed: tsv_lang::EmbedContext,
    host_scan: HostBoundaryScan,
    arena: &tsv_lang::doc::arena::DocArena,
) -> String {
    printer::format_css_embedded_in(stylesheet, source, line_table, embed, host_scan, arena)
}

/// A host document's answer to the CSS printer's boundary-whitespace precondition — whether
/// the whole source holds a member of the class anywhere (the CSS printer's
/// §The document precondition, in `printer/boundary_ws.rs`).
///
/// One answer per host, not per island: every `<style>` island of a Svelte component is
/// spanned against the same host source, so the host takes it once ([`Self::of`]) and hands
/// it to each [`format_embedded_in`] call instead of paying a whole-source scan per island.
/// Opaque, so the only way to hold one is to have scanned a source.
#[derive(Clone, Copy, Debug)]
pub struct HostBoundaryScan {
    pub(crate) holds_boundary_ws: bool,
}

impl HostBoundaryScan {
    /// Scan `source`, the whole host document the islands are spanned against.
    #[must_use]
    pub fn of(source: &str) -> Self {
        Self {
            holds_boundary_ws: printer::source_holds_boundary_ws(source),
        }
    }
}

#[cfg(feature = "convert")]
pub use crate::ast::convert::{CssComments, write_css_children, write_css_comments};
