//! Svelte parsing and formatting library
//!
//! This crate provides Svelte component parsing and code formatting.

pub mod ast;
mod lexer;
mod parser;
mod printer;

pub use tsv_lang::{ParseError, Result};

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
    parser::parse_svelte(source, arena).map_err(|e| e.with_context(source))
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

/// Format into a caller-provided doc arena.
///
/// Identical output to [`format`], but the doc IR is built into `arena` instead
/// of a freshly allocated one, so a driver that formats many files can reuse one
/// arena across them (`arena.reset()` between files retains the buffers). Nothing
/// borrowed from `arena` escapes — the result is an owned `String`. (Embedded
/// `<style>` blocks still format through their own per-block arena.)
pub fn format_in(root: &Root<'_>, source: &str, arena: &tsv_lang::doc::arena::DocArena) -> String {
    printer::format_svelte_in(root, source, arena)
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
pub fn convert_ast_json(root: &Root<'_>, source: &str) -> serde_json::Value {
    serde_json::from_slice(&convert_ast_json_bytes(root, source)).expect("writer emits valid JSON")
}

/// Convert internal AST to compact JSON wire bytes with character-based positions
///
/// Byte-identical to `serde_json::to_string(&convert_ast_json(...))`, but emits
/// the wire JSON directly during a single walk of the *internal* Svelte AST — the
/// typed public `Root` is never materialized. A **writer-mode conversion**
/// (`ast/convert/write.rs`) fuses byte→UTF-16 offset translation into the walk:
/// the Svelte spine (elements, blocks, tags, directives, attributes, `name_loc`)
/// emits final char-space positions directly, comment-free template expressions
/// route through `tsv_ts`'s embedded expression writer, and the residual
/// `serde_json::Value` islands (root comments, `<svelte:options>`, block
/// patterns, `{@const}`/`{const}`/`{let}` declarations, `<svelte:element>` tags,
/// `<script>` content) reuse the existing byte-space convert + attach builders,
/// translated per island. Embedded `<style>` children fuse via `tsv_css`'s
/// `write_css_node`. This is the hot path for the FFI parse binding and the
/// CLI's compact output — the bytes are valid UTF-8 by construction (every
/// emitted byte is a source slice or ASCII fragment), and byte-oriented
/// consumers skip the O(output) validation a `String` requires.
///
/// The output is the Svelte parser's JSON shape; `convert_ast_json` parses these
/// bytes back into a `Value`.
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes(root: &Root<'_>, source: &str) -> Vec<u8> {
    ast::convert::write_root_bytes(root, source)
}

/// Convert internal AST to a compact JSON string with character-based positions
///
/// The `String` form of `convert_ast_json_bytes` for `&str` boundaries (the
/// WASM binding's `JSON.parse`, N-API strings): same wire bytes plus one
/// UTF-8 validation of the output. Byte-oriented consumers should prefer the
/// bytes variant.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(root: &Root<'_>, source: &str) -> String {
    String::from_utf8(convert_ast_json_bytes(root, source)).expect("serde_json emits valid UTF-8")
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
