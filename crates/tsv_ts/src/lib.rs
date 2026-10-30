//! TypeScript parsing and formatting library
//!
//! This crate provides TypeScript AST parsing and code formatting.
//!
//! ## Usage
//!
//! ```
//! use tsv_ts::{Goal, convert_ast_json_bytes, format, format_with_goal, parse};
//!
//! // Format in one call: Module, retried as Script only if that fails
//! assert_eq!(format("const x=42;").unwrap(), "const x = 42;\n");
//! // ...or at exactly one goal, with no retry
//! assert!(format_with_goal("with (a) b;", Goal::Module).is_err());
//!
//! // Parse (the caller owns the bump arena the AST borrows) and emit the wire JSON AST
//! let source = "const x: number = 42;";
//! let arena = bumpalo::Bump::new();
//! let ast = parse(source, &arena).unwrap();
//! let json_bytes = convert_ast_json_bytes(&ast, source);
//! assert!(json_bytes.starts_with(b"{"));
//! ```

#[doc(hidden)]
pub mod __private;
pub mod ast;
mod goal;
mod lexer;
mod parser;
mod printer;
mod strip;

use __private::{PrinterInputs, format_folded_in, parse_folded};
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::is_format_ignore_directive;
use tsv_lang::printing::{FoldedSource, LineBreaks, LineTable};
use tsv_lang::{CommentFreeWindow, EmbedContext, Span};
pub use tsv_lang::{Error, ErrorKind, ErrorReport, Result, WirePoint};

pub use goal::Goal;
pub use strip::{StripError, Stripped, UnsupportedKind, strip, strip_with_goal};

/// The coordinates a TypeScript document's positions are reported in — acorn's: a
/// leading BOM **counted** (acorn reads U+FEFF as whitespace, so it is one UTF-16 unit at
/// offset 0), and ECMAScript's line terminators (LF, CR, CRLF as one, U+2028, U+2029).
/// The wire writer's tables and every parse error's point (`Error::wire_point`, and
/// the `line:col` its message prints) read this one statement.
pub const WIRE_COORDINATES: tsv_lang::WireCoordinates = tsv_lang::WireCoordinates {
    lines: tsv_lang::LineRule::Ecmascript,
    bom: tsv_lang::LeadingBom::Counted,
};

/// Build an *output* printer — pre-sizes the output buffer to the source
/// length for the rendering path. Used by the entry points that write the
/// buffer and return a `String` (`format`, `format_canonical`).
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

/// Run `build` on a doc-only printer over `inputs` ([`make_doc_printer`]), then hand the
/// comment-free window its searches drew back to the embedding printer
/// (`PrinterInputs::comment_free_window`), so the next island that printer constructs —
/// and its own next ask — start from where this one left off.
fn with_doc_printer<'a, R>(
    arena: &'a DocArena,
    inputs: &PrinterInputs<'a>,
    embed: EmbedContext,
    build: impl FnOnce(&printer::Printer<'a>) -> R,
) -> R {
    let printer = make_doc_printer(arena, inputs, embed);
    let out = build(&printer);
    if let Some(window) = inputs.comment_free_window {
        window.take_from(&printer.comment_free_gap);
    }
    out
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
/// * `Err(Error)` - If parsing fails
///
/// # Example
///
/// ```rust,ignore
/// let arena = bumpalo::Bump::new();
/// let ast = tsv_ts::parse("const x = 42;", &arena)?;
/// ```
pub fn parse<'arena>(source: &str, arena: &'arena bumpalo::Bump) -> Result<Program<'arena>> {
    Error::ensure_source_fits(source)?;
    parser::parse_typescript(source, arena).map_err(|e| e.with_context(source, WIRE_COORDINATES))
}

/// Parse TypeScript source against an explicit [`Goal`] (`Script` vs `Module`).
///
/// [`parse`] is the `Goal::Module` form (the default, correct for Svelte
/// `<script>` and ~all real TS). Pass `Goal::Script` to parse a standalone
/// script, where `await` is an ordinary identifier and top-level `import`/`export`
/// declarations, `import.meta`, and top-level `await` expressions are syntax
/// errors — a TypeScript namespace body holds `import`/`export` at either goal, since
/// tsc decides module-ness from top-level statements alone. The goal carries no
/// strictness of its own past `Module`: a script is
/// sloppy unless its directive prologue holds a `"use strict"`, so the strict-mode
/// rules the parser enforces parse only under a directive-free `Script`. Which
/// constructs those are is enumerated once, on the flag that gates them
/// (`Parser::strict`).
pub fn parse_with_goal<'arena>(
    source: &str,
    goal: Goal,
    arena: &'arena bumpalo::Bump,
) -> Result<Program<'arena>> {
    Error::ensure_source_fits(source)?;
    parser::parse_typescript_with_goal(source, goal, arena)
        .map_err(|e| e.with_context(source, WIRE_COORDINATES))
}

