//! CSS value printing
//!
//! Handles printing of all CSS value types:
//! - Simple values (identifiers, strings, dimensions, colors)
//! - Compound values (lists, functions)
//! - Semantic formatting with source fidelity
//!
//! ## Architecture
//!
//! This module uses a doc-first approach where all formatting logic lives in
//! `build_*_doc()` methods. The `print_*` methods are thin wrappers that call
//! the corresponding doc builder and write the result.
//!
//! The main entry point is `build_css_value_doc()`, which dispatches to
//! specialized doc builders for each value type.

use super::{Printer, value_normalization};
use crate::ast::internal::{CssValue, StringCooked};
use std::borrow::Cow;
use tsv_lang::Span;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::printing::format_string_literal;

impl<'a> Printer<'a> {
    /// Format a CSS value
    ///
    /// Uses the doc builder which handles source fidelity and proper formatting.
    pub(super) fn print_css_value(&mut self, value: &CssValue<'_>) {
        let doc = self.build_css_value_doc(value);
        self.write_arena_doc(doc);
    }

    /// Format a nested value (function arg, list item)
    ///
    /// Alias for `print_css_value` - kept for semantic clarity in call sites.
    #[inline]
    pub(super) fn print_nested_value(&mut self, value: &CssValue<'_>) {
        self.print_css_value(value);
    }

    //
    // Doc Builders - all formatting logic expressed as doc IR
    //

    /// Build a doc for a CSS value
    ///
    /// Main entry point for value formatting. Dispatches to specialized doc
    /// builders for each value type. Handles source fidelity by extracting
    /// from source where appropriate.
    pub(super) fn build_css_value_doc(&self, value: &CssValue<'_>) -> DocId {
        match value {
            CssValue::Identifier { span } => self.build_identifier_doc(*span),
            CssValue::String { content, span } => self.build_string_doc(content, *span),
            CssValue::Dimension { span, .. } => self.build_dimension_doc(*span),
            CssValue::Color { color, span } => self.build_color_doc(color, *span),
            CssValue::Function { name, args, span } => {
                self.build_value_function_doc(name, args, *span)
            }
            CssValue::SupportsCondition {
                name, condition, ..
            } => self.build_supports_condition_doc(name, condition),
            CssValue::List { values, .. } => self.build_separated_values_doc(values, " "),
            CssValue::CommaSeparated { values, span } => {
                let joined = self.build_separated_values_doc(values, ", ");
                // Joining N elements writes N-1 commas, so an authored closing comma —
                // one that terminated no element — is spelled back here.
                if super::declarations::list_has_closing_comma(
                    self.source,
                    values,
                    span.end_usize(),
                ) {
                    let d = self.d();
                    d.concat(&[joined, d.text(",")])
                } else {
                    joined
                }
            }
        }
    }

