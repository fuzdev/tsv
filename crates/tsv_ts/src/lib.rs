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
        &program.line_breaks,
        config,
    );
    printer.print_program(program);
    printer.into_string()
}

/// Format using Doc-based formatting (validation entry point)
///
/// This is a parallel implementation that uses Doc IR for all statement formatting.
/// Used during migration to validate doc-based output matches imperative output.
pub fn format_with_doc(program: &Program, source: &str) -> String {
    format_with_doc_config(program, source, tsv_lang::PrintConfig::default())
}

/// Format using Doc-based formatting with custom configuration
pub fn format_with_doc_config(
    program: &Program,
    source: &str,
    config: tsv_lang::PrintConfig,
) -> String {
    let mut printer = printer::Printer::with_config(
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
    let mut printer = printer::Printer::with_config(
        interner,
        source,
        &comments,
        line_breaks,
        tsv_lang::PrintConfig::default(),
    );
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
    let mut printer = printer::Printer::with_config(
        interner,
        source,
        comments,
        line_breaks,
        tsv_lang::PrintConfig::default(),
    );
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
    let mut printer = printer::Printer::with_config(
        interner,
        source,
        comments,
        line_breaks,
        tsv_lang::PrintConfig::default(),
    );
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
    let mut printer =
        printer::Printer::with_config(interner, source, comments, line_breaks, config);
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
    line_breaks: &[u32],
) -> tsv_lang::doc::Doc {
    let comments = Vec::new();
    let printer = printer::Printer::with_config(interner, source, &comments, line_breaks, *config);
    printer.build_expression_doc_public(expression)
}

/// Build a Doc for a TypeScript expression with comments.
///
/// Like `build_expression_doc`, but includes comments collected during parsing.
pub fn build_expression_doc_with_comments(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    config: &tsv_lang::PrintConfig,
    comments: &[ast::Comment],
    line_breaks: &[u32],
) -> tsv_lang::doc::Doc {
    let printer = printer::Printer::with_config(interner, source, comments, line_breaks, *config);
    printer.build_expression_doc_public(expression)
}

/// Build a Doc for a TypeScript expression with continuation indent for binary expressions.
///
/// When a binary expression breaks, continuation lines are indented relative to the first:
/// ```text
/// first &&
///   second &&
///   third
/// ```
///
/// This is used in attribute contexts (like Svelte's `={...}`) where prettier uses
/// this specific indentation style for binary expressions.
pub fn build_expression_doc_with_continuation_indent(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    config: &tsv_lang::PrintConfig,
    comments: &[ast::Comment],
    line_breaks: &[u32],
) -> tsv_lang::doc::Doc {
    let printer = printer::Printer::with_config(interner, source, comments, line_breaks, *config);
    printer.build_expression_doc_with_continuation_indent_public(expression)
}

/// Build a Doc for a condition expression (if/while test) with proper wrapping.
///
/// For binary expressions (&&, ||), uses an ungrouped version so the parent group
/// controls whether the condition breaks to multiple lines. This allows conditions
/// like `{#if a && b && c}` to wrap when they exceed print width.
///
/// # Arguments
///
/// * `expression` - The condition expression
/// * `source` - The source code
/// * `interner` - Shared string interner
/// * `config` - Print configuration
/// * `comments` - Comments from parsing
///
/// # Returns
///
/// A Doc that can be wrapped by a parent group for width-aware formatting.
pub fn build_condition_doc(
    expression: &Expression,
    source: &str,
    interner: Rc<RefCell<string_interner::DefaultStringInterner>>,
    config: &tsv_lang::PrintConfig,
    comments: &[ast::Comment],
    line_breaks: &[u32],
) -> tsv_lang::doc::Doc {
    let printer = printer::Printer::with_config(interner, source, comments, line_breaks, *config);
    printer.build_condition_doc_public(expression)
}

/// Build a Doc tree for a TypeScript program
///
/// Returns a Doc that can be wrapped with `indent()` and rendered.
/// Used when embedding TypeScript in other formats like Svelte's `<script>`.
///
/// # Arguments
///
/// * `program` - The TypeScript AST to format
/// * `source` - The original source code
/// * `config` - Print configuration (use `base_indent_offset` to account for outer context)
///
/// # Example
///
/// ```rust,ignore
/// // Format TypeScript embedded in Svelte <script>
/// let config = tsv_lang::PrintConfig {
///     base_indent_offset: 1, // Account for Svelte wrapper indent
///     ..Default::default()
/// };
/// let script_doc = tsv_ts::build_program_doc(&program, source, config);
///
/// // Wrap with indent and render
/// let indented = tsv_lang::doc::indent(script_doc);
/// let output = tsv_lang::doc::print_doc_with_indent_resolved(
///     &indented, &config, 0, 0, &*interner.borrow()
/// );
/// ```
pub fn build_program_doc(
    program: &Program,
    source: &str,
    config: tsv_lang::PrintConfig,
) -> tsv_lang::doc::Doc {
    let printer = printer::Printer::with_config(
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

#[cfg(test)]
mod doc_path_tests {
    use super::*;

    /// Test that doc-based formatting matches imperative formatting
    fn compare_paths(source: &str) {
        let program = parse(source).expect("parse failed");
        let imperative = format(&program, source);
        let doc_based = format_with_doc(&program, source);

        if imperative != doc_based {
            panic!(
                "Formatting paths differ!\n\
                === SOURCE ===\n{source}\n\
                === IMPERATIVE ===\n{imperative}\
                === DOC-BASED ===\n{doc_based}"
            );
        }
    }

    #[test]
    fn test_variable_declarations() {
        compare_paths("const x = 1;");
        compare_paths("let y = 2;");
        compare_paths("var z = 3;");
        compare_paths("const a = 1, b = 2;");
    }

    #[test]
    fn test_function_declarations() {
        compare_paths("function foo() {}");
        compare_paths("function bar(a, b) { return a + b; }");
        compare_paths("function baz<T>(x: T): T { return x; }");
    }

    #[test]
    fn test_class_declarations() {
        compare_paths("class Foo {}");
        compare_paths("class Bar extends Foo {}");
        compare_paths("class Baz<T> { x: T; }");
    }

    #[test]
    fn test_expression_statements() {
        compare_paths("1;");
        compare_paths("x + y;");
        compare_paths("foo();");
        compare_paths("obj.method();");
    }

    #[test]
    fn test_imports_exports() {
        compare_paths("import { a } from 'b';");
        compare_paths("import * as ns from 'mod';");
        compare_paths("export { x, y };");
        compare_paths("export default foo;");
    }

    #[test]
    fn test_control_flow() {
        compare_paths("if (x) {}");
        compare_paths("if (x) {} else {}");
        compare_paths("for (let i = 0; i < 10; i++) {}");
        compare_paths("while (true) {}");
        compare_paths("switch (x) { case 1: break; }");
    }

    #[test]
    fn test_type_declarations() {
        compare_paths("type T = number;");
        compare_paths("interface I { x: number; }");
        compare_paths("enum E { A, B }");
    }

    #[test]
    fn test_multiline_content() {
        compare_paths(
            r#"const x = {
  a: 1,
  b: 2,
};"#,
        );
        compare_paths(
            r#"function foo() {
  const x = 1;
  return x;
}"#,
        );
    }

    #[test]
    fn test_blank_lines() {
        compare_paths(
            r#"const x = 1;

const y = 2;"#,
        );
    }

    #[test]
    fn test_decorators() {
        compare_paths("@d\nclass A {}");
        compare_paths("@d()\nclass B {}");
        compare_paths("@d1\n@d2\nclass C {}");
    }

    #[test]
    fn test_class_members() {
        compare_paths("class A { x: number; }");
        compare_paths("class A { fn() {} }");
        compare_paths("class A { get x() { return 0; } }");
    }

    #[test]
    fn test_method_overloads() {
        compare_paths("class A { fn(x: string): void; fn(x: number): void; fn(x: any) {} }");
    }

    #[test]
    fn test_member_decorators() {
        compare_paths("class A { @d x: number; }");
        compare_paths("class A { @d fn() {} }");
        compare_paths("class A { @d1 @d2 x: number; }");
    }

    #[test]
    fn test_exported_decorated_classes() {
        compare_paths("@d\nexport class A {}");
        compare_paths("@d\nexport default class A {}");
        compare_paths("@d1\n@d2\nexport class B {}");
    }

    #[test]
    fn test_switch_complex() {
        compare_paths("switch (x) { case 1: case 2: break; }");
        compare_paths("switch (x) { case 1: a(); break; default: b(); }");
        compare_paths("switch (x) { case 1: a(); case 2: b(); break; default: c(); }");
    }

    #[test]
    fn test_try_catch_finally() {
        compare_paths("try {} catch {}");
        compare_paths("try {} catch (e) {}");
        compare_paths("try {} finally {}");
        compare_paths("try {} catch (e) {} finally {}");
    }

    #[test]
    fn test_class_member_modifiers() {
        compare_paths("class A { static x: number; }");
        compare_paths("class A { readonly x: number; }");
        compare_paths("class A { accessor x: number; }");
        compare_paths("class A { static readonly x: number; }");
        compare_paths("class A { public x: number; }");
        compare_paths("class A { private x: number; }");
        compare_paths("class A { protected x: number; }");
    }

    #[test]
    fn test_abstract_class_members() {
        compare_paths("abstract class A { abstract x: number; }");
        compare_paths("abstract class A { abstract fn(): void; }");
    }

    #[test]
    fn test_method_modifiers() {
        compare_paths("class A { static fn() {} }");
        compare_paths("class A { async fn() {} }");
        compare_paths("class A { *gen() {} }");
        compare_paths("class A { async *asyncGen() {} }");
        compare_paths("class A { override fn() {} }");
    }
}
