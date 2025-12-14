// tsv_css - CSS parsing and formatting library
//
// Provides CSS parsing, formatting, and AST conversion functionality.
// Part of the tsv (TypeScript and Svelte tools in Rust) project.

pub mod ast;
pub mod escapes;
pub mod lexer;
pub mod parser;
pub mod printer;

// Re-export commonly used types and functions
pub use ast::{CssDeclaration, CssNode, CssRule, CssStyleSheet, StyleContent, StyleSheet};
pub use parser::parse_css;
pub use printer::{Printer, format_css};
pub use tsv_lang::{ParseError, PrintConfig, Result, Span};

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
    parse_css(source, 0).map_err(|e| e.with_context(source))
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
    parse_css(source, base_offset).map_err(|e| e.with_context(source))
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
    format_css(stylesheet, source)
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
/// use tsv_lang::PrintConfig;
///
/// let css = "div{color:red;}";
/// let stylesheet = parse(css).expect("Failed to parse CSS");
/// let config = PrintConfig { base_indent_offset: 1, ..Default::default() };
/// let formatted = format_with_config(&stylesheet, css, config);
/// ```
pub fn format_with_config(stylesheet: &CssStyleSheet, source: &str, config: PrintConfig) -> String {
    printer::format_css_with_config(stylesheet, source, config)
}

/// Convert CSS AST to JSON representation
///
/// # Arguments
/// * `stylesheet` - CSS stylesheet (nodes + value comments)
/// * `source` - Original CSS source code
///
/// # Returns
/// * JSON value representing the StyleSheet
///
/// # Example
/// ```
/// use tsv_css::{parse, convert_ast};
///
/// let css = "div { color: red; }";
/// let stylesheet = parse(css).expect("Failed to parse CSS");
/// let json = convert_ast(&stylesheet, css);
/// ```
pub fn convert_ast(stylesheet: &CssStyleSheet, source: &str) -> serde_json::Value {
    ast::convert::convert_css_nodes(&stylesheet.nodes, source)
}
