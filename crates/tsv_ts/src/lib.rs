//! TypeScript parsing and formatting library
//!
//! This crate provides TypeScript formatting, the type strip, and parsing to the wire JSON
//! AST (acorn-typescript's shape).
//!
//! ## Usage
//!
//! ```
//! use tsv_ts::{Goal, format, format_with_goal, strip};
//!
//! // Format in one call: Module, retried as Script only if that fails
//! assert_eq!(format("const x=42;").unwrap(), "const x = 42;\n");
//! // ...or at exactly one goal, with no retry
//! assert!(format_with_goal("with (a) b;", Goal::Module).is_err());
//!
//! // Strip the types, keeping every surviving character at its line and column
//! assert_eq!(strip("let x: number = 1;").unwrap().code, "let x         = 1;");
//!
//! // Parse to the wire JSON AST (the `json` feature, on by default)
//! # #[cfg(feature = "json")]
//! # {
//! let json = tsv_ts::parse_to_json("const x: number = 42;").unwrap();
//! assert!(json.starts_with(r#"{"type":"Program""#));
//! # }
//! ```
//!
//! ## Goals
//!
//! A TypeScript source parses against a [`Goal`] — ECMAScript's `Module` or `Script` grammar —
//! and each entry point reads it in one of two tiers:
//!
//! - **The bare name** names no goal. [`format()`] and [`strip()`] parse at `Module` and
//!   retry at `Script` only if that fails, so a legacy sloppy script (a `with` statement, a
//!   leading-zero literal, `await` as a name) formats and strips while a module-valid source
//!   is never reinterpreted — neither output depends on which grammar read it.
//!   `parse_to_json` parses at `Module` alone, with no retry: the wire's
//!   `Program.sourceType` is a claim about which grammar produced it, which a retry would
//!   make depend on the input.
//! - **A named goal** — [`format_with_goal`], [`strip_with_goal`], and
//!   `JsonOptions::goal` for `parse_to_json_with` — parses at exactly that goal, with
//!   no retry: a source the goal's grammar rejects is an error even when the other goal
//!   would accept it.
//!
//! When both attempts of a fallback fail, the error reported is the attempt's whose grammar
//! the file was written against, decided in two steps. **A script attempt that died on a
//! goal gate** — a top-level `import` / `export`, an `import.meta`, a top-level `for await`,
//! or the operand a module reads after a top-level `await`, the constructs only a module
//! holds — has proved the file a module whatever else is in it, so the module attempt's
//! error is the file's own, even one that sits *after* the real error (definitions first,
//! `export` at the bottom). **Otherwise the error that reached furthest into the source**
//! is the file's own, the module attempt's on a tie: a broken sloppy script's module attempt
//! dies early at its first `with` / legacy literal / `await` name — the very construct the
//! retry exists to admit — so reporting *that* would point a legacy script's author away
//! from their typo. A module attempt the depth limit refused ([`ErrorKind::TooDeep`]) takes
//! no retry: the limit is no verdict on the grammar.

#[doc(hidden)]
pub mod __private;
#[doc(hidden)]
pub mod ast;
mod goal;
mod lexer;
mod parser;
mod printer;
mod strip;

use __private::{PrinterInputs, format_folded_in, parse_folded};
use ast::Program;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::is_format_ignore_directive;
use tsv_lang::printing::{FoldedSource, LineBreaks, LineTable};
use tsv_lang::{CommentFreeWindow, EmbedContext};
pub use tsv_lang::{
    Error, ErrorKind, ErrorReport, LeadingBom, LineRule, Result, Span, WireCoordinates, WirePoint,
};

pub use goal::Goal;
pub use strip::{StripError, Stripped, UnsupportedKind, strip, strip_with_goal};

/// The coordinates a TypeScript document's positions are reported in — acorn's: a
/// leading BOM **counted** (acorn reads U+FEFF as whitespace, so it is one UTF-16 unit at
/// offset 0), and ECMAScript's line terminators (LF, CR, CRLF as one, U+2028, U+2029).
/// The wire writer's tables and every parse error's point (`Error::wire_point`, and
/// the `line:col` its message prints) read this one statement.
pub const WIRE_COORDINATES: WireCoordinates = WireCoordinates {
    lines: LineRule::Ecmascript,
    bom: LeadingBom::Counted,
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

/// Parse and format `source` in one call, with no source type named.
///
/// Carriage returns (`\r\n` and a lone `\r`) are folded to `\n` ahead of the parse, so
/// the output is LF-only.
///
/// No goal is named, so the parse is the bare-name tier of [Goals](crate#goals) — `Module`,
/// retried at `Script` only if that fails — which is what every format surface that takes
/// no source type from its caller runs. [`format_with_goal`] is the exact-goal twin.
///
/// # Errors
///
/// A parse error under both goals (the one [Goals](crate#goals) picks).
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
    #[cfg(feature = "internal_comment_check")]
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

/// How [`parse_to_json_with`] parses and what its wire carries. The default is
/// [`parse_to_json`]'s: [`Goal::Module`], the span-only wire.
///
/// ```
/// # #[cfg(feature = "locations")]
/// # {
/// use tsv_ts::{Goal, JsonOptions, parse_to_json_with};
///
/// let options = JsonOptions::new().goal(Goal::Script).locations(true);
/// let json = parse_to_json_with("with (a) b;", &options).unwrap();
/// assert!(json.contains(r#""sourceType":"script""#));
/// assert!(json.contains(r#""loc":"#));
/// # }
/// ```
#[cfg(feature = "json")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct JsonOptions {
    goal: Goal,
    #[cfg(feature = "locations")]
    locations: bool,
}

