//! TypeScript parsing and formatting library
//!
//! This crate provides TypeScript AST parsing and code formatting.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use tsv_ts::{parse, format, convert_ast_json_bytes};
//!
//! // Parse TypeScript code (the caller owns the bump arena the AST borrows)
//! let source = "const x: number = 42;";
//! let arena = bumpalo::Bump::new();
//! let ast = parse(source, &arena)?;
//!
//! // Format TypeScript code
//! let formatted = format(&ast, source);
//!
//! // Emit the wire JSON AST
//! let json_bytes = convert_ast_json_bytes(&ast, source);
//! ```

pub mod ast;
mod goal;
mod lexer;
mod parser;
mod printer;

use std::rc::Rc;

use tsv_lang::EmbedContext;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::is_format_ignore_directive;
use tsv_lang::printing::build_line_breaks_into;
pub use tsv_lang::{ParseError, Result, SharedInterner};

pub use goal::Goal;

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
    /// Whether any comment in this document is owned by a node (`owned_by_node`).
    /// A document-level presence flag that short-circuits the owned-leading-comment
    /// path (`prepend_owned_leading_comment` & siblings), which otherwise runs a byte
    /// gate once per expression node — the highest-frequency comment path — to conclude
    /// "no owned comment" for the ~all documents that have none.
    ///
    /// **Compute this once per document, from the parsed comment list** (e.g.
    /// `comments.iter().any(|c| c.owned_by_node)`), and pass it in. It must NOT be
    /// derived inside `Printer::with_context` or `tsv_svelte`'s `ts_inputs()`: the latter
    /// is called per template `{expr}`, so an `any()` scan there is O(islands × comments)
    /// and regresses `.svelte`. `owned_by_node` is set at parse time (including for
    /// Svelte-embedded TS, parsed eagerly), so the flag is stable before any printing.
    pub has_owned_comments: bool,
    /// Whether any comment in this document is a `format-ignore` directive.
    /// A document-level presence flag that short-circuits `has_format_ignore_in_range`,
    /// which otherwise runs a range binary search + directive-string match once per
    /// top-level statement / member expression / object property — concluding "no
    /// format-ignore" for the ~all documents that have none.
    ///
    /// **Compute this once per document, from the parsed comment list** (e.g.
    /// `comments.iter().any(|c| is_format_ignore_directive(c.content(source)))`), and pass
    /// it in — never inside `Printer::with_context` or `tsv_svelte`'s `ts_inputs()` (the
    /// per-`{expr}` O(islands × comments) trap the sibling `has_owned_comments` documents).
    pub has_format_ignore: bool,
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
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_ts::parse("const x = 42;", &arena)?;
/// ```
pub fn parse<'arena>(source: &str, arena: &'arena bumpalo::Bump) -> Result<Program<'arena>> {
    parser::parse_typescript(source, arena).map_err(|e| e.with_context(source))
}

/// Parse TypeScript source against an explicit [`Goal`] (`Script` vs `Module`).
///
/// [`parse`] is the `Goal::Module` form (the default, correct for Svelte
/// `<script>` and ~all real TS). Pass `Goal::Script` to parse a standalone
/// strict script, where `await` is an ordinary identifier and `import`/`export`
/// declarations, `import.meta`, and top-level `await` expressions are syntax
/// errors. tsv is strict under both goals.
pub fn parse_with_goal<'arena>(
    source: &str,
    goal: Goal,
    arena: &'arena bumpalo::Bump,
) -> Result<Program<'arena>> {
    parser::parse_typescript_with_goal(source, goal, arena).map_err(|e| e.with_context(source))
}

/// Parse standalone TypeScript with grouping parens preserved.
///
/// Like [`parse`] but keeps `(expr)` as a `ParenthesizedExpression` node (acorn's
/// `preserveParens: true`) against a fresh interner, so the paren structure is
/// present in the AST and its wire JSON. The binding audit uses this to reparse
/// formatted output and see which parenthesized subtree a glued comment binds to
/// — a re-binding is invisible in the paren-free public AST.
pub fn parse_preserve_parens<'arena>(
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<Program<'arena>> {
    parser::parse_typescript_preserve_parens(source, arena).map_err(|e| e.with_context(source))
}