/// Parse TypeScript against `goal`, or — when the goal is unset — at `Module`
/// with a `Script` retry.
///
/// The **format** path's parse, and the type strip's (`strip_with_goal`). `Some(goal)` is
/// exact, the same parse [`parse_with_goal`] runs. `None` means the caller never named a
/// source type (`tsv format <path>`, an editor's `format_typescript(source)`), and there the
/// module grammar is tried first and the script grammar only if it *fails* — so a
/// legacy sloppy script (a `with` statement, a leading-zero literal, `await` as a
/// name) formats from a bare path while a module-valid source is never reinterpreted.
/// When both attempts fail, the reported error is the attempt's whose grammar the file
/// was written against, decided in two steps. **A script attempt that died on a goal
/// gate** — a top-level `import` / `export`, an `import.meta`, a top-level `for await`, or
/// the operand a module reads after a top-level `await` (where a Script's name reading of
/// the word fails), the constructs only a module holds ([`Error::is_goal_gated`]) —
/// has proved the file a module whatever else is in it, so the module attempt's error is
/// the file's own: a
/// broken module's script attempt dies there at its first module-only construct, even one
/// that sits *after* the real error (definitions first, `export` at the bottom).
/// **Otherwise the error that reached furthest into the source** is the file's own, the
/// module attempt's on a tie: a broken sloppy script's module attempt dies early at its
/// first `with` / legacy literal / `await` name — the very construct the retry exists to
/// admit, so reporting *that* would point a legacy script's author away from their typo.
/// prettier reads the same way in practice: its babel parser tolerates the
/// strict-mode rules tsv's parser enforces at the module goal (`allowedReasonCodes`), so the
/// error it shows on such a file is the typo. Pinned by
/// `tests/format_fallback_error_attribution.rs`.
///
/// **A module attempt the depth limit refused takes no retry** ([`ErrorKind::TooDeep`]):
/// the limit is no verdict on the grammar, and the script grammar reads every construct the
/// module one accepts as the same tree but `await`, which it reads as a name — so a script
/// attempt could only succeed by reading the source differently
/// (`await <T>a` as `await < T > a`) — a reading chosen by the limit rather than by the
/// source, which the same text one level shallower would never have been given.
///
/// [`parse`] and [`parse_with_goal`] have no fallback: a parse hands back an AST
/// whose `sourceType` is a claim about which grammar produced it, and a retry would
/// make that claim depend on the input. Formatting exposes neither the AST nor the
/// source type, and the printer never reads the goal, so the retry changes no output
/// for anything the module grammar already accepts — the strip's reason too: it hands
/// back JS text, and no verdict of its walk reads the goal. The retry parses into the same
/// arena with no reset between the two attempts, so a script's arena high-water is the
/// failed module attempt plus the script parse — an error-path cost only.
pub fn parse_with_goal_or_fallback<'arena>(
    source: &str,
    goal: Option<Goal>,
    arena: &'arena bumpalo::Bump,
) -> Result<Program<'arena>> {
    if let Some(goal) = goal {
        return parse_with_goal(source, goal, arena);
    }
    parse_with_goal(source, Goal::Module, arena).or_else(|module_error| {
        if module_error.kind() == ErrorKind::TooDeep {
            return Err(module_error);
        }
        parse_with_goal(source, Goal::Script, arena).map_err(|script_error| {
            // A goal gate proves the source a module; past that, the further error is
            // the file's own and a tie keeps the module's. The one positionless error a
            // TypeScript parse raises (`FileTooLarge`) is raised by both attempts alike, so
            // the comparison never sees a `None` against a `Some`.
            if !script_error.is_goal_gated() && script_error.position() > module_error.position() {
                script_error
            } else {
                module_error
            }
        })
    })
}

