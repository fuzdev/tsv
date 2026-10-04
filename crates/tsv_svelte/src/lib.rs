//! Svelte parsing and formatting library
//!
//! This crate provides Svelte component parsing and code formatting.

pub mod ast;
mod lexer;
mod parser;
mod printer;
#[cfg(test)]
mod test_support;
mod whitespace;

pub use ast::Root;
pub use tsv_lang::{ParseError, Result, WirePoint};

/// The coordinates a Svelte document's positions are reported in — Svelte's: a leading
/// BOM **elided** (Svelte's `parse` strips it before parsing, so every offset indexes the
/// BOM-less string, its acorn islands included), and `\n` alone ending a line, in the
/// markup and in every `<script>`, template expression and `<style>` alike. The wire
/// writer's tables and every parse error's point (`ParseError::wire_point`, and the
/// `line:col` its message prints) read this one statement.
pub const WIRE_COORDINATES: tsv_lang::WireCoordinates = tsv_lang::WireCoordinates {
    lines: tsv_lang::LineRule::Lf,
    bom: tsv_lang::LeadingBom::Elided,
};

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
/// * `Err(ParseError)` - If parsing fails
///
/// # Example
///
/// ```rust,ignore
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_svelte::parse("<div>Hello</div>", &arena)?;
/// ```
pub fn parse<'arena>(source: &str, arena: &'arena bumpalo::Bump) -> Result<Root<'arena>> {
    ParseError::ensure_source_fits(source)?;
    parser::parse_svelte(source, arena).map_err(|e| e.with_context(source, WIRE_COORDINATES))
}

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
/// holding a lone `<CR>` is refused ([`ParseError::refusal`], positionless — the document is
/// valid Svelte). A `<CR><LF>` is one line break in both readings, and a document the fold
/// rewrote no lone `<CR>` in pays nothing.
pub fn parse_folded<'arena>(
    folded: &tsv_lang::printing::FoldedSource<'_>,
    arena: &'arena bumpalo::Bump,
) -> Result<Root<'arena>> {
    if folded.folded_lone_cr() {
        refuse_lone_cr_in_tag_line_comment(folded.original(), arena)?;
    }
    folded.parse_with(|text| parse(text, arena))
}

/// [`parse_folded`]'s refusal: parse the author's own bytes, and refuse the first in-tag `//`
/// comment that holds a lone `<CR>`. Its parse lands in `arena` beside the folded one, which
/// the caller resets with it — a cost only a document holding a lone `<CR>` pays.
#[cold]
#[inline(never)]
fn refuse_lone_cr_in_tag_line_comment(source: &str, arena: &bumpalo::Bump) -> Result<()> {
    let root = parse(source, arena)?;
    let bytes = source.as_bytes();
    for comment in &root.comments {
        // Only the template reader's `//` runs to a `\n` alone; acorn's and the CSS
        // parser's end at a `<CR>` already.
        if !comment.from_template_reader || comment.is_block {
            continue;
        }
        // A `<CR>` opening a `<CR><LF>` is the comment's own line end, folded with its pair;
        // a lone one with only whitespace after it (`<CR><CR><LF>`, a doubled line ending)
        // ends the comment where the fold does, so nothing turns into markup.
        let content = comment.content_span.range();
        let end = content.end;
        let lone = content
            .clone()
            .find(|&i| bytes[i] == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
        if let Some(position) = lone
            && bytes[position + 1..end]
                .iter()
                .any(|b| !matches!(b, b' ' | b'\t' | b'\r'))
        {
            return Err(ParseError::refusal(
                "Lone carriage return inside a '//' comment in a tag: formatting folds it to a \
                 line feed, which would end the comment early",
                source,
                position,
                WIRE_COORDINATES,
            ));
        }
    }
    Ok(())
}

/// Format a Svelte AST back to source code
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
/// let ast = tsv_svelte::parse(source, &arena)?;
/// let formatted = tsv_svelte::format(&ast, source);
/// ```
pub fn format(root: &Root<'_>, source: &str) -> String {
    printer::format_svelte(root, source)
}

/// Parse and format `source` in one call.
///
/// The fully-fused one-shot convenience (the parse/format analogue of
/// `tsv_ts::format_str`), for callers that just want the formatted string and
/// never touch the AST. Batch drivers thread [`parse`] + [`format_in`] instead.
pub fn format_str(source: &str) -> Result<String> {
    // The format path's line-terminator fold, ahead of the parse (see
    // `tsv_lang::printing::normalize_carriage_returns`); `parse` leaves the author's bytes
    // alone so its offsets stay a drop-in contract with Svelte's.
    let folded = tsv_lang::printing::normalize_carriage_returns(source);
    let arena = bumpalo::Bump::new();
    let root = parse_folded(&folded, &arena)?;
    let doc_arena = tsv_lang::doc::arena::DocArena::for_source(folded.text());
    Ok(format_folded_in(&root, &folded, &doc_arena))
}

/// Format into a caller-provided doc arena.
///
/// Identical output to [`fn@format`], but the doc IR is built into `arena` instead
/// of a freshly allocated one, so a driver that formats many files can reuse one
/// arena across them (`arena.reset()` between files retains the buffers). Nothing
/// borrowed from `arena` escapes — the result is an owned `String`. Embedded
/// `<style>` blocks share this same arena (the CSS renders to its own string but
/// builds its doc nodes into the host arena, not a second per-block one).
pub fn format_in(root: &Root<'_>, source: &str, arena: &tsv_lang::doc::arena::DocArena) -> String {
    printer::format_svelte_in(root, source, arena)
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
/// let ast = tsv_svelte::parse(source, &arena)?;
/// let wire = tsv_svelte::convert_ast_json_bytes(&ast, source);
/// ```
#[cfg(feature = "convert")]
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
/// There is no `_with_locations` `String` twin: the `&str` boundaries this form serves are
/// the bindings, which ship the span-only wire alone, and the loc-bearing wire's only
/// callers (`tsv parse --locations`, `tsv_debug`) write bytes.
#[cfg(feature = "convert")]
#[expect(clippy::expect_used)]
pub fn convert_ast_json_string(root: &Root<'_>, source: &str) -> String {
    String::from_utf8(convert_ast_json_bytes(root, source))
        .expect("writer emits valid UTF-8 (source slices + ASCII fragments)")
}

/// Byte spans of the instance/module `<script>` element contents.
///
/// Comments inside these spans belong to the embedded TS programs; comments
/// outside them are template expression comments. The wire writer partitions
/// the root comment list on it.
#[cfg(feature = "convert")]
pub(crate) fn script_content_spans(root: &Root<'_>) -> Vec<(u32, u32)> {
    let mut script_spans: Vec<(u32, u32)> = Vec::new();
    if let Some(script) = root.instance {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
    if let Some(script) = root.module {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
    script_spans
}
