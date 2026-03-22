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
    let arena =
        tsv_lang::doc::arena::DocArena::with_source_size_hint(source.len(), config.tab_width);
    let mut printer = printer::Printer::with_config(
        &arena,
        Rc::clone(&program.interner),
        source,
        &program.comments,
        &program.line_breaks,
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

/// Convert internal AST to JSON with character-based positions
///
/// Like `convert_ast`, but returns `serde_json::Value` with all byte-based
/// positions (`start`, `end`, `loc.*.column`) translated to Unicode character
/// offsets to match acorn output.
///
/// This is the preferred function for producing JSON AST output.
#[allow(clippy::expect_used)]
pub fn convert_ast_json(program: &Program, source: &str) -> serde_json::Value {
    let tracker = tsv_lang::LocationTracker::new(source);
    let public_ast = ast::convert::convert_program(program, source, &tracker);
    let mut json = serde_json::to_value(&public_ast).expect("AST types derive Serialize correctly");
    let map = tsv_lang::ByteToCharMap::new(source);
    ast::convert::translate_byte_to_char_offsets(&mut json, &map, &tracker);
    json
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

/// Parse a single TypeScript expression and return it with any comments.
///
/// This is used when parsing expressions in contexts where comments need to be
/// preserved (e.g., Svelte expression tags `{/* comment */ expr}`).
pub fn parse_expression_with_comments(
    source: &str,
    base_offset: usize,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> Result<(Expression, Vec<ast::Comment>)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser
        .parse_expression_with_comments()
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
    line_breaks: &[u32],
) -> String {
    format_expression_with_indent(expression, source, interner, 0, line_breaks)
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
    line_breaks: &[u32],
) -> String {
    let comments = Vec::new();
    let config = tsv_lang::PrintConfig::default();
    let arena =
        tsv_lang::doc::arena::DocArena::with_source_size_hint(source.len(), config.tab_width);
    let mut printer =
        printer::Printer::with_config(&arena, interner, source, &comments, line_breaks, config);
    printer.set_indent_level(indent_level);
    printer.print_expression(expression);
    printer.into_string()
}

/// Format a single TypeScript expression with a base indentation level and comments
///
/// This is used when formatting expressions embedded in other content
/// (e.g., Svelte templates) where the expression needs to respect the
/// surrounding indentation context, and comments need to be preserved.
pub fn format_expression_with_indent_and_comments(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    indent_level: usize,
    comments: &[ast::Comment],
    line_breaks: &[u32],
) -> String {
    let config = tsv_lang::PrintConfig::default();
    let arena =
        tsv_lang::doc::arena::DocArena::with_source_size_hint(source.len(), config.tab_width);
    let mut printer =
        printer::Printer::with_config(&arena, interner, source, comments, line_breaks, config);
    printer.set_indent_level(indent_level);
    printer.print_expression(expression);
    printer.into_string()
}

/// Format a single TypeScript expression with comments.
///
/// Preserves comments that were collected during parsing.
/// Used for Svelte expression tags that may contain comments.
pub fn format_expression_with_comments(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    comments: &[ast::Comment],
    line_breaks: &[u32],
) -> String {
    let config = tsv_lang::PrintConfig::default();
    let arena =
        tsv_lang::doc::arena::DocArena::with_source_size_hint(source.len(), config.tab_width);
    let mut printer =
        printer::Printer::with_config(&arena, interner, source, comments, line_breaks, config);
    printer.print_expression(expression);
    printer.into_string()
}

/// Format a single TypeScript expression with custom print configuration.
///
/// Like `format_expression_with_comments`, but accepts a PrintConfig
/// to control formatting behavior. Use `first_line_offset` to account for
/// expressions that start mid-line (e.g., `{#each expr as item}`).
pub fn format_expression_with_config(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    comments: &[ast::Comment],
    config: tsv_lang::PrintConfig,
    line_breaks: &[u32],
) -> String {
    let arena =
        tsv_lang::doc::arena::DocArena::with_source_size_hint(source.len(), config.tab_width);
    let mut printer =
        printer::Printer::with_config(&arena, interner, source, comments, line_breaks, config);
    // Set indent level from base_indent_offset so wrapped lines (e.g., method chains)
    // are indented relative to the outer context (e.g., Svelte block directives)
    printer.set_indent_level(config.base_indent_offset);
    printer.print_expression(expression);
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

/// Parse a pattern and return it with any collected comments.
///
/// Like `parse_pattern`, but also returns comments for preservation
/// in Svelte template contexts.
pub fn parse_pattern_with_comments(
    source: &str,
    base_offset: usize,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> Result<(Expression, Vec<ast::Comment>)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    let expr = parser
        .parse_expression_public()
        .map_err(|e| e.with_context(source))?;
    let pattern = parser
        .expression_to_pattern(expr)
        .map_err(|e| e.with_context(source))?;
    let comments = parser.take_comments();
    Ok((pattern, comments))
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

/// Parse a partial expression and return it with any collected comments.
///
/// Like `parse_expression_partial`, but also returns comments for preservation
/// in Svelte template contexts.
pub fn parse_expression_partial_with_comments(
    source: &str,
    base_offset: usize,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
) -> Result<(Expression, usize, Vec<ast::Comment>)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    let (expr, end_pos) = parser
        .parse_assignment_expression_partial()
        .map_err(|e| e.with_context(source))?;
    let comments = parser.take_comments();
    Ok((expr, end_pos, comments))
}

/// Build a DocId for a TypeScript expression with comments in the caller's arena.
///
/// Pass `&[]` for comments when no comments need to be preserved.
pub fn build_expression_doc_with_comments(
    arena: &tsv_lang::doc::arena::DocArena,
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    config: &tsv_lang::PrintConfig,
    comments: &[ast::Comment],
    line_breaks: &[u32],
) -> tsv_lang::doc::arena::DocId {
    let printer =
        printer::Printer::with_config(arena, interner, source, comments, line_breaks, *config);
    printer.build_expression_doc_public(expression)
}

/// Build a DocId for a TypeScript program in the caller's arena.
///
/// Returns a DocId that can be rendered with the arena.
/// Used when embedding TypeScript in other formats like Svelte's `<script>`.
pub fn build_program_doc(
    arena: &tsv_lang::doc::arena::DocArena,
    program: &Program,
    source: &str,
    config: tsv_lang::PrintConfig,
) -> tsv_lang::doc::arena::DocId {
    let printer = printer::Printer::with_config(
        arena,
        Rc::clone(&program.interner),
        source,
        &program.comments,
        &program.line_breaks,
        config,
    );
    printer.build_program_doc(program)
}

// Re-export key types for convenience
pub use ast::internal::{
    ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression,
    AssignmentOperator, AssignmentPattern, Expression, Identifier, Literal, LiteralValue,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, Program, Property, RestElement,
    SpreadElement, Statement, TSKeywordKind, TSKeywordType, TSType, TSTypeAnnotation,
    VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};
