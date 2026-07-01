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

/// Convert internal AST to public JSON-compatible AST
///
/// # Arguments
///
/// * `root` - The internal AST to convert
/// * `source` - The original source code (for location tracking)
///
/// # Returns
///
/// A public AST that can be serialized to JSON
///
/// # Example
///
/// ```rust,ignore
/// let source = "<div>Hello</div>";
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_svelte::parse(source, &arena)?;
/// let public_ast = tsv_svelte::convert_ast(&ast, source);
/// let json = serde_json::to_string_pretty(&public_ast)?;
/// ```
#[cfg(feature = "convert")]
pub fn convert_ast<'src>(root: &Root<'_>, source: &'src str) -> ast::public::Root<'src> {
    ast::convert::convert_root(root, source)
}

/// Convert internal AST to JSON with character-based positions
///
/// Like `convert_ast`, but returns `serde_json::Value` with all byte-based
/// positions (`start`, `end`, `loc.*.column`, `character`) translated to
/// Unicode character offsets to match Svelte/acorn output.
///
/// This is the `Value` oracle path: every pass here is `Value`-based
/// (whole-document attach + translate), independent of the typed walks, so
/// `convert_ast_json_string`'s byte-identity gates cross-check two
/// implementations. Use it when a `Value` is needed (pretty-printing,
/// fixture comparison); the compact-wire hot path is
/// `convert_ast_json_string`.
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
    // One tracker shared by conversion and translation (each build is a
    // full-source line-index scan).
    let tracker = tsv_lang::LocationTracker::new(source);
    let public_ast = ast::convert::convert_root_with_tracker(root, source, &tracker);
    let mut json = serde_json::to_value(&public_ast).expect("AST types derive Serialize correctly");

    // Attach comments to template expressions (outside <script> tags)
    // Must happen before byte→char translation since comment positions are byte-based
    let script_spans = script_content_spans(root);
    ast::convert::attach_template_expression_comments(
        &mut json,
        &root.comments,
        &script_spans,
        source,
    );

    let map = tsv_lang::ByteToCharMap::new(source);
    tsv_ts::ast::convert::translate_byte_to_char_offsets(&mut json, &map, &tracker);
    json
}

/// Convert internal AST to a compact JSON string with character-based positions
///
/// Byte-identical to `serde_json::to_string(&convert_ast_json(...))`, but
/// serializes the typed public AST directly — skipping the intermediate
/// `serde_json::Value` — on every input. Comments outside `<script>` content
/// spans go through the island-scoped attachment pass
/// (`ast::convert::attach_template_expression_comments_typed`), which converts
/// only the expressions they land on into `Value` islands (a no-op on the
/// common comment-free template). ASCII sources then serialize as-is;
/// multibyte sources get the typed offset-translation walk
/// (`ast::convert::translate_byte_to_char_offsets_typed`) first. This is the
/// hot path for the FFI/WASM parse bindings and the CLI's compact output.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(root: &Root<'_>, source: &str) -> String {
    let mut buf = Vec::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    // One tracker shared by conversion and the multibyte translation walk
    // (each build is a full-source line-index scan).
    let tracker = tsv_lang::LocationTracker::new(source);
    let mut public_ast = ast::convert::convert_root_with_tracker(root, source, &tracker);
    // Attach before translation — comment positions are byte-based.
    let script_spans = script_content_spans(root);
    ast::convert::attach_template_expression_comments_typed(
        &mut public_ast,
        &root.comments,
        &script_spans,
        source,
    );
    // `ByteToCharMap::new` short-circuits to an empty map for ASCII sources.
    let map = tsv_lang::ByteToCharMap::new(source);
    if map.has_multibyte() {
        ast::convert::translate_byte_to_char_offsets_typed(&mut public_ast, &map, &tracker);
    }
    serde_json::to_writer(&mut buf, &public_ast).expect("AST types derive Serialize correctly");
    String::from_utf8(buf).expect("serde_json emits valid UTF-8")
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
