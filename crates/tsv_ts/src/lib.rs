//! TypeScript parsing and formatting library
//!
//! This crate provides TypeScript AST parsing and code formatting.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use tsv_ts::{parse, format, convert_ast};
//!
//! // Parse TypeScript code
//! let source = "const x: number = 42;";
//! let ast = parse(source)?;
//!
//! // Format TypeScript code
//! let formatted = format(&ast);
//!
//! // Convert to JSON AST
//! let json_ast = convert_ast(&ast, source);
//! ```

pub mod ast;
mod formatter;
mod formatter_core;
mod lexer;
pub(crate) mod parser;

use std::cell::RefCell;
use std::rc::Rc;

pub use tsv_lang::{ParseError, Result};

/// Parse TypeScript source code into an internal AST
///
/// # Arguments
///
/// * `source` - The TypeScript source code to parse
///
/// # Returns
///
/// * `Ok(Program)` - The parsed AST
/// * `Err(ParseError)` - If parsing fails
///
/// # Example
///
/// ```rust,ignore
/// let ast = tsv_ts::parse("const x = 42;")?;
/// ```
pub fn parse(source: &str) -> Result<ast::internal::Program> {
    parser::parse_typescript(source)
}

/// Format a TypeScript AST back to source code
///
/// # Arguments
///
/// * `program` - The TypeScript AST to format
///
/// # Returns
///
/// The formatted TypeScript source code as a String
///
/// # Example
///
/// ```rust,ignore
/// let ast = tsv_ts::parse("const x=42;")?;
/// let formatted = tsv_ts::format(&ast);
/// assert_eq!(formatted, "const x = 42;\n");
/// ```
pub fn format(program: &ast::internal::Program) -> String {
    let mut formatter = formatter_core::Formatter::new(program.interner.clone());
    formatter.format_program(program);
    formatter.into_string()
}

/// Convert internal AST to public JSON-compatible AST
///
/// # Arguments
///
/// * `program` - The internal AST to convert
/// * `source` - The original source code (for location tracking)
///
/// # Returns
///
/// A public AST that can be serialized to JSON
///
/// # Example
///
/// ```rust,ignore
/// let source = "const x: number = 42;";
/// let ast = tsv_ts::parse(source)?;
/// let public_ast = tsv_ts::convert_ast(&ast, source);
/// let json = serde_json::to_string_pretty(&public_ast)?;
/// ```
pub fn convert_ast(program: &ast::internal::Program, source: &str) -> ast::public::Program {
    let tracker = tsv_lang::LocationTracker::new(source);
    ast::convert::convert_program(program, &tracker)
}

/// Parse TypeScript with a shared string interner and base offset
///
/// This is used when parsing embedded TypeScript in Svelte files.
///
/// # Arguments
///
/// * `source` - The TypeScript source code to parse
/// * `base_offset` - Offset in the full source file
/// * `interner` - Shared string interner
///
/// # Returns
///
/// * `Ok(Program)` - The parsed AST
/// * `Err(ParseError)` - If parsing fails
pub fn parse_with_interner(
    source: &str,
    base_offset: usize,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> Result<ast::internal::Program> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser.parse()
}

/// Parse a single TypeScript expression
///
/// This is used when parsing embedded expressions in Svelte templates.
///
/// # Arguments
///
/// * `source` - The TypeScript expression source code
/// * `base_offset` - Offset in the full source file
/// * `interner` - Shared string interner
///
/// # Returns
///
/// * `Ok(Expression)` - The parsed expression
/// * `Err(ParseError)` - If parsing fails
pub fn parse_expression(
    source: &str,
    base_offset: usize,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> Result<ast::internal::Expression> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser.parse_expression_public()
}

// Re-export key types for convenience
pub use ast::internal::{
    Expression, Identifier, Literal, LiteralValue, Program, Statement, TSNumberKeyword, TSType,
    TSTypeAnnotation, VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};
