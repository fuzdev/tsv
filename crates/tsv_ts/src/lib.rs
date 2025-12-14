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
pub mod lexer;
pub mod parser;
pub mod printer;

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

/// Format a single TypeScript expression back to source code
///
/// This formats an expression AST node that was parsed as part of a larger document
/// (e.g., a Svelte template). The source must be the full document source that the
/// expression's spans refer to.
///
/// # Arguments
///
/// * `expression` - The expression AST to format
/// * `source` - The original full source code (the expression's spans index into this)
/// * `interner` - Shared string interner (same one used during parsing)
///
/// # Returns
///
/// The formatted expression as a String
pub fn format_expression(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> String {
    format_expression_with_indent(expression, source, interner, 0)
}

/// Format a single TypeScript expression with a base indentation level
///
/// This is used when formatting expressions embedded in other content
/// (e.g., Svelte templates) where the expression needs to respect the
/// surrounding indentation context.
pub fn format_expression_with_indent(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    indent_level: usize,
) -> String {
    let comments = Vec::new();
    let mut printer = printer::Printer::with_config(
        interner,
        source,
        &comments,
        tsv_lang::PrintConfig::default(),
    );
    printer.set_indent_level(indent_level);
    printer.print_expression(expression);
    printer.into_string()
}

/// Format a single TypeScript expression in an isolated context.
///
/// Similar to `format_expression`, but handles sequence expressions specially:
/// they are NOT wrapped in parentheses since the surrounding context (like
/// Svelte's `={...}`) already provides the necessary grouping.
pub fn format_expression_isolated(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> String {
    let comments = Vec::new();
    let mut printer = printer::Printer::with_config(
        interner,
        source,
        &comments,
        tsv_lang::PrintConfig::default(),
    );
    printer.print_expression_isolated(expression);
    printer.into_string()
}

/// Parse an expression and convert it to a binding pattern.
///
/// This parses an expression and then converts it to a pattern:
/// - ObjectExpression → ObjectPattern
/// - ArrayExpression → ArrayPattern
/// - SpreadElement → RestElement
/// - AssignmentExpression → AssignmentPattern
/// - Identifier → Identifier (unchanged)
///
/// Used for parsing destructuring patterns in contexts like `@const {a, b} = expr`.
///
/// # Arguments
///
/// * `source` - The source code of the pattern
/// * `base_offset` - Offset in the full source file
/// * `interner` - Shared string interner
///
/// # Returns
///
/// * `Ok(Expression)` - The parsed pattern (ObjectPattern, ArrayPattern, etc.)
/// * `Err(ParseError)` - If parsing or conversion fails
pub fn parse_pattern(
    source: &str,
    base_offset: usize,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> Result<Expression> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    let expr = parser
        .parse_expression_public()
        .map_err(|e| e.with_context(source))?;
    parser
        .expression_to_pattern(expr)
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

/// Build a Doc tree for a TypeScript expression
///
/// This returns a Doc that can be used in the doc-based formatting system
/// for proper line wrapping decisions. Unlike `format_expression` which returns
/// a formatted string, this preserves break points and wrapping information.
///
/// Used when embedding TS expressions in larger documents (e.g., Svelte attributes)
/// where the surrounding context needs to participate in wrapping decisions.
pub fn build_expression_doc(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    config: &tsv_lang::PrintConfig,
) -> tsv_lang::doc::Doc {
    let comments = Vec::new();
    let printer = printer::Printer::with_config(interner, source, &comments, *config);
    printer.build_expression_doc_public(expression)
}

/// Build a Doc tree for a TypeScript expression in an isolated context.
///
/// Similar to `build_expression_doc`, but handles sequence expressions specially:
/// they are NOT wrapped in parentheses since the surrounding context (like
/// Svelte's `={...}`) already provides the necessary grouping.
///
/// Use this when the expression is inside braces or other grouping syntax
/// where the outer delimiter already disambiguates the comma operator.
pub fn build_expression_doc_isolated(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    config: &tsv_lang::PrintConfig,
) -> tsv_lang::doc::Doc {
    let comments = Vec::new();
    let printer = printer::Printer::with_config(interner, source, &comments, *config);
    printer.build_expression_doc_isolated_public(expression)
}

// Re-export key types for convenience
pub use ast::internal::{
    ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AssignmentPattern, Expression, Identifier, Literal, LiteralValue,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, Program, Property, RestElement,
    SpreadElement, Statement, TSKeywordKind, TSKeywordType, TSType, TSTypeAnnotation,
    VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};