/// Whether a block comment's content (delimiters excluded) is a JSDoc type cast —
/// the `/**`-form comment carrying `@type`/`@satisfies` that binds forward to the
/// `(` it precedes (prettier's `isTypeCastComment`).
///
/// Exposed for the binding audit, which classifies a glued comment's anchor rule
/// by whether it is a cast (its parens are expected — the audit looks *inside*
/// them) or an ordinary glued/annotation comment (its next token is what it binds).
pub fn is_jsdoc_type_cast_comment(content: &str) -> bool {
    parser::is_jsdoc_type_cast_comment(content)
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
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_ts::parse(source, &arena)?;
/// let formatted = tsv_ts::format(&ast, source);
/// assert_eq!(formatted, "const x = 42;\n");
/// ```
pub fn format(program: &Program<'_>, source: &str) -> String {
    let arena = DocArena::for_source(source);
    format_in(program, source, &arena)
}

/// Format into a caller-provided doc arena.
///
/// Identical output to [`format`], but the doc IR is built into `arena` instead
/// of a freshly allocated one, so a driver that formats many files can reuse one
/// arena across them (`arena.reset()` between files retains the buffers). Nothing
/// borrowed from `arena` escapes — the result is an owned `String` — so the
/// caller may reset and reuse it the moment this returns.
pub fn format_in(program: &Program<'_>, source: &str, arena: &DocArena) -> String {
    // The print-once comment ledger's expectation for this document (diagnostic; see
    // `tsv_lang::comment_ledger`). A Svelte host registers its own `Root.comments`, so
    // an embedded `<script>` never reaches here.
    #[cfg(feature = "comment_check")]
    tsv_lang::comment_ledger::register_parsed(source, program.comments);

    // Fill the arena-parked line-break table (one warm table across a
    // multi-file driver's files instead of a fresh Vec per file).
    let mut line_breaks = arena.take_line_breaks_scratch();
    build_line_breaks_into(source, &mut line_breaks);
    let inputs = PrinterInputs {
        source,
        interner: Rc::clone(&program.interner),
        comments: program.comments,
        line_breaks: &line_breaks,
        has_owned_comments: program.comments.iter().any(|c| c.owned_by_node),
        has_format_ignore: program
            .comments
            .iter()
            .any(|c| is_format_ignore_directive(c.content(source))),
    };
    let mut printer = make_printer(arena, &inputs, EmbedContext::default());
    printer.print_program(program);
    let output = printer.into_string();
    arena.park_line_breaks_scratch(line_breaks);
    output
}

/// Convert internal AST to JSON with character-based positions
///
/// Returns a `serde_json::Value` parsed from the wire bytes
/// `convert_ast_json_bytes` emits — a thin wrapper over the sole emission
/// path, not an independent conversion. Used where a `Value` is needed (the
/// CLI's `--pretty`); byte-oriented consumers should call
/// `convert_ast_json_bytes` directly.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json(program: &Program<'_>, source: &str) -> serde_json::Value {
    serde_json::from_slice(&convert_ast_json_bytes(program, source))
        .expect("writer emits valid JSON")
}

/// Convert internal AST to compact JSON wire bytes with character-based positions
///
/// Byte-identical to `serde_json::to_string(&convert_ast_json(...))`, but emits
/// the wire JSON directly during a single walk of the internal AST (the writer
/// in `ast/convert/write/`), never materializing the typed public tree, and
/// fuses the byte→UTF-16 offset translation into that walk: the writer receives
/// the `ByteToCharMap` via `LocationMapper` and emits final char-space
/// positions directly, so no post-conversion translation walk runs. For ASCII
/// sources the map is empty and emission is byte-space passthrough. This is
/// the hot path for the FFI parse binding and the CLI's compact output — both
/// hand the bytes on without ever needing `&str`, so they skip the O(output)
/// UTF-8 validation `convert_ast_json_string` pays (the output is ~15× the
/// source).
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes(program: &Program<'_>, source: &str) -> Vec<u8> {
    convert_ast_json_bytes_variant(program, source, true)
}

/// Convert internal AST to compact JSON wire bytes **without** per-node `loc`.
///
/// The opt-in `no-locations` variant of `convert_ast_json_bytes`: emits
/// `start`/`end` offsets but drops the per-node `loc` object (line/column) that
/// the acorn/svelte drop-in wire carries. `loc` is a pure function of a node's
/// `start`/`end` (UTF-16 offsets) plus the source, so a consumer that has the
/// source loses nothing — line/column is derived lazily. Dropping it removes
/// ~46% of the wire and ~61% of the downstream `JSON.parse` cost (three nested
/// objects per node), and lets emission skip the per-node line/column lookup
/// entirely. This is a distinct, narrower product from the default wire — not a
/// second encoding of the acorn contract — mirroring acorn's own
/// `locations: false`.
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes_no_locations(program: &Program<'_>, source: &str) -> Vec<u8> {
    convert_ast_json_bytes_variant(program, source, false)
}

