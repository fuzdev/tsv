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
mod lexer;
mod parser;
mod printer;

use std::rc::Rc;

use tsv_lang::EmbedContext;
use tsv_lang::doc::arena::{DocArena, DocId};
pub use tsv_lang::{ParseError, Result, SharedInterner};

/// The per-document environment shared by every formatting entry point: the
/// source the AST's spans index into, the shared interner, the comment buffer,
/// and the precomputed line breaks. Bundling these keeps the printer
/// constructor — and the `tsv_svelte` embedding call sites — from re-threading
/// the same values. The [`EmbedContext`] and the expression/program being
/// printed vary per call, so they stay separate args.
pub struct PrinterInputs<'a> {
    /// Full source the AST's spans index into.
    pub source: &'a str,
    /// Interner shared with the parse phase (cloned per printer).
    pub interner: SharedInterner,
    /// Detached comment buffer for the document.
    pub comments: &'a [ast::Comment],
    /// Precomputed newline offsets for O(log n) line/column lookup.
    pub line_breaks: &'a [u32],
}

/// Build an *output* printer — pre-sizes the output buffer to the source
/// length for the rendering path. Used by the entry points that write the
/// buffer and return a `String` (`format`, `format_expression`).
fn make_printer<'a>(
    arena: &'a DocArena,
    inputs: &PrinterInputs<'a>,
    embed: EmbedContext,
) -> printer::Printer<'a> {
    printer::Printer::with_context(arena, inputs, embed, inputs.source.len())
}

/// Build a *doc-only* printer for the embedding entry points (`build_*_doc`):
/// they emit a `DocId` into the caller's arena and never render, so the
/// source-length output buffer [`make_printer`] reserves would be a pure
/// per-call allocation (one per template `{expr}` / directive / `<script>`).
/// A zero-capacity `String` does not allocate, so the buffer stays free unless
/// something writes to it — which these paths never do.
fn make_doc_printer<'a>(
    arena: &'a DocArena,
    inputs: &PrinterInputs<'a>,
    embed: EmbedContext,
) -> printer::Printer<'a> {
    printer::Printer::with_context(arena, inputs, embed, 0)
}

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
    let arena = DocArena::for_source(source);
    let inputs = PrinterInputs {
        source,
        interner: Rc::clone(&program.interner),
        comments: &program.comments,
        line_breaks: &program.line_breaks,
    };
    let mut printer = make_printer(&arena, &inputs, EmbedContext::default());
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
#[cfg(feature = "convert")]
pub fn convert_ast(program: &Program, source: &str) -> ast::public::Program {
    let tracker = tsv_lang::LocationTracker::new_ecmascript(source);
    ast::convert::convert_program(program, source, &tracker, ast::convert::Schema::Acorn)
}

/// Convert internal AST to JSON with character-based positions
///
/// Like `convert_ast`, but returns `serde_json::Value` with all byte-based
/// positions (`start`, `end`, `loc.*.column`) translated to Unicode character
/// offsets to match acorn output.
///
/// This is the preferred function for producing JSON AST output.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json(program: &Program, source: &str) -> serde_json::Value {
    let tracker = tsv_lang::LocationTracker::new_ecmascript(source);
    let public_ast =
        ast::convert::convert_program(program, source, &tracker, ast::convert::Schema::Acorn);
    let mut json = serde_json::to_value(&public_ast).expect("AST types derive Serialize correctly");
    let map = tsv_lang::ByteToCharMap::new(source);
    ast::convert::translate_byte_to_char_offsets(&mut json, &map, &tracker);
    json
}

/// Convert internal AST to a compact JSON string with character-based positions
///
/// Byte-identical to `serde_json::to_string(&convert_ast_json(...))`, but
/// serializes the typed public AST directly, skipping the intermediate
/// `serde_json::Value` (`serde_json`'s `preserve_order` keeps struct-field key
/// order). For ASCII sources byte offsets already equal char offsets; multibyte
/// sources get the typed offset-translation walk
/// (`translate_byte_to_char_offsets_typed`) before serialization. This is the
/// hot path for the FFI/WASM parse bindings and the CLI's compact output.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(program: &Program, source: &str) -> String {
    let tracker = tsv_lang::LocationTracker::new_ecmascript(source);
    let mut public_ast =
        ast::convert::convert_program(program, source, &tracker, ast::convert::Schema::Acorn);
    // No ASCII gate: `ByteToCharMap::new` short-circuits to an empty map for
    // ASCII sources and the typed walk early-returns on it, so gating here
    // would just scan the source a second time.
    let map = tsv_lang::ByteToCharMap::new(source);
    ast::convert::translate_byte_to_char_offsets_typed(&mut public_ast, &map, &tracker);
    let mut buf = Vec::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    serde_json::to_writer(&mut buf, &public_ast).expect("AST types derive Serialize correctly");
    String::from_utf8(buf).expect("serde_json emits valid UTF-8")
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
    interner: SharedInterner,
) -> Result<Program> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser.parse().map_err(|e| e.with_context(source))
}

