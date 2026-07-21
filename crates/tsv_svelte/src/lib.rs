//! Svelte parsing and formatting library
//!
//! This crate provides Svelte component parsing and code formatting.

pub mod ast;
mod lexer;
mod parser;
mod printer;

pub use tsv_lang::{Interner, ParseError, Result};

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
pub fn parse<'arena>(
    source: &str,
    arena: &'arena bumpalo::Bump,
    interner: &mut Interner,
) -> Result<Root<'arena>> {
    parser::parse_svelte(source, arena, interner).map_err(|e| e.with_context(source))
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
pub fn format(root: &Root<'_>, source: &str, interner: &Interner) -> String {
    printer::format_svelte(root, source, interner)
}

/// Parse and format `source` in one call, owning the interner.
///
/// The fully-fused one-shot convenience (the parse/format/interner analogue of
/// `tsv_ts::format_str`), for callers that just want the formatted string and
/// never touch the AST or interner. Batch drivers thread [`parse`] +
/// [`format_in`] with their own reusable [`Interner`] instead.
pub fn format_str(source: &str) -> Result<String> {
    let arena = bumpalo::Bump::new();
    let mut interner = Interner::new();
    let root = parse(source, &arena, &mut interner)?;
    Ok(format(&root, source, &interner))
}

/// Format into a caller-provided doc arena.
///
/// Identical output to [`format`], but the doc IR is built into `arena` instead
/// of a freshly allocated one, so a driver that formats many files can reuse one
/// arena across them (`arena.reset()` between files retains the buffers). Nothing
/// borrowed from `arena` escapes — the result is an owned `String`. Embedded
/// `<style>` blocks share this same arena (the CSS renders to its own string but
/// builds its doc nodes into the host arena, not a second per-block one).
pub fn format_in(
    root: &Root<'_>,
    source: &str,
    arena: &tsv_lang::doc::arena::DocArena,
    interner: &Interner,
) -> String {
    printer::format_svelte_in(root, source, arena, interner)
}

/// Convert internal AST to JSON with character-based positions
///
/// Returns a `serde_json::Value` parsed from the wire bytes
/// `convert_ast_json_bytes` emits — a thin wrapper over the sole emission
/// path, not an independent conversion. All byte-based positions (`start`,
/// `end`, `loc.*.column`, `character`) are already translated to Unicode
/// character offsets by the writer. Used where a `Value` is needed (the CLI's
/// `--pretty`, the fixture gate); byte-oriented consumers should call
/// `convert_ast_json_bytes` directly.
///
/// # Example
///
/// ```rust,ignore
/// let source = "<div>Hello</div>";
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_svelte::parse(source, &arena)?;
/// let json = tsv_svelte::convert_ast_json(&ast, source);
/// ```
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json(root: &Root<'_>, source: &str, interner: &Interner) -> serde_json::Value {
    serde_json::from_slice(&convert_ast_json_bytes(root, source, interner))
        .expect("writer emits valid JSON")
}

/// Parse and emit the compact wire-JSON string in one call, owning the interner.
///
/// The parse analogue of [`format_str`] — the fully-fused one-shot convenience
/// for callers that want the JSON string and never touch the AST or interner.
#[cfg(feature = "convert")]
pub fn parse_to_json(source: &str) -> Result<String> {
    let arena = bumpalo::Bump::new();
    let mut interner = Interner::new();
    let root = parse(source, &arena, &mut interner)?;
    Ok(convert_ast_json_string(&root, source, &interner))
}

/// Convert internal AST to compact JSON wire bytes with character-based positions
///
/// Emits the wire JSON directly during a single walk of the *internal* Svelte
/// AST — no typed public tree, no intermediate `serde_json::Value` for the
/// output. A **writer-mode conversion** (`ast/convert/write.rs`) fuses
/// byte→UTF-16 offset translation into the walk: the whole document — the
/// Svelte spine (elements, blocks, tags, directives, attributes, `name_loc`),
/// embedded template expressions and `<script>` content via `tsv_ts`'s
/// embedded writers, `<style>` children via `tsv_css`'s `write_css_node` —
/// emits final char-space positions directly. Comment-bearing islands
/// (template expressions with comments, comment-carrying `<script>`s) first
/// precompute a span-keyed `WriterComments` map off a byte-space skeleton
/// (`ast/convert/special.rs`), which the fused emit consults at each node's
/// close. This is the hot path for the FFI parse binding and the
/// CLI's compact output — the bytes are valid UTF-8 by construction (every
/// emitted byte is a source slice or ASCII fragment), and byte-oriented
/// consumers skip the O(output) validation a `String` requires.
///
/// The output is the Svelte parser's JSON shape; `convert_ast_json` parses these
/// bytes back into a `Value`.
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes(root: &Root<'_>, source: &str, interner: &Interner) -> Vec<u8> {
    ast::convert::write_root_bytes(root, source, interner)
}

/// Convert internal AST to compact JSON wire bytes **without** line/column data.
///
/// The opt-in `no-locations` variant of `convert_ast_json_bytes`: drops every
/// line/column object from the Svelte wire — the acorn `loc` on
/// `<script>`/`{expr}` nodes, the `name_loc` on elements/attributes/directives,
/// and the root-comment `loc` — keeping only `start`/`end` offsets. All are
/// derivable from those offsets plus source, so a consumer that has the source
/// loses nothing; a name's exact span reconstructs as `node.start + a fixed
/// per-node-type prefix`. Because this removes *all* line/column emission,
/// nothing queries the line table. Mirrors acorn's `locations: false`; a
/// distinct, narrower product from the default drop-in wire.
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes_no_locations(
    root: &Root<'_>,
    source: &str,
    interner: &Interner,
) -> Vec<u8> {
    ast::convert::write_root_bytes_no_locations(root, source, interner)
}

/// Convert internal AST to a compact JSON string with character-based positions
///
/// The `String` form of `convert_ast_json_bytes` for `&str` boundaries (the
/// WASM binding's `JSON.parse`, N-API strings): same wire bytes plus one
/// UTF-8 validation of the output. Byte-oriented consumers should prefer the
/// bytes variant.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(root: &Root<'_>, source: &str, interner: &Interner) -> String {
    String::from_utf8(convert_ast_json_bytes(root, source, interner))
        .expect("serde_json emits valid UTF-8")
}

/// The `String` form of `convert_ast_json_bytes_no_locations` for `&str`
/// boundaries (the WASM binding's `JSON.parse`, N-API strings).
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string_no_locations(
    root: &Root<'_>,
    source: &str,
    interner: &Interner,
) -> String {
    String::from_utf8(convert_ast_json_bytes_no_locations(root, source, interner))
        .expect("serde_json emits valid UTF-8")
}

/// Byte spans of the instance/module `<script>` element contents.
///
/// Comments inside these spans belong to the embedded TS programs; comments
/// outside them are template expression comments. Public so tooling can
/// extract script contents as standalone TS (e.g. the fixture suite's
/// typed-walk parity probes).
#[cfg(feature = "convert")]
pub fn script_content_spans(root: &Root<'_>) -> Vec<(u32, u32)> {
    let mut script_spans: Vec<(u32, u32)> = Vec::new();
    if let Some(script) = root.instance {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
    if let Some(script) = root.module {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
    script_spans
}

pub use ast::Root;
