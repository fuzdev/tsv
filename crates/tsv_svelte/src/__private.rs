//! Plumbing for the sibling tsv crates — the bindings' shared format path (`tsv_arena`)
//! and the dev tooling. Not public API: nothing here is covered by semver, and any item
//! may change or go away in any release.

use super::*;

/// Parse a document the format path folded (`tsv_lang::printing::normalize_carriage_returns`)
/// — the parse every Svelte format entry point runs: the CLI, the bindings' `parse_format!`
/// and [`format()`]. A parse error is reported in the caller's own coordinates
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

/// [`format_ast_in`] over a document the caller folded ahead of the parse
/// (`tsv_lang::printing::normalize_carriage_returns`) — the format entry points that fold
/// (the CLI, the bindings, [`format()`]). Identical output; the document's line verdict
/// comes from the fold's own pass instead of a second walk of the source.
pub fn format_folded_in(
    root: &Root<'_>,
    folded: &tsv_lang::printing::FoldedSource<'_>,
    arena: &tsv_lang::doc::arena::DocArena,
) -> String {
    printer::format_svelte_folded_in(root, folded, arena)
}

/// Format a Svelte AST back to source code — the hidden, AST-taking primitive beneath [`format()`], which
/// is the public one-shot (parse and format a `&str`). Unlike [`format()`] it does no fold.
///
/// `source` must hold **no carriage return**: the printers split lines on `\n` alone, so a
/// `\r` would reach the output verbatim and the same document could format two ways on two
/// passes. Parse a CR-folded text (`tsv_lang::printing::normalize_carriage_returns`, via
/// `parse_folded`) and format it with `format_folded_in`, or call [`format()`], which
/// does both. Debug builds assert it.
///
/// # Arguments
///
/// * `root` - The Svelte AST to format
/// * `source` - The original source code (for blank line preservation and escape sequences)
///
/// # Returns
///
/// The formatted Svelte source code as a String
///
/// # Example
///
/// ```rust,ignore
/// let source = "<div>Hello</div>";
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_svelte::__private::parse(source, &arena)?;
/// let formatted = tsv_svelte::__private::format_ast(&ast, source);
/// ```
pub fn format_ast(root: &Root<'_>, source: &str) -> String {
    printer::format_svelte(root, source)
}

/// Format into a caller-provided doc arena.
///
/// The same precondition on `source` as [`fn@format_ast`]: no carriage return.
///
/// Identical output to [`fn@format_ast`], but the doc IR is built into `arena` instead
/// of a freshly allocated one, so a driver that formats many files can reuse one
/// arena across them (`arena.reset()` between files retains the buffers). Nothing
/// borrowed from `arena` escapes — the result is an owned `String`. Embedded
/// `<style>` blocks share this same arena (the CSS renders to its own string but
/// builds its doc nodes into the host arena, not a second per-block one).
pub fn format_ast_in(
    root: &Root<'_>,
    source: &str,
    arena: &tsv_lang::doc::arena::DocArena,
) -> String {
    printer::format_svelte_in(root, source, arena)
}

/// Parse Svelte source code into an internal AST
///
/// # Arguments
///
/// * `source` - The Svelte component source code to parse
/// * `arena` - The bump arena that owns the parsed graph (the template AST plus
///   the embedded TS `<script>`/`{expr}` ASTs, which share this one `Bump`); the
///   returned `Root<'arena>` borrows from it (caller-owns-`Bump`).
///
/// # Returns
///
/// * `Ok(Root)` - The parsed AST
/// * `Err(Error)` - If parsing fails
///
/// # Example
///
/// ```rust,ignore
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_svelte::__private::parse("<div>Hello</div>", &arena)?;
/// ```
pub fn parse<'arena>(source: &str, arena: &'arena bumpalo::Bump) -> Result<Root<'arena>> {
    Error::ensure_source_fits(source)?;
    parser::parse_svelte(source, arena).map_err(|e| e.with_context(source, WIRE_COORDINATES))
}

/// Convert internal AST to compact JSON wire bytes with character-based positions
///
/// The span-only wire every binding ships, and `tsv parse`'s default: the Svelte parser's
/// JSON shape with every line/column object dropped — every `loc` and the `name_loc` on
/// elements/attributes/directives — keeping only `start`/`end` offsets. All are derivable
/// from those offsets plus the source, so a consumer that has the source loses nothing (a
/// name's exact span is reconstructed from its node's offsets plus the source); nothing
/// queries the line table. Mirrors acorn's `locations: false`.
///
/// The **sole emission path** for its wire: emits the wire JSON directly during a
/// single walk of the *internal* Svelte AST — no typed public tree, no intermediate
/// `Value` for the output. A **writer-mode conversion** (`ast/convert/write.rs`) fuses
/// byte→UTF-16 offset translation into the walk: the whole document — the
/// Svelte spine (elements, blocks, tags, directives, attributes), embedded template
/// expressions and `<script>` content via `tsv_ts`'s embedded writers, `<style>`
/// children via `tsv_css`'s `write_css_children` — emits final char-space positions
/// directly (`start`, `end`). Comment-bearing islands (template expressions
/// with comments, comment-carrying `<script>`s) run acorn's attach **online** off this
/// same emit's node opens and closes (`ast/convert/comment_attachment.rs` declares each
/// island's window), so each node emits its own `leadingComments` / `trailingComments` at
/// its close — no second pass and no per-node map. The bytes are valid UTF-8 by
/// construction (every emitted byte is a source slice or ASCII fragment), and
/// byte-oriented consumers skip the O(output) validation a `String` requires.
///
/// # Example
///
/// ```rust,ignore
/// let source = "<div>Hello</div>";
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_svelte::__private::parse(source, &arena)?;
/// let wire = tsv_svelte::__private::convert_ast_json_bytes(&ast, source);
/// ```
#[cfg(feature = "json")]
pub fn convert_ast_json_bytes(root: &Root<'_>, source: &str) -> Vec<u8> {
    ast::convert::write_root_bytes(root, source, tsv_lang::Wire::Span)
}

/// The `convert_ast_json_bytes` wire plus a `loc` on every object carrying `start`/`end`
/// (a superset of Svelte's, which gives `loc` to acorn-parsed nodes only) and the
/// `name_loc` on elements/attributes/directives — line (1-based) and column (0-based,
/// UTF-16 code units) under `\n` alone, the one line rule for a Svelte document and
/// everything embedded in it. `tsv parse --locations` writes it.
#[cfg(feature = "locations")]
pub fn convert_ast_json_bytes_with_locations(root: &Root<'_>, source: &str) -> Vec<u8> {
    ast::convert::write_root_bytes(root, source, tsv_lang::Wire::Loc)
}

/// The `String` form of `convert_ast_json_bytes` for `&str` boundaries (the WASM
/// binding's `JSON.parse`, N-API strings): same wire bytes plus one UTF-8 validation of
/// the output. Byte-oriented consumers should prefer the bytes variant.
///
/// There is no `_with_locations` `String` twin: the bindings this form serves ship the
/// span-only wire alone, and `parse_to_json_with` validates the loc-bearing bytes itself.
#[cfg(feature = "json")]
#[expect(clippy::expect_used)]
pub fn convert_ast_json_string(root: &Root<'_>, source: &str) -> String {
    String::from_utf8(convert_ast_json_bytes(root, source))
        .expect("writer emits valid UTF-8 (source slices + ASCII fragments)")
}