/// Parse a single TypeScript expression and return it with any comments.
///
/// This is used when parsing expressions in contexts where comments need to be
/// preserved (e.g., Svelte expression tags `{/* comment */ expr}`).
pub fn parse_expression_with_comments(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
) -> Result<(Expression, Vec<ast::Comment>)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    parser
        .parse_expression_with_comments()
        .map_err(|e| e.with_context(source))
}

/// Format a single TypeScript expression to a string.
///
/// `expression` was parsed as part of a larger document (e.g., a Svelte
/// template); `inputs.source` is the full document the expression's spans index
/// into. `embed.base_indent_offset` seeds the printer's indent level so wrapped
/// lines (method chains, multiline arrays) indent relative to the surrounding
/// context.
pub fn format_expression(
    expression: &Expression,
    inputs: &PrinterInputs<'_>,
    embed: EmbedContext,
) -> String {
    let arena = DocArena::for_source(inputs.source);
    let mut printer = make_printer(&arena, inputs, embed);
    printer.set_indent_level(embed.base_indent_offset);
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
///   Parse a pattern with comments, handling optional type annotations.
///
/// Used in Svelte block contexts (`{:then}`, `{:catch}`) where patterns
/// may have type annotations (e.g., `{:then num: number}`).
pub fn parse_pattern_with_comments(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
) -> Result<(Expression, Vec<ast::Comment>)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    let expr = parser
        .parse_expression_public()
        .map_err(|e| e.with_context(source))?;
    let mut pattern = parser
        .expression_to_pattern(expr)
        .map_err(|e| e.with_context(source))?;
    // Check for type annotation (`: Type`) — used in Svelte block contexts
    // like `{:then num: number}` and `{:catch error: Error}`
    if parser.at_colon() {
        let ta = parser
            .parse_type_annotation_public()
            .map_err(|e| e.with_context(source))?;
        if let Expression::Identifier(id) = &mut pattern {
            id.type_annotation = Some(ta);
        }
    }
    let comments = parser.take_comments();
    Ok((pattern, comments))
}

/// Parse a type annotation (`: Type`) and return it with the position where parsing stopped.
///
/// Used in Svelte block contexts where patterns may have type annotations
/// after simple identifiers (e.g., `{#each items as x: number}`).
/// The source must start with `:`.
pub fn parse_type_annotation_partial(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
) -> Result<(TSTypeAnnotation, usize)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    let ta = parser
        .parse_type_annotation_public()
        .map_err(|e| e.with_context(source))?;
    let pos = parser.current_absolute_position();
    Ok((ta, pos))
}

/// Parse a partial expression, stopping at top-level commas, and return it
/// with any collected comments.
///
/// Used when parsing patterns in contexts where commas have other meanings,
/// such as `{#each items as pattern, index}` where the comma separates the
/// pattern from the index variable. Uses assignment-expression parsing which
/// stops at top-level commas (but handles commas inside objects/arrays/calls
/// correctly).
pub fn parse_expression_partial_with_comments(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
) -> Result<(Expression, usize, Vec<ast::Comment>)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    let (expr, end_pos) = parser
        .parse_assignment_expression_partial()
        .map_err(|e| e.with_context(source))?;
    let comments = parser.take_comments();
    Ok((expr, end_pos, comments))
}

/// Parse a single statement and return it with any collected comments.
///
/// Used by embedders whose host syntax wraps a statement — e.g. Svelte's
/// `{const …}` / `{let …}` tags, which are a `VariableDeclaration` (no trailing
/// `;`). The statement's `span().end` is the byte offset just past it.
pub fn parse_statement_with_comments(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
) -> Result<(Statement, Vec<ast::Comment>)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner)?;
    let stmt = parser
        .parse_statement()
        .map_err(|e| e.with_context(source))?;
    let comments = parser.take_comments();
    Ok((stmt, comments))
}

