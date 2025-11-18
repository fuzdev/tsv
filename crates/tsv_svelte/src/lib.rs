//! Svelte parsing and formatting library
//!
//! This crate provides Svelte component parsing and code formatting.

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod printer;

pub use tsv_lang::{ParseError, Result};

/// Parse Svelte source code into an internal AST
pub fn parse(source: &str) -> Result<ast::Root> {
    parser::parse_svelte(source).map_err(|e| e.with_context(source))
}

/// Format a Svelte AST back to source code
pub fn format(root: &ast::Root, source: &str) -> String {
    printer::format_svelte(root, source)
}

/// Convert internal AST to public JSON-compatible AST
pub fn convert_ast(root: &ast::Root, source: &str) -> ast::public::Root {
    ast::convert::convert_root(root, source)
}

// Re-export commonly used types
pub use ast::{
    Attribute, AttributeValue, Element, ExpressionTag, Fragment, FragmentNode, Root, Script,
    ScriptContext, Style, Text,
};
