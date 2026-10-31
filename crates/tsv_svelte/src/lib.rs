//! Svelte formatting, and parsing to the wire JSON of Svelte's modern `parse()` — part of
//! [tsv](https://github.com/fuzdev/tsv).
//!
//! [`format()`] formats one component in one call; a [`Formatter`] formats many on one
//! thread, keeping its arenas warm between calls. Each `<script>`, `<style>` and template
//! expression is formatted by [`tsv_ts`](https://docs.rs/tsv_ts) or
//! [`tsv_css`](https://docs.rs/tsv_css) as part of one document. The formatter is
//! non-configurable: Prettier's style with a print width of 100, tabs, single quotes and no
//! trailing commas. Formatting output is not frozen — it changes between releases as
//! formatting bugs are fixed.
//!
//! ```
//! use tsv_svelte::{ErrorKind, format};
//!
//! assert_eq!(
//!     format("<script>let x=1</script><p>{x}</p>").unwrap(),
//!     "<script>\n\tlet x = 1;\n</script>\n\n<p>{x}</p>\n"
//! );
//!
//! // a refusal: formatting folds this `\r` to `\n`, which would end the in-tag comment early
//! let err = format("<div // a\rb\n></div>").unwrap_err();
//! assert_eq!(err.kind(), ErrorKind::Refusal);
//! assert_eq!(err.span(), Some(9..10));
//!
//! // Parse to the wire JSON AST (the `json` feature, on by default)
//! # #[cfg(feature = "json")]
//! # {
//! let json = tsv_svelte::parse_to_json("<p>x</p>").unwrap();
//! assert!(json.contains(r#""type":"Root""#));
//! # }
//! ```
//!
//! Every tsv language crate returns the same [`Error`]: its [`Error::kind`], its
//! [`Error::span`] in the input's UTF-8 bytes, and [`Error::report`] in the wire's UTF-16
//! offsets and lines; one line under `{}`, the `tsv` CLI's frame under `{:#}`.
//!
//! ## Deep input
//!
//! The parser refuses a tree past its depth limits with [`ErrorKind::TooDeep`], so every
//! walk of an accepted tree fits a 2 MiB thread in an optimized build and a 16 MiB one in a
//! debug build. The limits count a whole
//! component, its `<script>`, `<style>` and template expressions included, against one
//! budget. A stack overflow aborts the process rather than panicking, so a debug
//! build handling input it does not control — `cargo test` runs each test on a 2 MiB thread —
//! wraps the call in [`run_with_stack`], which runs it on a thread reserving [`STACK_SIZE`].
//!
//! ## Features
//!
#![cfg_attr(
    feature = "json",
    doc = " - `json` (default) — [`parse_to_json`], [`parse_to_json_with`] and [`JsonOptions`]. Turn"
)]
#![cfg_attr(
    not(feature = "json"),
    doc = " - `json` (default) — `parse_to_json`, `parse_to_json_with` and `JsonOptions`. Turn"
)]
//!   default features off for a format-only build.
//! - `locations` — `JsonOptions::locations`, adding a `loc` (line and column) to every
//!   positioned object of the wire, and a `name_loc` to elements, attributes and directives.
//!
//! The `_internal_*` features are tsv's own diagnostics, not for consumers
//! ([Stability](#stability)).
//!
//! ## Stability
//!
//! This release exposes no AST: parsing returns the wire JSON, in Svelte's modern
//! `parse()` shape, which `@fuzdev/tsv-parse-wasm`'s `tsv_ast.d.ts` types.
//!
//! Items hidden from these docs (`__private`, `ast` and every other `#[doc(hidden)]` item)
//! and the `_internal_*` features are tsv's own plumbing for its sibling crates, which pin
//! each other exactly; they change in any release, outside semver.

#![warn(missing_docs)]

#[doc(hidden)]
pub mod __private;
#[doc(hidden)]
// A hidden module renders nowhere public, so its links to private items are maintainer
// navigation (`docs:audit` documents private items); only the public-only docs.rs build
// fires the lint, which `docs:audit:docsrs` denies everywhere else.
#[expect(rustdoc::private_intra_doc_links)]
pub mod ast;
mod formatter;
mod lexer;
mod parser;
mod printer;
#[cfg(test)]
mod test_support;
mod whitespace;

use __private::{format_folded_in, parse_folded};
use ast::Root;
pub use formatter::Formatter;
pub use tsv_lang::{
    Error, ErrorKind, ErrorReport, LeadingBom, LineRule, Result, STACK_SIZE, WireCoordinates,
    WirePoint, run_with_stack,
};