#[cfg(feature = "convert")]
fn convert_ast_json_bytes_variant(program: &Program<'_>, source: &str, locations: bool) -> Vec<u8> {
    // One fused source scan builds both; ASCII sources take a byte-level line
    // scan and get the identity map. The `no-locations` path emits no line/column,
    // so it skips the line-start scan entirely (`new_map_only` builds just the
    // byte→char map) — a once-per-file entry branch, no per-node cost.
    let (tracker, map) = if locations {
        tsv_lang::LocationTracker::new_ecmascript_with_map(source)
    } else {
        tsv_lang::LocationTracker::new_map_only(source)
    };
    ast::convert::write_program_json(
        program,
        source,
        tsv_lang::LocationMapper {
            tracker: &tracker,
            map: &map,
        },
        ast::convert::Schema::Acorn,
        locations,
    )
}

/// Convert internal AST to a compact JSON string with character-based positions
///
/// The `String` form of `convert_ast_json_bytes` for `&str` boundaries (the
/// WASM binding's `JSON.parse`, N-API strings): same wire bytes plus one
/// UTF-8 validation of the output. Byte-oriented consumers should prefer the
/// bytes variant.
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string(program: &Program<'_>, source: &str) -> String {
    String::from_utf8(convert_ast_json_bytes(program, source))
        .expect("writer emits valid UTF-8 (source slices + ASCII fragments)")
}

/// The `String` form of `convert_ast_json_bytes_no_locations` for `&str`
/// boundaries (the WASM binding's `JSON.parse`, N-API strings).
#[cfg(feature = "convert")]
#[allow(clippy::expect_used)]
pub fn convert_ast_json_string_no_locations(program: &Program<'_>, source: &str) -> String {
    String::from_utf8(convert_ast_json_bytes_no_locations(program, source))
        .expect("writer emits valid UTF-8 (source slices + ASCII fragments)")
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
pub fn parse_with_interner<'arena>(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
    arena: &'arena bumpalo::Bump,
) -> Result<Program<'arena>> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner, arena)?;
    parser.parse().map_err(|e| e.with_context(source))
}

/// Parse embedded TypeScript with grouping parens preserved.
///
/// Like [`parse_with_interner`] but keeps `(expr)` as an internal
/// `ParenthesizedExpression` node instead of discarding it. Used for the
/// `{#snippet}`-parameter sub-parse (`function f(PARAMS) {}`), where Svelte
/// parses with acorn's `preserveParens: true` and — unlike every other template
/// expression — skips `remove_parens`, so its public AST keeps the parens. All
/// other embedded parses ([`parse_with_interner`], expression/pattern parses)
/// stay paren-free, matching acorn/Svelte.
pub fn parse_with_interner_preserve_parens<'arena>(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
    arena: &'arena bumpalo::Bump,
) -> Result<Program<'arena>> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner, arena)?;
    parser.preserve_parens = true;
    parser.parse().map_err(|e| e.with_context(source))
}

/// Parse a single TypeScript expression and return it with any comments.
///
/// This is used when parsing expressions in contexts where comments need to be
/// preserved (e.g., Svelte expression tags `{/* comment */ expr}`).
pub fn parse_expression_with_comments<'arena>(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
    arena: &'arena bumpalo::Bump,
) -> Result<(Expression<'arena>, &'arena [ast::Comment])> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner, arena)?;
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
    expression: &Expression<'_>,
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
pub fn parse_pattern_with_comments<'arena>(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
    arena: &'arena bumpalo::Bump,
) -> Result<(Expression<'arena>, &'arena [ast::Comment])> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner, arena)?;
    let expr = parser
        .parse_expression_unbounded()
        .map_err(|e| e.with_context(source))?;
    let mut pattern = parser
        .expression_to_pattern(expr)
        .map_err(|e| e.with_context(source))?;
    // Check for type annotation (`: Type`) — used in Svelte block contexts
    // like `{:then num: number}` and `{:catch error: Error}`
    if parser.at_colon() {
        let ta = parser
            .parse_type_annotation()
            .map_err(|e| e.with_context(source))?;
        if let Expression::Identifier(id) = &mut pattern {
            // Re-bind the identifier's binding extra with the parsed type
            // annotation (preserving any decorators already present).
            let decorators = id.decorators();
            id.extra = Some(arena.alloc(ast::internal::IdentifierParamExtra {
                type_annotation: Some(ta),
                decorators,
            }));
        }
    }
    // The pattern (plus any type annotation) must fill the whole slice — the Svelte
    // callers (`{@const id = …}`, `{:then pattern}`, `{:catch pattern}`) hand us a slice
    // bounded by `=`/`}`. Without this a trailing token is silently dropped
    // (`{@const x y = a}` → `{@const x = a}`), losing content.
    parser
        .expect_end_of_input()
        .map_err(|e| e.with_context(source))?;
    let comments = parser.take_comments();
    Ok((pattern, comments))
}

