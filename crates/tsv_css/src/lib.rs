//! CSS parsing and formatting library
//!
//! Provides CSS parsing, formatting, and AST conversion functionality.
//! Part of the tsv (a formatter, parser, and future linter + more for TypeScript, Svelte, and CSS) project.

pub mod ast;
mod escapes;
mod lexer;
mod number;
mod parser;
mod printer;
mod url;

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
/// let stylesheet = parse(css).expect("Failed to parse CSS");
/// ```
pub fn parse(source: &str) -> Result<CssStyleSheet> {
    parser::parse_css(source, 0).map_err(|e| e.with_context(source))
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
pub fn parse_embedded(source: &str, base_offset: usize) -> Result<CssStyleSheet> {
    parser::parse_css(source, base_offset).map_err(|e| e.with_context(source))
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
/// let stylesheet = parse(css).expect("Failed to parse CSS");
/// let formatted = format(&stylesheet, css);
/// assert_eq!(formatted, "div {\n\tcolor: red;\n}\n");
/// ```
pub fn format(stylesheet: &CssStyleSheet, source: &str) -> String {
    printer::format_css(stylesheet, source)
}

/// Format CSS stylesheet with custom configuration
///
/// Use this when CSS is nested inside another language (e.g., Svelte)
/// with base_indent_offset to account for wrapper indentation.
///
/// # Arguments
/// * `stylesheet` - CSS stylesheet (nodes + value comments)
/// * `source` - Original CSS source code (for blank line preservation)
/// * `config` - Print configuration with optional base_indent_offset
///
/// # Example
/// ```
/// use tsv_css::{parse, format_with_config};
/// use tsv_lang::{EmbedContext, PrintConfig};
///
/// let css = "div{color:red;}";
/// let stylesheet = parse(css).expect("Failed to parse CSS");
/// let embed = EmbedContext { base_indent_offset: 1, ..EmbedContext::default() };
/// let formatted = format_with_config(&stylesheet, css, PrintConfig::default(), embed);
/// ```
pub fn format_with_config(
    stylesheet: &CssStyleSheet,
    source: &str,
    config: tsv_lang::PrintConfig,
    embed: tsv_lang::EmbedContext,
) -> String {
    printer::format_css_with_config(stylesheet, source, config, embed)
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
/// let stylesheet = parse(css).expect("Failed to parse CSS");
/// let public_ast = convert_ast(&stylesheet, css);
/// let json = serde_json::to_string_pretty(&public_ast).unwrap();
/// ```
#[cfg(feature = "convert")]
pub fn convert_ast(stylesheet: &CssStyleSheet, source: &str) -> StyleSheet {
    ast::convert::convert_css_nodes(&stylesheet.nodes, source)
}

/// Convert CSS AST to JSON with character-based positions
///
/// Like `convert_ast`, but returns `serde_json::Value` with all byte-based
/// positions (`start`, `end`) translated to Unicode character offsets.
///
/// Produces a standalone `StyleSheetFile` JSON matching Svelte's `parseCss()` output
/// (no `attributes` or `content` fields, `end` set to full source length).
///
/// This is the preferred function for producing JSON AST output.
#[cfg(feature = "convert")]
pub fn convert_ast_json(stylesheet: &CssStyleSheet, source: &str) -> serde_json::Value {
    let mut json = ast::convert::convert_css_nodes_standalone(&stylesheet.nodes, source);
    let map = tsv_lang::ByteToCharMap::new(source);
    ast::convert::translate_byte_to_char_offsets(&mut json, &map);
    json
}

/// Like `convert_ast_json`, serialized to a compact JSON string
///
/// CSS conversion builds the `Value` directly (no typed public-AST tree), so
/// unlike `tsv_ts`/`tsv_svelte` there is no direct-serialization fast path
/// here. This exists so the FFI/WASM bindings have one uniform
/// string-returning entry point per language.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(stylesheet: &CssStyleSheet, source: &str) -> String {
    let mut buf = Vec::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    serde_json::to_writer(&mut buf, &convert_ast_json(stylesheet, source))
        .expect("Value serialization cannot fail");
    String::from_utf8(buf).expect("serde_json emits valid UTF-8")
}