/// Parse standalone TypeScript with grouping parens preserved.
///
/// Like [`parse`] but keeps `(expr)` as a `ParenthesizedExpression` node (acorn's
/// `preserveParens: true`), so the paren structure is
/// present in the AST and its wire JSON. The binding audit uses this to reparse
/// formatted output and see which parenthesized subtree a glued comment binds to
/// — a re-binding is invisible in the paren-free public AST.
///
/// ⚠️ The tree is for the **wire** (`convert_ast_json_*`) and for walks that read it,
/// never for [`format()`] or any other printer entry point: a `ParenthesizedExpression`
/// answers no paren question about its position, so the printer refuses one in a debug
/// build and prints its contents bare in a release one — dropping every pair the
/// position needs (`(x + y) * 2` → `x + y * 2`). Format the [`parse`] tree instead.
pub fn parse_preserve_parens<'arena>(
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<Program<'arena>> {
    Error::ensure_source_fits(source)?;
    parser::parse_typescript_preserve_parens(source, arena)
        .map_err(|e| e.with_context(source, WIRE_COORDINATES))
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

/// Parse and format `source` in one call, with no source type named.
///
/// Carriage returns (`\r\n` and a lone `\r`) are folded to `\n` ahead of the parse, so
/// the output is LF-only.
///
/// The parse is [`parse_with_goal_or_fallback`]'s unset form — `Module`, retried
/// at `Script` only if that fails — which is what every format surface that takes
/// no source type from its caller runs. [`format_with_goal`] is the exact-goal twin.
///
/// # Errors
///
/// A parse error under both goals (the reported one follows
/// [`parse_with_goal_or_fallback`]).
pub fn format(source: &str) -> Result<String> {
    format_with_optional_goal(source, None)
}

/// Parse and format `source` in one call at exactly `goal`, with no Module-to-Script
/// fallback.
///
/// The exact-goal twin of [`format()`]: a source the goal's grammar rejects is an error
/// even when the other goal would accept it (`with (a) b;` is an error under
/// [`Goal::Module`]; `import x from 'y';` is one under [`Goal::Script`]). Folds carriage
/// returns ahead of the parse like [`format()`].
///
/// # Errors
///
/// A parse error at `goal`.
pub fn format_with_goal(source: &str, goal: Goal) -> Result<String> {
    format_with_optional_goal(source, Some(goal))
}

fn format_with_optional_goal(source: &str, goal: Option<Goal>) -> Result<String> {
    // The format path's line-terminator fold, ahead of the parse — see
    // `tsv_lang::printing::normalize_carriage_returns` for why it belongs here and not on
    // the finished string. `parse` deliberately does NOT do this: its offsets are a
    // drop-in contract over the author's own bytes.
    let folded = tsv_lang::printing::normalize_carriage_returns(source);
    let arena = bumpalo::Bump::new();
    let program = parse_folded(&folded, goal, &arena)?;
    let doc_arena = DocArena::for_source(folded.text());
    Ok(format_folded_in(&program, &folded, &doc_arena))
}

/// The shared body of `__private::format_ast_in` and `format_folded_in`: the one place the
/// intent-preserving format merges its comments, registers them and hands the printer its
/// table.
fn format_document_in(
    program: &Program<'_>,
    source: &str,
    line_breaks: LineBreaks<'_>,
    arena: &DocArena,
) -> String {
    // The printer's comment VIEW: a byte-adjacent indentable pair is one comment (the rule,
    // and why it is a view rather than a parse, is `tsv_lang::merge_nestled_block_comments`).
    let comments = tsv_lang::merge_nestled_block_comments(source, program.comments);
    // The print-once comment ledger's expectation for this document (diagnostic; see
    // `tsv_lang::comment_ledger`) — the MERGED list, which is the one the printers emit. A
    // Svelte host registers its own `Root.comments`, so an embedded `<script>` never
    // reaches here.
    #[cfg(feature = "comment_check")]
    tsv_lang::comment_ledger::register_parsed(source, &comments);
    format_program_in(program, source, &comments, line_breaks, arena, false)
}

