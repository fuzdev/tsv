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
pub mod escapes;
mod lexer;
pub(crate) mod parser;
mod printer;

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
pub fn parse(source: &str) -> Result<Program> {
    parser::parse_typescript(source).map_err(|e| e.with_context(source))
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
/// let source = "const x=42;";
/// let ast = tsv_ts::parse(source)?;
/// let formatted = tsv_ts::format(&ast, source);
/// assert_eq!(formatted, "const x = 42;\n");
/// ```
pub fn format(program: &Program, source: &str) -> String {
    format_with_config(program, source, tsv_lang::PrintConfig::default())
}

/// Format an internal AST back to source code with custom configuration
///
/// This allows specifying print configuration like `base_indent_offset` for
/// when TypeScript is embedded inside another format (e.g., Svelte `<script>` tags).
pub fn format_with_config(
    program: &Program,
    source: &str,
    config: tsv_lang::PrintConfig,
) -> String {
    let mut printer = printer::Printer::with_config(
        Rc::clone(&program.interner),
        source,
        &program.comments,
        config,
    );
    printer.print_program(program);
    printer.into_string()
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
pub fn convert_ast(program: &Program, source: &str) -> ast::public::Program {
    let tracker = tsv_lang::LocationTracker::new(source);
    ast::convert::convert_program(program, source, &tracker)
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
) -> Result<Program> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser.parse().map_err(|e| e.with_context(source))
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
) -> Result<Expression> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser
        .parse_expression_public()
        .map_err(|e| e.with_context(source))
}

/// Parse a partial expression, stopping at top-level commas.
///
/// This is used when parsing patterns in contexts where commas have other meanings,
/// such as `{#each items as pattern, index}` where the comma separates the pattern
/// from the index variable.
///
/// Unlike `parse_expression`, this uses assignment expression parsing which stops
/// at top-level commas (but handles commas inside objects/arrays/calls correctly).
///
/// # Arguments
///
/// * `source` - The source code starting at the expression
/// * `base_offset` - Offset in the full source file
/// * `interner` - Shared string interner
///
/// # Returns
///
/// * `Ok((Expression, usize))` - The parsed expression and the absolute position
///   where parsing stopped (start of next token)
/// * `Err(ParseError)` - If parsing fails
pub fn parse_expression_partial(
    source: &str,
    base_offset: usize,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> Result<(Expression, usize)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser
        .parse_assignment_expression_partial()
        .map_err(|e| e.with_context(source))
}

// Re-export key types for convenience
pub use ast::internal::{
    ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AssignmentPattern, Expression, Identifier, Literal, LiteralValue,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, Program, Property, RestElement,
    SpreadElement, Statement, TSKeywordKind, TSKeywordType, TSType, TSTypeAnnotation,
    VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};
