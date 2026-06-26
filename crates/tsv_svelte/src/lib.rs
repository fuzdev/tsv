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
pub fn convert_ast(root: &Root<'_>, source: &str) -> ast::public::Root {
    ast::convert::convert_root(root, source)
}

/// Convert internal AST to JSON with character-based positions
///
/// Like `convert_ast`, but returns `serde_json::Value` with all byte-based
/// positions (`start`, `end`, `loc.*.column`, `character`) translated to
/// Unicode character offsets to match Svelte/acorn output.
///
/// This is the preferred function for producing JSON AST output.
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
    let public_ast = ast::convert::convert_root(root, source);
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
    let tracker = tsv_lang::LocationTracker::new(source);
    tsv_ts::ast::convert::translate_byte_to_char_offsets(&mut json, &map, &tracker);
    json
}

/// Convert internal AST to a compact JSON string with character-based positions
///
/// Byte-identical to `serde_json::to_string(&convert_ast_json(...))`, but
/// serializes the typed public AST directly — skipping the intermediate
/// `serde_json::Value` — when the source is ASCII (byte→char translation is a
/// no-op) **and** no comments fall outside `<script>` content spans (the
/// template-comment attachment pass is a no-op). Otherwise takes the current
/// `Value`-based path. This is the hot path for the FFI/WASM parse bindings
/// and the CLI's compact output.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(root: &Root<'_>, source: &str) -> String {
    let script_spans = script_content_spans(root);
    let mut buf = Vec::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    if source.is_ascii()
        && !ast::convert::has_template_expression_comments(&root.comments, &script_spans)
    {
        let public_ast = ast::convert::convert_root(root, source);
        serde_json::to_writer(&mut buf, &public_ast).expect("AST types derive Serialize correctly");
    } else {
        serde_json::to_writer(&mut buf, &convert_ast_json(root, source))
            .expect("Value serialization cannot fail");
    }
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
