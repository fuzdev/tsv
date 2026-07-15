//! CSS parsing and formatting library
//!
//! Provides CSS parsing, formatting, and AST conversion functionality.
//! Part of the tsv (precise language tools for TypeScript/JS, CSS, and Svelte in Rust) project.

pub mod ast;
mod color;
mod escapes;
mod keyword_set;
mod lexer;
mod number;
mod parser;
mod printer;
mod url;
mod whitespace;

// Re-export commonly used types
pub use ast::{CssDeclaration, CssNode, CssRule, CssStyleSheet};
pub use tsv_lang::{ParseError, Result};

/// Parse CSS source into internal AST
///
/// # Arguments
/// * `source` - CSS source code
///
/// # Returns
/// * `Ok(CssStyleSheet)` - Parsed AST with nodes and value comments
/// * `Err(ParseError)` - Parse error with position and context
///
/// # Example
/// ```
/// use tsv_css::parse;
///
/// let css = "div { color: red; }";
/// let arena = bumpalo::Bump::new();
/// let stylesheet = parse(css, &arena).expect("Failed to parse CSS");
/// ```
///
/// `arena` owns every AST node of the returned [`CssStyleSheet`]
/// (caller-owns-`Bump`); the stylesheet borrows from it for `'arena`.
pub fn parse<'arena>(source: &str, arena: &'arena bumpalo::Bump) -> Result<CssStyleSheet<'arena>> {
    parser::parse_css(source, 0, arena).map_err(|e| e.with_context(source))
}

/// Parse embedded CSS source into internal AST
///
/// Use this when parsing CSS embedded in another language (e.g., Svelte `<style>` tags)
/// where span positions need to reflect the offset in the parent file.
///
/// # Arguments
/// * `source` - CSS source code
/// * `base_offset` - Offset in parent file (for error reporting and span calculation)
///
/// # Returns
/// * `Ok(CssStyleSheet)` - Parsed AST with nodes and value comments
/// * `Err(ParseError)` - Parse error with position and context
///
/// `arena` owns the returned AST's nodes (shared with the host document's arena
/// when CSS is embedded in a Svelte `<style>`).
pub fn parse_embedded<'arena>(
    source: &str,
    base_offset: usize,
    arena: &'arena bumpalo::Bump,
) -> Result<CssStyleSheet<'arena>> {
    parser::parse_css(source, base_offset, arena).map_err(|e| e.with_context(source))
}

/// Format CSS stylesheet to a formatted string
///
/// # Arguments
/// * `stylesheet` - CSS stylesheet (nodes + value comments)
/// * `source` - Original CSS source code (for blank line preservation)
///
/// # Returns
/// * Formatted CSS string
///
/// # Example
/// ```
/// use tsv_css::{parse, format};
///
/// let css = "div{color:red;}";
/// let arena = bumpalo::Bump::new();
/// let stylesheet = parse(css, &arena).expect("Failed to parse CSS");
/// let formatted = format(&stylesheet, css);
/// assert_eq!(formatted, "div {\n\tcolor: red;\n}\n");
/// ```
pub fn format(stylesheet: &CssStyleSheet<'_>, source: &str) -> String {
    printer::format_css(stylesheet, source)
}

/// Format into a caller-provided doc arena.
///
/// Identical output to [`format`], but the doc IR is built into `arena` instead
/// of a freshly allocated one, so a driver that formats many files can reuse one
/// arena across them (`arena.reset()` between files retains the buffers). Nothing
/// borrowed from `arena` escapes — the result is an owned `String`.
pub fn format_in(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    arena: &tsv_lang::doc::arena::DocArena,
) -> String {
    printer::format_css_in(stylesheet, source, arena)
}

/// Format a CSS stylesheet embedded in another language (e.g., Svelte).
///
/// Pass an [`EmbedContext`](tsv_lang::EmbedContext) with `base_indent_offset`
/// so wrapped lines respect the host's indentation.
///
/// # Arguments
/// * `stylesheet` - CSS stylesheet (nodes + value comments)
/// * `source` - The whole host document (spans are absolute)
/// * `line_breaks` - The host's whole-source line-break table
///   (`tsv_lang::printing::build_line_breaks(source)`) — the host printer
///   already holds one, so embedding doesn't rebuild it per `<style>` block
/// * `embed` - Embedding context (e.g., `base_indent_offset`)
///
/// # Example
/// ```
/// use tsv_css::{parse, format_embedded};
/// use tsv_lang::EmbedContext;
///
/// let css = "div{color:red;}";
/// let arena = bumpalo::Bump::new();
/// let stylesheet = parse(css, &arena).expect("Failed to parse CSS");
/// let line_breaks = tsv_lang::printing::build_line_breaks(css);
/// let embed = EmbedContext { base_indent_offset: 1, ..EmbedContext::default() };
/// let formatted = format_embedded(&stylesheet, css, &line_breaks, embed);
/// ```
pub fn format_embedded(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    line_breaks: &[u32],
    embed: tsv_lang::EmbedContext,
) -> String {
    printer::format_css_embedded(stylesheet, source, line_breaks, embed)
}