#[cfg(feature = "json")]
impl JsonOptions {
    /// The default options: [`Goal::Module`], the span-only wire.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            goal: Goal::Module,
            #[cfg(feature = "locations")]
            locations: false,
        }
    }

    /// Parse at exactly `goal`, with no retry (see [Goals](crate#goals)). Default
    /// [`Goal::Module`].
    #[must_use]
    pub const fn goal(mut self, goal: Goal) -> Self {
        self.goal = goal;
        self
    }

    /// Add a `loc` to every node: the line (1-based) and column (0-based, UTF-16 code
    /// units) of its own `start` / `end`, under ECMAScript's line terminators (LF, CR, CRLF,
    /// U+2028, U+2029), which are acorn's. `tsv parse --locations` writes it. Default
    /// `false`, the span-only wire.
    #[cfg(feature = "locations")]
    #[must_use]
    pub const fn locations(mut self, locations: bool) -> Self {
        self.locations = locations;
        self
    }
}

#[cfg(feature = "json")]
impl Default for JsonOptions {
    fn default() -> Self {
        Self::new()
    }
}

// Options are plain data a caller may build once and share across threads; pin that so a
// field added later cannot take it away unnoticed.
#[cfg(feature = "json")]
const _: () = {
    const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    assert_send_sync_static::<JsonOptions>();
};

/// Parse `source` as a module and return its wire JSON AST: compact JSON in the
/// acorn-typescript shape, every node's `start` / `end` a UTF-16 offset and no `loc`.
///
/// The span-only wire every binding ships and `tsv parse` writes by default. Line and
/// column are a pure function of those offsets plus the source, so a consumer holding the
/// source loses nothing; `JsonOptions::locations` (the `locations` feature) adds them. The
/// source is read as written — no line-terminator fold — so the offsets index the caller's
/// own text.
///
/// The goal is [`Goal::Module`], with no retry (see [Goals](crate#goals));
/// [`parse_to_json_with`] names another.
///
/// # Errors
///
/// A parse error at the module goal.
#[cfg(feature = "json")]
pub fn parse_to_json(source: &str) -> Result<String> {
    parse_to_json_with(source, &JsonOptions::new())
}

/// [`parse_to_json`] under `options`: the goal it parses at (exact, no retry) and whether
/// the wire carries `loc`.
///
/// Each call parses into a fresh arena sized to the source, dropped once the wire is
/// written.
///
/// # Errors
///
/// A parse error at the options' goal.
#[cfg(feature = "json")]
pub fn parse_to_json_with(source: &str, options: &JsonOptions) -> Result<String> {
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    let program = __private::parse_with_goal(source, options.goal, &arena)?;
    #[cfg(feature = "locations")]
    if options.locations {
        return Ok(wire_string(
            __private::convert_ast_json_bytes_with_locations(&program, source),
        ));
    }
    Ok(__private::convert_ast_json_string(&program, source))
}

/// The wire's bytes as the `String` they are: the writer emits source slices and ASCII
/// fragments alone.
#[cfg(feature = "locations")]
#[expect(clippy::expect_used)]
fn wire_string(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("writer emits valid UTF-8 (source slices + ASCII fragments)")
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
pub(crate) fn closes_type_arguments(bytes: &[u8], gt: usize, lower_bound: usize) -> bool {
    parser::closes_type_arguments(bytes, gt, lower_bound)
}

/// Whether the `)` at `rparen` in `bytes` closes a statement header this crate's parser
/// reads — `if (c)!/re/`, where the glued `!` is a prefix not and the `/` opens a regex,
/// against `f()! / 2`, a postfix non-null divided. The TypeScript grammar's
/// `tsv_lang::source_scan::ClosesStatementHeader` (`OPERAND_GRAMMAR`). `lower_bound` is
/// the scan's own start, where its forward walk begins.
#[must_use]
pub(crate) fn closes_statement_header(bytes: &[u8], rparen: usize, lower_bound: usize) -> bool {
    parser::closes_statement_header(bytes, rparen, lower_bound)
}

/// Printer buffer-population sampling for `tsv_debug buffer_sizes`. Behind the
/// `internal_buffer_stats` feature (off in production builds).
#[cfg(feature = "internal_buffer_stats")]
pub use printer::buffer_stats::{
    BufferInlineCapacities, BufferStats, inline_capacities, set_buffer_stats, take_buffer_stats,
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
/// `internal_debug_lex` feature (off in production builds); used by `tsv_debug lex_diff`.
#[cfg(feature = "internal_debug_lex")]
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
/// type-name and literal keywords the lexer also lexes. Behind the `internal_debug_lex`
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
#[cfg(feature = "internal_debug_lex")]
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