/// The coordinates a Svelte document's positions are reported in — Svelte's: a leading
/// BOM **elided** (Svelte's `parse` strips it before parsing, so every offset indexes the
/// BOM-less string, its acorn islands included), and `\n` alone ending a line, in the
/// markup and in every `<script>`, template expression and `<style>` alike. The wire
/// writer's tables and every parse error's point (`Error::wire_point`, and the
/// `line:col` its message prints) read this one statement.
pub const WIRE_COORDINATES: WireCoordinates =
    WireCoordinates::new(LineRule::Lf, LeadingBom::Elided);

/// `parse_folded`'s refusal: parse the author's own bytes, and refuse the first in-tag `//`
/// comment that holds a lone `<CR>`. Its parse lands in `arena` beside the folded one, which
/// the caller resets with it — a cost only a document holding a lone `<CR>` pays.
#[cold]
#[inline(never)]
fn refuse_lone_cr_in_tag_line_comment(source: &str, arena: &bumpalo::Bump) -> Result<()> {
    let root = __private::parse(source, arena)?;
    let bytes = source.as_bytes();
    for comment in &root.comments {
        // Only the template reader's `//` runs to a `\n` alone; acorn's and the CSS
        // parser's end at a `<CR>` already.
        if !comment.from_template_reader || comment.is_block {
            continue;
        }
        // A `<CR>` opening a `<CR><LF>` is the comment's own line end, folded with its pair;
        // a lone one with only whitespace after it (`<CR><CR><LF>`, a doubled line ending)
        // ends the comment where the fold does, so nothing turns into markup.
        let content = comment.content_span.range();
        let end = content.end;
        let lone = content
            .clone()
            .find(|&i| bytes[i] == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
        if let Some(position) = lone
            && bytes[position + 1..end]
                .iter()
                .any(|b| !matches!(b, b' ' | b'\t' | b'\r'))
        {
            return Err(Error::refusal(
                "Lone carriage return inside a '//' comment in a tag: formatting folds it to a \
                 line feed, which would end the comment early",
                source,
                // the span is the `<CR>` itself, the one character the refusal is about
                position,
                position + 1,
                WIRE_COORDINATES,
            ));
        }
    }
    Ok(())
}

/// Parse and format `source` in one call.
///
/// Carriage returns (`\r\n` and a lone `\r`) are folded to `\n` ahead of the parse, so
/// the output is LF-only.
///
/// # Errors
///
/// A parse error, or a refusal of a lone `\r` inside an in-tag `//` comment, where the
/// fold would end the comment early.
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
    // alone so its offsets stay a drop-in contract with Svelte's.
    let folded = tsv_lang::printing::normalize_carriage_returns(source);
    let root = parse_folded(&folded, arena)?;
    let doc_arena = doc_arena(folded.text());
    Ok(format_folded_in(&root, &folded, doc_arena.borrow()))
}

/// What [`parse_to_json_with`]'s wire carries. The default is [`parse_to_json`]'s: the
/// span-only wire.
///
/// ```
/// # #[cfg(feature = "locations")]
/// # {
/// use tsv_svelte::{JsonOptions, parse_to_json_with};
///
/// let json = parse_to_json_with("<p>x</p>", JsonOptions::new().locations(true)).unwrap();
/// assert!(json.contains(r#""loc":"#));
/// assert!(json.contains(r#""name_loc":"#));
/// # }
/// ```
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
    /// (0-based, UTF-16 code units) of those offsets, under `\n` alone — the one line rule for
    /// a Svelte document and everything embedded in it (a superset of Svelte's `loc`, which
    /// acorn-parsed nodes alone carry); elements, attributes and directives also gain their
    /// `name_loc`. `tsv parse --locations` writes it. Default `false`, the span-only wire.
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

/// Parse `source` and return its wire JSON AST: compact JSON in Svelte's modern `parse()`
/// shape, every node's `start` / `end` a UTF-16 offset and no `loc`.
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
    parse_to_json_with(source, JsonOptions::new())
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
pub fn parse_to_json_with(source: &str, options: JsonOptions) -> Result<String> {
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

/// Byte spans of the instance/module `<script>` element contents.
///
/// Comments inside these spans belong to the embedded TS programs; comments
/// outside them are template expression comments. The wire writer partitions
/// the root comment list on it.
#[cfg(feature = "json")]
pub(crate) fn script_content_spans(root: &Root<'_>) -> Vec<(u32, u32)> {
    let mut script_spans: Vec<(u32, u32)> = Vec::new();
    if let Some(script) = root.instance {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
    if let Some(script) = root.module {
        script_spans.push((script.content.span.start, script.content.span.end));
    }
    script_spans
}