/// Build a DocId for a variable declaration in the caller's arena.
///
/// `emit_semicolon` is `false` for embedders that supply their own terminator
/// (Svelte declaration tags close with `}`). Set `inputs.comments` to `&[]` when
/// no comments need to be preserved.
pub fn build_variable_declaration_doc_with_comments(
    arena: &DocArena,
    decl: &VariableDeclaration,
    inputs: &PrinterInputs<'_>,
    embed: &EmbedContext,
    emit_semicolon: bool,
) -> DocId {
    let printer = make_doc_printer(arena, inputs, *embed);
    printer.build_variable_declaration_doc(decl, emit_semicolon)
}

/// Build a DocId for a TypeScript expression with comments in the caller's arena.
///
/// Set `inputs.comments` to `&[]` when no comments need to be preserved.
pub fn build_expression_doc_with_comments(
    arena: &DocArena,
    expression: &Expression,
    inputs: &PrinterInputs<'_>,
    embed: &EmbedContext,
) -> DocId {
    let printer = make_doc_printer(arena, inputs, *embed);
    printer.build_expression_doc(expression)
}

/// Build a DocId for a function parameter list (`(…)`) with comments, in the caller's arena.
///
/// Routes each parameter through the same comment-aware, `FunctionParameter`-context
/// printer a real function signature uses, so interior comments (`{ a = /* c */ 1 }`),
/// boundary comments (`a /* c */, b`), the single-pattern hug, and nesting-depth
/// expansion all match a standalone parameter list. `params_start` / `trailing_comments_end`
/// are the source positions of the `(` and `)` (for leading / dangling / trailing comment
/// lookup). Emits no group of its own — the caller's surrounding group controls breaking.
/// Used by `tsv_svelte` for `{#snippet}` parameters.
pub fn build_function_params_doc_with_comments(
    arena: &DocArena,
    params: &[Expression],
    params_start: Option<u32>,
    trailing_comments_end: Option<u32>,
    inputs: &PrinterInputs<'_>,
    embed: &EmbedContext,
) -> DocId {
    let printer = make_doc_printer(arena, inputs, *embed);
    printer.build_params_doc_with_comments(params, params_start, trailing_comments_end)
}

/// Build a DocId for a type-parameter declaration (`<…>`) with comments, in the caller's arena.
///
/// Routes the generics through the same comment-aware, width-wrapping type-parameter printer a
/// real function/class signature uses, so constraints (`<T extends X>`), defaults (`<T = X>`),
/// modifiers (`<const T>`), interior comments (`<T /* c */>`), and per-param wrapping of a long
/// generic list all match a standalone declaration. The emitted doc includes its own group and
/// the surrounding `<` / `>`, breaking independently of the parameter list. Used by `tsv_svelte`
/// for `{#snippet}` generics.
pub fn build_type_parameters_doc_with_comments(
    arena: &DocArena,
    type_parameters: &TSTypeParameterDeclaration,
    inputs: &PrinterInputs<'_>,
    embed: &EmbedContext,
) -> DocId {
    let printer = make_doc_printer(arena, inputs, *embed);
    printer.build_type_parameter_declaration_doc_wrapping(type_parameters)
}

/// Build a DocId for a TypeScript program in the caller's arena.
///
/// Returns a DocId that can be rendered with the arena.
/// Used when embedding TypeScript in other formats like Svelte's `<script>`.
pub fn build_program_doc(
    arena: &DocArena,
    program: &Program,
    source: &str,
    embed: EmbedContext,
) -> DocId {
    let inputs = PrinterInputs {
        source,
        interner: Rc::clone(&program.interner),
        comments: &program.comments,
        line_breaks: &program.line_breaks,
    };
    let printer = make_doc_printer(arena, &inputs, embed);
    printer.build_program_doc(program)
}

// Assignment-layout predicates for embedders: tsv_svelte's {@const} tag
// mirrors Prettier's assignment layout selection and must apply the same
// break-after-operator rules as our own assignment printer.
pub use printer::{conditional_should_break_after_op, should_inline_logical_expression};

// Re-exports of types that appear in this crate's public function signatures
// (`Program`, `Expression`, `TSTypeAnnotation`) or are named via the short
// `tsv_ts::Foo` path by external consumers (`Statement`, `ObjectProperty`,
// `ObjectPatternProperty` — currently only by tsv_svelte). All other AST
// types remain accessible through the full `tsv_ts::ast::internal::Foo` path.
pub use ast::internal::{
    Expression, ObjectPatternProperty, ObjectProperty, Program, Statement, TSTypeAnnotation,
    TSTypeParameterDeclaration, VariableDeclaration,
};