    /// Build a doc for an identifier value
    ///
    /// Uses source extraction to preserve escapes, with whitespace normalization
    /// for parenthesized expressions (like calc sub-expressions).
    /// Parenthesized groups like `(100vw - var(--a) - var(--b))` get fill-based
    /// wrapping so they can break at operator boundaries when exceeding print width.
    fn build_identifier_doc(&self, span: Span) -> DocId {
        let d = self.d();
        // The identifier text is recovered from source (escapes preserved verbatim).
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            if !raw.is_empty() {
                // The verbatim case first, asked of the document's word: a value holding no
                // byte the normalizer acts on — the overwhelmingly-common single-token
                // identifier (`red`, `flex`, `100%`), 98.6% of identifier values on a real
                // corpus — is emitted as a zero-allocation `DocText::SourceSpan` (the same
                // source borrow the number / dimension normalizers take) without the
                // normalizer's own byte-at-a-time scan-skip test over the bare slice.
                if value_normalization::normalize_is_noop_in(self.source, span) {
                    return d.source_span(span, self.source);
                }
                // Normalize whitespace for parenthesized expressions
                // (e.g., "(  100%  -  40px  )" → "(100% - 40px)").
                let normalized = value_normalization::normalize_css_whitespace(raw);

                // Parenthesized groups with multiple space-separated tokens get
                // fill-based wrapping so they can break at operator boundaries.
                // Matches prettier's group(indent(fill(parts))) for paren groups.
                if normalized.starts_with('(') && normalized.ends_with(')') {
                    let inner = &normalized[1..normalized.len() - 1];
                    let tokens = value_normalization::split_by_space_preserving_parens(inner);
                    if tokens.len() >= 3 {
                        return self.build_paren_group_doc(&tokens);
                    }
                }

                return match normalized {
                    // Verbatim value the host-word test refused (it holds a control byte the
                    // normalizer keeps, `<VT>` and its kin): normalized == source[span], so
                    // the same zero-allocation `DocText::SourceSpan`. (A verbatim value can never
                    // contain `(`, so it never reaches the paren-group branch above.)
                    Cow::Borrowed(_) => d.source_span(span, self.source),
                    Cow::Owned(s) => d.text_pooled(&s),
                };
            }
        }
        // Empty / whitespace-only span (the empty-identifier sentinel) or an
        // out-of-range span (never in practice — spans index the printer's source):
        // nothing to emit.
        d.text("")
    }

    /// Build a doc for a parenthesized group with fill-based wrapping
    ///
    /// Structure: group("(" indent(softline group(indent(fill(tokens...)))) softline ")")
    /// - Flat: `(a - b - c)`
    /// - Break: `(\n  a - b -\n    c\n)`
    fn build_paren_group_doc(&self, tokens: &[&str]) -> DocId {
        let d = self.d();
        let mut fill_parts = DocBuf::with_capacity(tokens.len() * 2);
        for (i, token) in tokens.iter().enumerate() {
            fill_parts.push(d.text_pooled(token));
            if i < tokens.len() - 1 {
                fill_parts.push(d.line());
            }
        }
        // Inner: group(indent(fill(tokens))) — continuation indent for wrapped lines
        let inner = d.group(d.indent(d.fill(&fill_parts)));
        // Outer: group("(" indent(softline inner) softline ")")
        let open = d.text("(");
        let close = d.text(")");
        d.group(d.concat(&[
            open,
            d.indent(d.concat(&[d.softline(), inner])),
            d.softline(),
            close,
        ]))
    }

    /// Build a doc for a string value
    ///
    /// Source-extracts the raw string so escape sequences are preserved verbatim
    /// (`\a`, `\41`, `\\`, line continuations), normalizing only the quote char
    /// (`"` → `'`) to match prettier. Re-serializing the *decoded* `content` would
    /// corrupt escapes — e.g. emit `\a` as a literal newline (content loss). Mirrors
    /// `build_identifier_doc` and the plain-declaration-value path
    /// (`extract_string_value`); the decoded `content` is only the fallback when the
    /// span is unavailable.
    fn build_string_doc(&self, content: &StringCooked<'_>, span: Span) -> DocId {
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            // The span covers the full literal including quotes (see
            // `parse_string_literal`); strip them and re-emit with quote normalization.
            // The original quote is the first byte of the span (recovered from source,
            // not stored).
            if raw.len() >= 2 && (raw.starts_with('\'') || raw.starts_with('"')) {
                let quote = raw.as_bytes()[0] as char;
                let inner = &raw[1..raw.len() - 1];
                return self.d().text_pooled(&format_string_literal(inner, quote));
            }
        }
        // Fallback: span unavailable (never in practice — spans index the printer's
        // source). Re-emit the decoded content; a `Verbatim` value has no recoverable
        // text without its span, so emit nothing (mirrors `build_identifier_doc`).
        match content {
            StringCooked::Decoded(s) => self.d().text_pooled(&format_string_literal(s, '\'')),
            StringCooked::Verbatim => self.d().text(""),
        }
    }

    /// Build a doc for a dimension value (number + unit)
    ///
    /// Normalizes trailing zeros and adds leading zeros, preserving source
    /// characteristics like leading zeros and signs. An already-canonical
    /// dimension (`10px`, `0.5rem`) borrows its source slice, so it emits a
    /// zero-allocation `source_span`; only a rewritten dimension allocates.
    /// Mirrors the TS literal path (`Printer::build_number_literal_doc`).
    fn build_dimension_doc(&self, span: Span) -> DocId {
        let raw = span.extract(self.source);
        match value_normalization::normalize_dimension_from_source(raw) {
            Cow::Borrowed(_) => self.d().source_span(span, self.source),
            Cow::Owned(s) => self.d().text_pooled(&s),
        }
    }

    /// Build a doc for a color value
    ///
    /// Preserves color syntax (hex, rgb, hsl, etc.) from source.
    fn build_color_doc(&self, color: &crate::ast::internal::Color, span: Span) -> DocId {
        // A verbatim named color comes back `Cow::Borrowed` (== source[span]) and is
        // emitted as a zero-allocation `DocText::SourceSpan`, like the identifier /
        // dimension paths; hex and function syntaxes own their reconstructed text.
        match value_normalization::format_color_from_source(color, self.source, span) {
            Cow::Borrowed(_) => self.d().source_span(span, self.source),
            Cow::Owned(s) => self.d().text_pooled(&s),
        }
    }

    /// Build a flat (non-wrapping) `name(args_doc)` function doc.
    ///
    /// The shared `name(` … `)` envelope for the `url()` and `var(--a,)`
    /// empty-fallback paths, which are kept flat by design (opaque / no break
    /// points). Every other function goes through the wrapping path
    /// (`build_value_function_doc`'s `group(…softline…)` structure) so it can
    /// break when it exceeds width.
    fn flat_function_doc(&self, name: &str, args_doc: DocId) -> DocId {
        let d = self.d();
        d.concat(&[d.text_pooled(name), d.text("("), args_doc, d.text(")")])
    }

    /// Build a doc for a function value with automatic wrapping
    ///
    /// Uses proper doc structure with group/softline/indent so the renderer
    /// decides wrapping based on actual line position (like Prettier). Every
    /// function gets break points (a softline after `(` and before `)`), so a
    /// single over-width arg wraps onto its own line just like a multi-arg list
    /// — matching prettier's `parenthesized-value-group`.
    ///
    /// - Multi-arg functions: wrap each arg on its own line when exceeds width
    /// - Single-arg List (e.g., drop-shadow): wrap on space separators
    /// - Single-arg non-List (e.g., `fn(token)`): wrap the arg onto its own line
    ///   when it exceeds width
    /// - `url()` and the `var(--a,)` empty fallback: kept flat — handled before
    ///   this point
    pub(super) fn build_value_function_doc(
        &self,
        name: &str,
        args: &[CssValue<'_>],
        span: Span,
    ) -> DocId {
        let d = self.d();
        // `url` is opaque whether or not its content was parsed, so it answers first and
        // in one place — the prelude path leaves `@import url(a.css)` unparsed (empty
        // args) while a declaration value parses them, and both want the same verbatim
        // form. Matched ASCII-case-insensitively (css-syntax): `URL(…)` is a url too, so
        // it takes the same path (casing preserved via `span`) rather than generic-function
        // normalization — which would space an interior `/*` in the now-lexed `URL(x/*y)`
        // url-token.
        if name.eq_ignore_ascii_case("url") {
            // Quoted url() — a single string arg. Print it through the normal string
            // path so the quote is normalized (`"x"` → `'x'`), matching prettier.
            if let [arg @ CssValue::String { .. }] = args {
                return self.flat_function_doc(name, self.build_css_value_doc(arg));
            }
            // The same string plus a trailing comment region (`url('a.css' /* c */)`) —
            // still the string path, so the quote still normalizes; the region joins
            // single-spaced, the crate's uniform comment-spacing rule (prettier freezes the
            // authored spacing — conformance_prettier_css.md §CSS: Comments). Kept separate
            // from the arm above rather than folded into it: `join` mints a separator
            // `text` node the single-arg case would never spend, and that case is every
            // `url()` in a stylesheet. A *leading* comment cannot reach here at all — it
            // makes `url(` an opaque `<url-token>`, which lands on the verbatim arm below.
            if let [CssValue::String { .. }, ..] = args {
                return self.flat_function_doc(name, self.build_separated_values_doc(args, " "));
            }
            // Unquoted url() — the content is opaque. Emit the raw source verbatim,
            // stripping only the whitespace right after `url(` and right before `)`
            // (prettier's `printer-postcss.js` url handling). Rejoining parsed args
            // would drop empty/trailing comma segments (`url(a,b,)` → `url(a,b)`),
            // silently changing the URL — the comma is part of the resource ref — so
            // raw text wins over the args even when it isn't parenthesized at all (which
            // a parsed function's span cannot be; the arm is defensive).
            //
            // Deliberately pooled, not `source_span`: this arm is hot on CSS corpora
            // (every `url(...)`) and the span form's render-time resolution hop measured
            // +0.07% instructions there for no allocation win (the pool is amortized).
            // TODO: re-measure that verdict — it predates the render's inlined `resolve_text`,
            // and the selector leaf (`span_leaf_doc`, ~12-byte slices) since read −0.34% as a
            // `source_span` with no hop visible; a url is longer still. The `Borrowed` arm of
            // `trim_url_raw` is the document's own bytes and would take the span form directly.
            if span.end_usize() <= self.source.len() {
                let raw = span.extract(self.source);
                return match crate::url::trim_url_raw(raw) {
                    Some(trimmed) => d.text_pooled(&trimmed),
                    None => d.text_pooled(raw),
                };
            }
            // Fallback (span unavailable): rejoin args with no space after commas.
            let args_doc = d.join(args.iter().map(|arg| self.build_css_value_doc(arg)), ",");
            return self.flat_function_doc(name, args_doc);
        }

        // A function whose grammar tsv doesn't read parsed no args (`scope((.a) to (.b))`);
        // it stays verbatim.
        if args.is_empty() && span.end_usize() <= self.source.len() {
            return d.text_pooled(span.extract(self.source));
        }

        // A comma **closing** the argument list (`rgb(1, 2, 3,)`, `var(--a,)`,
        // `linear-gradient(red, ,)`) terminated no argument, so joining the args would
        // drop it. `extract_function_parts` requires the closing paren to be the value's
        // last byte, which is what bounds the list at `span.end - 1`.
        let closing_comma = super::declarations::list_has_closing_comma(
            self.source,
            args,
            span.end_usize().saturating_sub(1),
        );

        // var()'s empty fallback (`var(--a,)`, `var(--a, ,)`) is kept flat: the generic
        // path below spells the same closing comma, but wraps the argument list in a
        // breakable group, and prettier never breaks a `var()`. `var(--a, red)` has no
        // closing comma and takes the generic path with the normal `, ` separator.
        if name.eq_ignore_ascii_case("var") && closing_comma {
            let args_doc = d.join(args.iter().map(|arg| self.build_css_value_doc(arg)), ", ");
            let comma = d.text(",");
            return self.flat_function_doc(name, d.concat(&[args_doc, comma]));
        }

        // Build with group/softline structure for automatic wrapping
        // Structure: name(
        //   arg1,
        //   arg2,
        //   arg3
        // )
        // When flat: name(arg1, arg2, arg3)
        let mut inner_parts = DocBuf::new();
        for (i, arg) in args.iter().enumerate() {
            // For List args (space-separated values like calc math expressions),
            // use fill with line() separators so content can break at operators.
            // Matches prettier's group(indent(fill(parts))) pattern.
            if let CssValue::List { values, .. } = arg {
                inner_parts.push(self.build_space_fill_value_doc(values));
            } else {
                inner_parts.push(self.build_css_value_doc(arg));
            }
            if i < args.len() - 1 {
                inner_parts.push(d.text(","));
                inner_parts.push(d.line()); // space when flat, newline when broken
            }
        }
        // The closing comma the args' own separators can't spell: `red, ` is a ONE-argument
        // list, so `linear-gradient(red,,)` would lose its empty argument — and with it the
        // reason the UA drops the declaration — while `rgb(1, 2, 3,)` would lose the comma
        // that makes it invalid in the first place. `var()` takes its own flat form above.
        // The comma joins the last argument rather than standing as its own part, so the
        // group's break can never strand it on a line of its own.
        if closing_comma && let Some(last) = inner_parts.pop() {
            inner_parts.push(d.concat(&[last, d.text(",")]));
        }

        let name_doc = d.text_pooled(name);
        let inner = d.concat(&inner_parts);
        d.group(d.concat(&[
            name_doc,
            d.text("("),
            d.indent(d.concat(&[d.softline(), inner])),
            d.softline(),
            d.text(")"),
        ]))
    }

    /// Build a doc for space-separated values inside a function argument
    ///
    /// Uses fill with line() separators so the renderer can break at space boundaries
    /// when content exceeds print width. Wrapped in group(indent(fill(...))) to match
    /// prettier's CSS value group pattern — continuation lines get extra indent.
    ///
    /// Example: `calc(0.5 * (100vw - var(--a)))` breaks as:
    /// ```text
    /// calc(
    ///   0.5 *
    ///     (100vw - var(--a))
    /// )
    /// ```
    pub(super) fn build_space_fill_value_doc(&self, values: &[CssValue<'_>]) -> DocId {
        let d = self.d();
        let parts = self.build_space_fill_parts(values);
        d.group(d.indent(d.fill(&parts)))
    }

    /// Build a doc for a value list joined by `sep` — `" "` for a space-separated
    /// list (`CssValue::List`), `", "` for a comma-separated one
    /// (`CssValue::CommaSeparated`).
    pub(crate) fn build_separated_values_doc(
        &self,
        values: &[CssValue<'_>],
        sep: &'static str,
    ) -> DocId {
        self.d()
            .join(values.iter().map(|v| self.build_css_value_doc(v)), sep)
    }
}