/// The shared body of every format: given the document's line verdict (the arena-parked
/// table behind it is filled only if a line question falls back to it — one warm table
/// across a multi-file driver's files instead of a fresh Vec per file), build the printer,
/// render, park the table. `canonical` switches the printer into canonical mode after
/// construction — the build takes the real table (comment classification needs it) and
/// `set_canonical` then erases only the *layout* table, so blank-line / expansion reads
/// collapse while classification keeps the real lines.
fn format_program_in(
    program: &Program<'_>,
    source: &str,
    comments: &[ast::Comment],
    line_breaks: LineBreaks<'_>,
    arena: &DocArena,
    canonical: bool,
) -> String {
    // The intent-preserving entry points read lines off `source` with `\n` alone, so they
    // take CR-folded text; the canonical reprint erases those reads and is exempt.
    debug_assert!(
        canonical || !source.contains('\r'),
        "a format entry point takes CR-folded text (`tsv_lang::printing::normalize_carriage_returns`)"
    );
    let inputs = PrinterInputs::for_document(source, comments, line_breaks.table());
    // This printer IS the document — a break after any doc it builds is its own, so an
    // emitter may defer a `//` to the end of a line.
    let mut printer = make_printer(arena, &inputs, EmbedContext::line_owning());
    if canonical {
        printer.set_canonical();
    }
    printer.print_program(program);
    let output = printer.into_string();
    arena.park_line_breaks_scratch(line_breaks.into_scratch());
    output
}

/// Convert internal AST to compact JSON wire bytes with character-based positions
///
/// The span-only wire every binding ships, and `tsv parse`'s default: each node carries
/// `start` / `end` as UTF-16 offsets and no `loc`. Line/column is a pure function of those
/// offsets plus the source, so a consumer holding the source loses nothing — it derives
/// `loc` on demand (the npm packages' `{locations: true}`), and emission skips the line
/// table entirely, mirroring acorn's own `locations: false`.
///
/// The **sole emission path** for its wire: emits the wire JSON directly during a single
/// walk of the internal AST (the writer in `ast/convert/write/`), never materializing a
/// typed public tree or an intermediate `Value`, and fuses the byte→UTF-16 offset
/// translation into that walk: the writer receives the byte→UTF-16 map via
/// `WirePositions` and emits final char-space positions directly, so no post-conversion
/// translation walk runs. For ASCII sources the map is empty and emission is byte-space
/// passthrough. Returning bytes lets a consumer that never needs `&str` skip the
/// O(output) UTF-8 validation `convert_ast_json_string` pays (the output is several times
/// the source).
#[cfg(feature = "convert")]
pub fn convert_ast_json_bytes(program: &Program<'_>, source: &str) -> Vec<u8> {
    ast::convert::write_program_bytes(program, source, tsv_lang::Wire::Span)
}

/// Convert internal AST to compact JSON wire bytes **with** per-node `loc`.
///
/// The `convert_ast_json_bytes` wire plus a `loc` on every node — the line (1-based) and
/// column (0-based, UTF-16 code units) of its own `start` / `end` under ECMAScript's line
/// terminators (CR, LF, CRLF, LS, PS), which is acorn's. `tsv parse --locations` writes it.
#[cfg(feature = "locations")]
pub fn convert_ast_json_bytes_with_locations(program: &Program<'_>, source: &str) -> Vec<u8> {
    ast::convert::write_program_bytes(program, source, tsv_lang::Wire::Loc)
}

/// The `String` form of `convert_ast_json_bytes` for `&str` boundaries (the WASM
/// binding's `JSON.parse`, N-API strings): same wire bytes plus one UTF-8 validation of
/// the output. Byte-oriented consumers should prefer the bytes variant.
///
/// There is no `_with_locations` `String` twin: the `&str` boundaries this form serves are
/// the bindings, which ship the span-only wire alone, and the loc-bearing wire's only
/// callers (`tsv parse --locations`, `tsv_debug`) write bytes.
#[cfg(feature = "convert")]
#[expect(clippy::expect_used)]
pub fn convert_ast_json_string(program: &Program<'_>, source: &str) -> String {
    String::from_utf8(convert_ast_json_bytes(program, source))
        .expect("writer emits valid UTF-8 (source slices + ASCII fragments)")
}

