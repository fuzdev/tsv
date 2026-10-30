//! CSS parsing and formatting library
//!
//! Provides CSS parsing, formatting, and AST conversion functionality: [`format()`] formats
//! one source in one call, and a [`Formatter`] formats many on one thread, keeping its
//! arenas warm between calls.
//! Part of the tsv (precise language tools for TypeScript/JS, CSS, and Svelte in Rust) project.

#[doc(hidden)]
pub mod __private;
#[doc(hidden)]
pub mod ast;
mod color;
mod comments;
mod escapes;
mod formatter;
mod keyword_set;
mod lexer;
mod number;
mod parser;
mod printer;
mod url;
mod whitespace;

use __private::{format_folded_in, parse_folded};
pub use formatter::Formatter;
pub use tsv_lang::{
    Error, ErrorKind, ErrorReport, LeadingBom, LineRule, Result, STACK_SIZE, WireCoordinates,
    WirePoint, run_with_stack,
};

/// The coordinates a CSS document's positions are reported in — `parseCss`'s: a leading
/// BOM **elided** (`parseCss` strips it before parsing, so every offset indexes the
/// BOM-less string), and `\n` alone ending a line. The wire writer's tables and every
/// parse error's point (`Error::wire_point`, and the `line:col` its message prints)
/// read this one statement.
pub const WIRE_COORDINATES: WireCoordinates = WireCoordinates {
    lines: LineRule::Lf,
    bom: LeadingBom::Elided,
};

/// Parse and format `source` in one call.
///
/// Carriage returns (`\r\n` and a lone `\r`) are folded to `\n` ahead of the parse, so
/// the output is LF-only.
///
/// # Errors
///
/// A parse error.
pub fn format(source: &str) -> Result<String> {
    format_in(
        source,
        &bumpalo::Bump::new(),
        tsv_lang::doc::arena::DocArena::for_source,
    )
}

/// The one format path, shared by [`format()`] and [`Formatter`]: fold, parse into `arena`,
/// then print into the doc arena `doc_arena` hands over once the parse has succeeded (handed
/// the folded text, so a fresh one is sized to it and a failed parse never builds one).
/// Nothing borrowed from either arena escapes: the result is owned.
pub(crate) fn format_in<D: std::borrow::Borrow<tsv_lang::doc::arena::DocArena>>(
    source: &str,
    arena: &bumpalo::Bump,
    doc_arena: impl FnOnce(&str) -> D,
) -> Result<String> {
    // The format path's line-terminator fold, ahead of the parse (see
    // `tsv_lang::printing::normalize_carriage_returns`); `parse` leaves the author's bytes
    // alone so its offsets stay a drop-in contract with `parseCss`'s.
    let folded = tsv_lang::printing::normalize_carriage_returns(source);
    let stylesheet = parse_folded(&folded, arena)?;
    let doc_arena = doc_arena(folded.text());
    Ok(format_folded_in(&stylesheet, &folded, doc_arena.borrow()))
}

/// What [`parse_to_json_with`]'s wire carries. The default is [`parse_to_json`]'s: the
/// span-only wire.
#[cfg(feature = "json")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct JsonOptions {
    #[cfg(feature = "locations")]
    locations: bool,
}

#[cfg(feature = "json")]
impl JsonOptions {
    /// The default options: the span-only wire.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "locations")]
            locations: false,
        }
    }

    /// Add a `loc` to every object carrying `start` / `end`: the line (1-based) and column
    /// (0-based, UTF-16 code units) of those offsets, under `\n` alone. `tsv parse
    /// --locations` writes it. Default `false`, the span-only wire.
    #[cfg(feature = "locations")]
    #[must_use]
    pub const fn locations(mut self, locations: bool) -> Self {
        self.locations = locations;
        self
    }
}

// Options are plain data a caller may build once and share across threads; pin that so a
// field added later cannot take it away unnoticed.
#[cfg(feature = "json")]
const _: () = {
    const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    assert_send_sync_static::<JsonOptions>();
};

/// Parse `source` and return its wire JSON AST: compact JSON in Svelte's `parseCss()` shape
/// (a standalone `StyleSheetFile`), every node's `start` / `end` a UTF-16 offset and no
/// `loc`.
///
/// The span-only wire every binding ships and `tsv parse` writes by default. Line and
/// column are a pure function of those offsets plus the source, so a consumer holding the
/// source loses nothing; `JsonOptions::locations` (the `locations` feature) adds them. The
/// source is read as written — no line-terminator fold — so the offsets index the caller's
/// own text.
///
/// # Errors
///
/// A parse error.
#[cfg(feature = "json")]
pub fn parse_to_json(source: &str) -> Result<String> {
    parse_to_json_with(source, &JsonOptions::new())
}

/// [`parse_to_json`] under `options`: whether the wire carries `loc`.
///
/// Each call parses into a fresh arena sized to the source, dropped once the wire is
/// written.
///
/// # Errors
///
/// A parse error.
#[cfg(feature = "json")]
pub fn parse_to_json_with(source: &str, options: &JsonOptions) -> Result<String> {
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    let ast = __private::parse(source, &arena)?;
    #[cfg(feature = "locations")]
    if options.locations {
        return Ok(wire_string(
            __private::convert_ast_json_bytes_with_locations(&ast, source),
        ));
    }
    #[cfg(not(feature = "locations"))]
    let _ = options;
    Ok(__private::convert_ast_json_string(&ast, source))
}

/// The wire's bytes as the `String` they are: the writer emits source slices and ASCII
/// fragments alone.
#[cfg(feature = "locations")]
#[expect(clippy::expect_used)]
fn wire_string(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("writer emits valid UTF-8 (source slices + ASCII fragments)")
}

/// Drive the raw lexer over `source` and return a deterministic, line-per-token
/// dump (`<kind> <start>..<end> [text=…]`, terminated by `Eof` or `ERROR …`).
///
/// The differential gate for lexer changes: two lexer implementations are
/// token-identical iff this string matches for every corpus file. Behind the
/// `internal_debug_lex` feature (off in production builds); used by `tsv_debug lex_diff`.
///
/// For `Identifier` tokens it prints the **resolved text** — the decoded value
/// when escapes were present, otherwise the verbatim source slice. That keeps the
/// golden invariant across the lazy-decode change: a no-escape identifier's
/// decoded value equals its source slice, so the stream is stable whether the
/// lexer allocates the decode eagerly or only when an escape is seen, while a
/// wrong escape decode still shows up as a divergent `text=`.
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
                if matches!(token.kind, TokenKind::Identifier) {
                    let slice = &source[token.start as usize..token.end as usize];
                    let text = lexer.decoded_str().unwrap_or(slice);
                    let _ = write!(out, " text={text:?}");
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