/// Format an embedded CSS stylesheet into a caller-provided doc arena.
///
/// Identical output to [`format_embedded`], but the doc IR is built into `arena`
/// rather than a freshly allocated one — so a Svelte host formatting a `<style>`
/// block can share its own document arena instead of allocating a second
/// whole-host-sized `DocArena` per block. Nothing borrowed from `arena` escapes
/// (the embedded CSS renders to an owned `String`), and the arena is **not**
/// reset, so the host's in-flight doc nodes stay valid across the call.
pub fn format_embedded_in(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    line_breaks: &[u32],
    embed: tsv_lang::EmbedContext,
    arena: &tsv_lang::doc::arena::DocArena,
) -> String {
    printer::format_css_embedded_in(stylesheet, source, line_breaks, embed, arena)
}

/// Convert CSS AST to JSON with character-based positions
///
/// Returns a `serde_json::Value` parsed from the wire bytes
/// `convert_ast_json_bytes` emits — a thin wrapper over the sole emission
/// path, not an independent conversion. Produces a standalone `StyleSheetFile`
/// JSON matching Svelte's `parseCss()` output (no `attributes` or `content`
/// fields, `end` set to full source length). Used where a `Value` is needed
/// (the CLI's `--pretty`, the fixture gate); byte-oriented consumers should
/// call `convert_ast_json_bytes` directly.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json(stylesheet: &CssStyleSheet<'_>, source: &str) -> serde_json::Value {
    serde_json::from_slice(&convert_ast_json_bytes(stylesheet, source))
        .expect("writer emits valid JSON")
}

/// Convert CSS AST to compact JSON wire bytes — the **sole emission path**
///
/// The writer (`ast/convert/write.rs`) walks the internal AST once and emits the
/// wire JSON directly, never materializing a typed public tree, fusing the
/// byte→char offset translation into the walk (each position through a
/// `ByteToCharMap`; identity on ASCII). The output is `parseCss()`'s JSON shape;
/// `convert_ast_json` parses these bytes back into a `Value`. The hot path for
/// the FFI parse binding and the CLI's compact output — the bytes are valid
/// UTF-8 by construction (source slices + ASCII fragments), and byte-oriented
/// consumers skip the O(output) validation a `String` requires.
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes(stylesheet: &CssStyleSheet<'_>, source: &str) -> Vec<u8> {
    ast::convert::write_stylesheet_file_bytes(stylesheet.nodes, source)
}

/// The `no-locations` variant, for parity with the TS/Svelte writers and the
/// uniform `lang_bindings!` macro. `parseCss` emits no per-node `loc`, so the
/// CSS wire already carries only `start`/`end` offsets — this is an exact alias
/// of `convert_ast_json_bytes` (a documented no-op, not a distinct shape).
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes_no_locations(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
) -> Vec<u8> {
    convert_ast_json_bytes(stylesheet, source)
}

/// Like `convert_ast_json_bytes`, as a `String` for `&str` boundaries (the
/// WASM binding's `JSON.parse`, N-API strings): same wire bytes plus one
/// UTF-8 validation of the output.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(stylesheet: &CssStyleSheet<'_>, source: &str) -> String {
    String::from_utf8(convert_ast_json_bytes(stylesheet, source))
        .expect("serde_json emits valid UTF-8")
}

/// The `String` form of `convert_ast_json_bytes_no_locations` (an alias — CSS
/// has no `loc`).
#[cfg(feature = "convert")]
pub fn convert_ast_json_string_no_locations(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
) -> String {
    convert_ast_json_string(stylesheet, source)
}

/// Drive the raw lexer over `source` and return a deterministic, line-per-token
/// dump (`<kind> <start>..<end> [text=…]`, terminated by `Eof` or `ERROR …`).
///
/// The differential gate for lexer changes: two lexer implementations are
/// token-identical iff this string matches for every corpus file. Behind the
/// `debug_lex` feature (off in production builds); used by `tsv_debug lex_diff`.
///
/// For `Identifier` tokens it prints the **resolved text** — the decoded value
/// when escapes were present, otherwise the verbatim source slice. That keeps the
/// golden invariant across the lazy-decode change: a no-escape identifier's
/// decoded value equals its source slice, so the stream is stable whether the
/// lexer allocates the decode eagerly or only when an escape is seen, while a
/// wrong escape decode still shows up as a divergent `text=`.
#[cfg(feature = "debug_lex")]
pub fn debug_token_stream(source: &str) -> String {
    use crate::lexer::{Lexer, TokenKind};
    use std::fmt::Write as _;

    let mut lexer = Lexer::new(source);
    let mut out = String::new();
    loop {
        match lexer.next_token() {
            Ok(token) => {
                let _ = write!(out, "{:?} {}..{}", token.kind, token.start, token.end);
                if matches!(token.kind, TokenKind::Identifier) {
                    let slice = &source[token.start as usize..token.end as usize];
                    let decoded = lexer.take_decoded();
                    let text = decoded.as_deref().map_or(slice, String::as_str);
                    let _ = write!(out, " text={text:?}");
                }
                out.push('\n');
                if matches!(token.kind, TokenKind::Eof) {
                    break;
                }
            }
            Err(err) => {
                let _ = writeln!(out, "ERROR {err:?}");
                break;
            }
        }
    }
    out
}
