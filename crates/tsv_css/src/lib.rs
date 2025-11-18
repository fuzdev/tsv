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
/// * `base_offset` - Offset in larger file (for embedded CSS in Svelte)
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
/// let stylesheet = parse(css, 0).expect("Failed to parse CSS");
/// ```
pub fn parse(source: &str, base_offset: usize) -> Result<CssStyleSheet> {
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
/// let stylesheet = parse(css, 0).expect("Failed to parse CSS");
/// let formatted = format(&stylesheet, css);
/// assert_eq!(formatted, "div {\n\tcolor: red;\n}\n");
/// ```
pub fn format(stylesheet: &CssStyleSheet, source: &str) -> String {
    printer::format_css(stylesheet, source)
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
/// let stylesheet = parse(css, 0).expect("Failed to parse CSS");
/// let json = convert_ast(&stylesheet, css);
/// ```
pub fn convert_ast(stylesheet: &CssStyleSheet, source: &str) -> serde_json::Value {
    ast::convert::convert_css_nodes(&stylesheet.nodes, source)
}