/// Parse a type annotation (`: Type`) and return it with the position where parsing stopped.
///
/// Used in Svelte block contexts where patterns may have type annotations
/// after simple identifiers (e.g., `{#each items as x: number}`).
/// The source must start with `:`.
pub fn parse_type_annotation_partial<'arena>(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
    arena: &'arena bumpalo::Bump,
) -> Result<(TSTypeAnnotation<'arena>, usize)> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner, arena)?;
    let ta = parser
        .parse_type_annotation()
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
pub fn parse_expression_partial_with_comments<'arena>(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
    arena: &'arena bumpalo::Bump,
) -> Result<(Expression<'arena>, usize, &'arena [ast::Comment])> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner, arena)?;
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
pub fn parse_statement_with_comments<'arena>(
    source: &str,
    base_offset: usize,
    interner: SharedInterner,
    arena: &'arena bumpalo::Bump,
) -> Result<(Statement<'arena>, &'arena [ast::Comment])> {
    let mut parser = parser::Parser::with_interner(source, base_offset, interner, arena)?;
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
    decl: &VariableDeclaration<'_>,
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
    expression: &Expression<'_>,
    inputs: &PrinterInputs<'_>,
    embed: &EmbedContext,
) -> DocId {
    let printer = make_doc_printer(arena, inputs, *embed);
    printer.build_expression_doc(expression)
}