/// Build an embedding [`parser::Parser`] at `base_offset`, run `f` over it, then the
/// host's `host_check` over what `f` returned. What `f` returns borrows only `arena`. It
/// runs through the driver the standalone parse takes too
/// (`parser::parse_retrying_acorn_followers`), so a failed parse that gave a
/// type-argument list up to tsc's comparison is run once more reading acorn-typescript's
/// lists — which is why `f` is `Fn`. A host refusal fails the parse it judged, so a
/// head the host refuses under tsc's comparison is re-read the same way; an entry point
/// whose host refuses nothing passes `|_| Ok(())`. Where both readings fail, the
/// re-read's error is the one reported: Svelte reads every embedded text as
/// acorn-typescript does, so that error lands where Svelte's does.
///
/// An error leaves here positioned in the host document and context-free. Its point is
/// the host's to take: `source` is a slice of that document, so a context taken over it
/// would read a host-absolute position against the wrong text and clamp it to a
/// plausible-but-wrong point. The host fills it over the whole document instead.
fn with_embedding_parser<'arena, T>(
    source: &str,
    base_offset: usize,
    host_nesting: u32,
    arena: &'arena bumpalo::Bump,
    f: impl Fn(&mut parser::Parser<'_, 'arena>) -> std::result::Result<T, Error>,
    host_check: impl Fn(&T) -> std::result::Result<(), Error>,
) -> Result<T> {
    parser::parse_retrying_acorn_followers(
        source,
        base_offset,
        host_nesting,
        Goal::Module,
        arena,
        true,
        f,
        host_check,
    )
}

/// Run `build` on the doc-only printer a whole `<script>` program prints through: its own
/// island-local comments in the merged view, the host's `source` and `line_table`, and the
/// program's span start as the document start the format-ignore placement floor reads
/// (`Printer::program_start`) — the environment `build_program_doc` documents.
fn with_program_printer<R>(
    arena: &DocArena,
    program: &Program<'_>,
    source: &str,
    line_table: LineTable<'_>,
    embed: EmbedContext,
    build: impl FnOnce(&printer::Printer<'_>) -> R,
) -> R {
    let comments = tsv_lang::merge_nestled_block_comments(source, program.comments);
    let inputs = PrinterInputs::for_document(source, &comments, line_table);
    let mut printer = make_doc_printer(arena, &inputs, embed);
    printer.program_start = program.span.start;
    build(&printer)
}

/// Whether the `>` at `gt` in `bytes` closes a type-argument list this crate's parser
/// reads — `f<T> / 2`, where the `/` divides, against `a > /re/`, where it opens a regex.
/// The TypeScript grammar's `tsv_lang::source_scan::ClosesTypeArguments`
/// (`OPERAND_GRAMMAR`). `lower_bound` is the scan's own start, which bounds the walk.
#[must_use]
pub fn closes_type_arguments(bytes: &[u8], gt: usize, lower_bound: usize) -> bool {
    parser::closes_type_arguments(bytes, gt, lower_bound)
}

/// Whether the `)` at `rparen` in `bytes` closes a statement header this crate's parser
/// reads — `if (c)!/re/`, where the glued `!` is a prefix not and the `/` opens a regex,
/// against `f()! / 2`, a postfix non-null divided. The TypeScript grammar's
/// `tsv_lang::source_scan::ClosesStatementHeader` (`OPERAND_GRAMMAR`). `lower_bound` is
/// the scan's own start, where its forward walk begins.
#[must_use]
pub fn closes_statement_header(bytes: &[u8], rparen: usize, lower_bound: usize) -> bool {
    parser::closes_statement_header(bytes, rparen, lower_bound)
}

/// Printer buffer-population sampling for `tsv_debug buffer_sizes`. Behind the
/// `buffer_stats` feature (off in production builds).
#[cfg(feature = "buffer_stats")]
pub use printer::buffer_stats::{
    BufferInlineCapacities, BufferStats, inline_capacities, set_buffer_stats, take_buffer_stats,
};

// Re-exports of types that appear in this crate's public function signatures
// (`Program`, `Expression` and its variant half `ExpressionKind`, `TSTypeAnnotation`)
// or are named via the short `tsv_ts::Foo` path by external consumers (`Statement` and
// its variant half `StatementKind`, `ObjectProperty`, `ObjectPatternProperty` — currently
// only by tsv_svelte). All other AST types remain accessible through the full
// `tsv_ts::ast::Foo` path.
pub use ast::{
    Expression, ExpressionKind, ObjectPatternProperty, ObjectProperty, Program, Statement,
    StatementKind, TSTypeAnnotation, TSTypeParameterDeclaration, VariableDeclaration,
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

    let mut lexer = Lexer::at_offset(source, 0);
    let mut out = String::new();
    loop {
        match lexer.next_token() {
            Ok(token) => {
                let _ = write!(out, "{:?} {}..{}", token.kind, token.start, token.end);
                if let Some(decoded) = lexer.decoded_str() {
                    let _ = write!(out, " decoded={decoded:?}");
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
