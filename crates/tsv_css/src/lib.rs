//! CSS parsing and formatting library
//!
//! Provides CSS parsing, formatting, and AST conversion functionality.
//! Part of the tsv (a formatter, parser, and future linter + more for Svelte, TypeScript, and CSS) project.

pub mod ast;
mod escapes;
mod lexer;
mod number;
mod parser;
mod printer;
mod url;
mod whitespace;

// Re-export commonly used types
pub use ast::{CssDeclaration, CssNode, CssRule, CssStyleSheet};
#[cfg(feature = "convert")]
pub use ast::{StyleContent, StyleSheet};
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
/// * `source` - Original CSS source code (for blank line preservation)
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
/// let embed = EmbedContext { base_indent_offset: 1, ..EmbedContext::default() };
/// let formatted = format_embedded(&stylesheet, css, embed);
/// ```
pub fn format_embedded(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    embed: tsv_lang::EmbedContext,
) -> String {
    printer::format_css_embedded(stylesheet, source, embed)
}

/// Convert CSS AST to public JSON-compatible AST
///
/// # Arguments
/// * `stylesheet` - CSS stylesheet (nodes + value comments)
/// * `source` - Original CSS source code
///
/// # Returns
/// A public AST that can be serialized to JSON
///
/// # Example
/// ```
/// use tsv_css::{parse, convert_ast};
///
/// let css = "div { color: red; }";
/// let arena = bumpalo::Bump::new();
/// let stylesheet = parse(css, &arena).expect("Failed to parse CSS");
/// let public_ast = convert_ast(&stylesheet, css);
/// let json = serde_json::to_string_pretty(&public_ast).unwrap();
/// ```
#[cfg(feature = "convert")]
pub fn convert_ast<'src>(stylesheet: &CssStyleSheet<'_>, source: &'src str) -> StyleSheet<'src> {
    ast::convert::convert_css_nodes(stylesheet.nodes, source)
}

/// Convert CSS AST to JSON with character-based positions
///
/// Like `convert_ast`, but returns `serde_json::Value` with all byte-based
/// positions (`start`, `end`) translated to Unicode character offsets.
///
/// Produces a standalone `StyleSheetFile` JSON matching Svelte's `parseCss()` output
/// (no `attributes` or `content` fields, `end` set to full source length).
///
/// Builds the typed public AST (`convert_stylesheet_file`), then materializes it
/// into a `Value` and translates positions there — the `Value` walk stays the
/// independent oracle the fixture suite checks `convert_ast_json_string`'s typed
/// walk against.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json(stylesheet: &CssStyleSheet<'_>, source: &str) -> serde_json::Value {
    let public_ast = ast::convert::convert_stylesheet_file(stylesheet.nodes, source);
    let mut json = serde_json::to_value(&public_ast).expect("public CSS AST serializes to a Value");
    let map = tsv_lang::ByteToCharMap::new(source);
    ast::convert::translate_byte_to_char_offsets(&mut json, &map);
    json
}

/// Like `convert_ast_json`, serialized to compact JSON wire bytes
///
/// Emits the wire JSON directly from the internal AST via the writer
/// (`ast/convert/write.rs`), never materializing the typed public tree, and
/// fuses the byte→char offset translation into that walk (each position through
/// a `ByteToCharMap`; identity on ASCII). The writer is a third emission mode of
/// the same `parseCss()` quirk catalog — every `write_*` mirrors its `convert_*`
/// twin and reuses its raw-source reconstruction helpers, so they stay in
/// lockstep (gated by the fixture suite's string-path identity check against the
/// `Value` oracle and `corpus:compare:parse --multibyte-only`). Byte-identical
/// to `serde_json::to_string(&convert_ast_json(...))`; the hot path for the FFI
/// parse binding and the CLI's compact output — the bytes are valid UTF-8 by
/// construction (source slices + ASCII fragments), and byte-oriented consumers
/// skip the O(output) validation a `String` requires.
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes(stylesheet: &CssStyleSheet<'_>, source: &str) -> Vec<u8> {
    ast::convert::write_stylesheet_file_bytes(stylesheet.nodes, source)
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