/// Build a DocId for a single comment (`/* … */` / `// …`) in the caller's arena,
/// through the same rendering the standalone TS printer uses.
///
/// A multi-line block comment reindents to context and propagates its break via a
/// `MultilineText` node, identically to an owned leading comment
/// (`prepend_owned_leading_comment`). `tsv_svelte` uses it for a *non-owned* leading
/// comment in a directive-value gap (`bind:value={/* c⏎*/ (a > b)}`): a discarded
/// grouping `(` leaves the comment positional rather than owned, so the gap emits it —
/// and a multi-line block there must force the same expansion the bare (owned)
/// authoring takes, which the gap's plain source-span emitter cannot. `EmbedContext`
/// is irrelevant to comment rendering, so it is not exposed (a default is used).
pub fn build_comment_doc(
    arena: &DocArena,
    comment: &ast::Comment,
    inputs: &PrinterInputs<'_>,
) -> DocId {
    let printer = make_doc_printer(arena, inputs, EmbedContext::default());
    printer.build_comment_doc(comment)
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
    params: &[Expression<'_>],
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
    type_parameters: &TSTypeParameterDeclaration<'_>,
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
///
/// `line_breaks` must be the host document's whole-source newline table
/// (spans are absolute, so a table built from an island slice is wrong);
/// `comments`/`interner` stay island-local, taken from `program`.
pub fn build_program_doc(
    arena: &DocArena,
    program: &Program<'_>,
    source: &str,
    line_breaks: &[u32],
    embed: EmbedContext,
) -> DocId {
    let inputs = PrinterInputs {
        source,
        interner: Rc::clone(&program.interner),
        comments: program.comments,
        line_breaks,
        has_owned_comments: program.comments.iter().any(|c| c.owned_by_node),
        has_format_ignore: program
            .comments
            .iter()
            .any(|c| is_format_ignore_directive(c.content(source))),
    };
    let printer = make_doc_printer(arena, &inputs, embed);
    printer.build_program_doc(program)
}

// Assignment-layout predicates for embedders: tsv_svelte's {@const} tag
// mirrors Prettier's assignment layout selection and must apply the same
// break-after-operator rules as our own assignment printer.
pub use printer::{conditional_should_break_after_op, should_inline_logical_expression};

/// Printer buffer-population sampling for `tsv_debug buffer_sizes`. Behind the
/// `buffer_stats` feature (off in production builds).
#[cfg(feature = "buffer_stats")]
pub use printer::buffer_stats::{
    BufferInlineCapacities, BufferStats, inline_capacities, set_buffer_stats, take_buffer_stats,
};

// Re-exports of types that appear in this crate's public function signatures
// (`Program`, `Expression`, `TSTypeAnnotation`) or are named via the short
// `tsv_ts::Foo` path by external consumers (`Statement`, `ObjectProperty`,
// `ObjectPatternProperty` — currently only by tsv_svelte). All other AST
// types remain accessible through the full `tsv_ts::ast::internal::Foo` path.
pub use ast::internal::{
    Expression, ObjectPatternProperty, ObjectProperty, Program, Statement, TSTypeAnnotation,
    TSTypeParameterDeclaration, VariableDeclaration,
};

/// Drive the raw lexer over `source` and return a deterministic, line-per-token
/// dump (`<kind> <start>..<end> [decoded=…]`, terminated by `Eof` or `ERROR …`).
///
/// The differential gate for lexer changes: two lexer implementations are
/// token-identical iff this string matches for every corpus file. It exercises
/// the **context-free** `next_token` dispatch only —
/// the parser-driven `read_regex_literal` / `continue_template_from_brace` paths
/// aren't reached by a raw `next_token` loop (a `/` lexes as division, a `}` as a
/// brace), so those stay gated by the AST/format byte-identity suites. Behind the
/// `debug_lex` feature (off in production builds); used by `tsv_debug lex_diff`.
#[cfg(feature = "debug_lex")]
pub fn debug_token_stream(source: &str) -> String {
    use crate::lexer::{Lexer, TokenKind};
    use std::fmt::Write as _;

    let mut lexer = Lexer::new(source);
    let mut out = String::new();
    loop {
        match lexer.next_token() {
            Ok(token) => {
                let _ = write!(out, "{:?} {}..{}", token.kind, token.start, token.end);
                if let Some(decoded) = lexer.take_decoded() {
                    let _ = write!(out, " decoded={:?}", &*decoded);
                }
                out.push('\n');
                if matches!(token.kind, TokenKind::Eof) {
                    break;
                }
            }
            Err(err) => {
                let _ = writeln!(out, "ERROR {err:?}");
                break;
            }
        }
    }
    out
}

/// The **reserved** control-flow and declaration keywords the lexer recognizes —
/// the 38 words that head a statement or declaration, as distinct from the
/// type-name and literal keywords the lexer also lexes. Behind the `debug_lex`
/// feature (off in production builds), like `debug_token_stream`.
///
/// This is the independent oracle for `tsv_debug`'s `SHAPE_KEYWORDS` drift guard.
/// A shape key keeps a keyword verbatim but abstracts an ordinary identifier to
/// `IDENT`, so `return⟨⟩` and `IDENT⟨⟩` name different bugs. A reserved word that
/// is missing from `SHAPE_KEYWORDS` degrades to `IDENT` and silently merges its
/// bug into the generic-identifier entry — a quiet failure nothing else notices.
/// The guard test asserts every word returned here is present in that table.
///
/// The set is the lexer's full keyword table (52 words) minus the 14 that are
/// *not* control-flow/declaration reserved words: the 4 literals
/// (`true`/`false`/`null`/`undefined`), the 9 primitive type-name keywords
/// (`number`/`string`/`boolean`/`any`/`never`/`unknown`/`object`/`symbol`/`bigint`),
/// and `debugger`. `void` stays in the set — it is the `void` unary operator, not
/// a type name.
#[cfg(feature = "debug_lex")]
pub fn reserved_words() -> &'static [&'static str] {
    &[
        "const",
        "let",
        "var",
        "void",
        "new",
        "instanceof",
        "in",
        "return",
        "if",
        "else",
        "for",
        "while",
        "do",
        "switch",
        "case",
        "default",
        "break",
        "continue",
        "try",
        "catch",
        "finally",
        "throw",
        "function",
        "class",
        "enum",
        "typeof",
        "delete",
        "async",
        "await",
        "this",
        "super",
        "extends",
        "export",
        "import",
        "from",
        "as",
        "satisfies",
        "yield",
    ]
}
