// tsv_css - CSS parsing and formatting library
//
// Provides CSS parsing, formatting, and AST conversion functionality.
// Part of the tsv (TypeScript and Svelte tools in Rust) project.

pub mod ast;
pub mod formatter;
pub mod lexer;
pub mod parser;

// Re-export commonly used types and functions
pub use ast::{CssDeclaration, CssNode, CssRule, StyleContent, StyleSheet};
pub use formatter::{FormatConfig, Formatter, format_css};
pub use parser::parse_css;
pub use tsv_lang::{ParseError, Result, Span};

/// Parse CSS source into internal AST
///
/// # Arguments
/// * `source` - CSS source code
/// * `base_offset` - Offset in larger file (for embedded CSS in Svelte)
///
/// # Returns
/// * `Ok(Vec<CssNode>)` - Parsed AST nodes
/// * `Err(ParseError)` - Parse error with position and context
///
/// # Example
/// ```
/// use tsv_css::parse;
///
/// let css = "div { color: red; }";
/// let nodes = parse(css, 0).expect("Failed to parse CSS");
/// ```
pub fn parse(source: &str, base_offset: usize) -> Result<Vec<CssNode>> {
    parse_css(source, base_offset)
}

/// Format CSS nodes to a formatted string
///
/// # Arguments
/// * `nodes` - CSS AST nodes to format
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
/// let nodes = parse(css, 0).expect("Failed to parse CSS");
/// let formatted = format(&nodes, css);
/// assert_eq!(formatted, "div {\n\tcolor: red;\n}\n");
/// ```
pub fn format(nodes: &[CssNode], source: &str) -> String {
    formatter::format_css_with_source(nodes, source)
}

/// Convert CSS AST nodes to JSON representation
///
/// # Arguments
/// * `nodes` - CSS AST nodes to convert
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
/// let nodes = parse(css, 0).expect("Failed to parse CSS");
/// let json = convert_ast(&nodes, css);
/// ```
pub fn convert_ast(nodes: &[CssNode], source: &str) -> serde_json::Value {
    ast::convert::convert_css_nodes(nodes, source)
}
