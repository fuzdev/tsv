//! Svelte parsing and formatting library
//!
//! This crate provides Svelte component parsing and code formatting.

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod printer;

pub use tsv_lang::{ParseError, Result};

/// Parse Svelte source code into an internal AST
///
/// # Arguments
///
/// * `source` - The Svelte component source code to parse
///
/// # Returns
///
/// * `Ok(Root)` - The parsed AST
/// * `Err(ParseError)` - If parsing fails
///
/// # Example
///
/// ```rust,ignore
/// let ast = tsv_svelte::parse("<div>Hello</div>")?;
/// ```
pub fn parse(source: &str) -> Result<Root> {
    parser::parse_svelte(source).map_err(|e| e.with_context(source))
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
/// let ast = tsv_svelte::parse(source)?;
/// let formatted = tsv_svelte::format(&ast, source);
/// ```
pub fn format(root: &Root, source: &str) -> String {
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
/// let ast = tsv_svelte::parse(source)?;
/// let public_ast = tsv_svelte::convert_ast(&ast, source);
/// let json = serde_json::to_string_pretty(&public_ast)?;
/// ```
pub fn convert_ast(root: &Root, source: &str) -> ast::public::Root {
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
/// let ast = tsv_svelte::parse(source)?;
/// let json = tsv_svelte::convert_ast_json(&ast, source);
/// ```
#[allow(clippy::expect_used)]
pub fn convert_ast_json(root: &Root, source: &str) -> serde_json::Value {
    let public_ast = ast::convert::convert_root(root, source);
    let mut json = serde_json::to_value(&public_ast).expect("AST types derive Serialize correctly");

    // Attach comments to template expressions (outside <script> tags)
    // Must happen before byte→char translation since comment positions are byte-based
    let mut script_spans: Vec<(u32, u32)> = Vec::new();
    if let Some(ref script) = root.instance {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
    if let Some(ref script) = root.module {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
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

// Re-export commonly used types
pub use ast::{
    Attribute, AttributeValue, Element, ExpressionTag, Fragment, FragmentNode, Root, Script,
    ScriptContext, Style, Text,
};
