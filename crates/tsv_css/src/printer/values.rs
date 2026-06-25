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

use super::{Printer, has_wrappable_args, value_normalization};
use crate::ast::internal::CssValue;
use tsv_lang::Span;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Format a CSS value
    ///
    /// Uses the doc builder which handles source fidelity and proper formatting.
    pub(super) fn print_css_value(&mut self, value: &CssValue) {
        let doc = self.build_css_value_doc(value);
        self.write_arena_doc(doc);
    }

    /// Format a nested value (function arg, list item)
    ///
    /// Alias for `print_css_value` - kept for semantic clarity in call sites.
    #[inline]
    pub(super) fn print_nested_value(&mut self, value: &CssValue) {
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
    pub(super) fn build_css_value_doc(&self, value: &CssValue) -> DocId {
        match value {
            CssValue::Identifier { name, span } => self.build_identifier_doc(name, *span),
            CssValue::String {
                content,
                quote,
                span,
            } => self.build_string_doc(content, *quote, *span),
            CssValue::Dimension { span, .. } => self.build_dimension_doc(*span),
            CssValue::Color { color, span } => self.build_color_doc(color, *span),
            CssValue::Function { name, args, span } => {
                self.build_value_function_doc(name, args, *span)
            }
            CssValue::List { values, .. } => self.build_separated_values_doc(values, " "),
            CssValue::CommaSeparated { values, .. } => {
                self.build_separated_values_doc(values, ", ")
            }
        }
    }

    /// Build a doc for an identifier value
    ///
    /// Uses source extraction to preserve escapes, with whitespace normalization
    /// for parenthesized expressions (like calc sub-expressions).
    /// Parenthesized groups like `(100vw - var(--a) - var(--b))` get fill-based
    /// wrapping so they can break at operator boundaries when exceeding print width.
    fn build_identifier_doc(&self, name: &str, span: Span) -> DocId {
        let d = self.d();
        // Try source extraction first to preserve any escapes
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            if !raw.is_empty() {
                // Normalize whitespace for parenthesized expressions
                // (e.g., "(  100%  -  40px  )" → "(100% - 40px)")
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

                return d.text_owned(normalized);
            }
        }
        // Fallback: semantic formatting
        let formatted = value_normalization::format_identifier_value(name);
        d.text_owned(formatted)
    }

    /// Build a doc for a parenthesized group with fill-based wrapping
    ///
    /// Structure: group("(" indent(softline group(indent(fill(tokens...)))) softline ")")
    /// - Flat: `(a - b - c)`
    /// - Break: `(\n  a - b -\n    c\n)`
    fn build_paren_group_doc(&self, tokens: &[&str]) -> DocId {
        let d = self.d();
        let mut fill_parts = Vec::with_capacity(tokens.len() * 2);
        for (i, token) in tokens.iter().enumerate() {
            fill_parts.push(d.text_owned(token.to_string()));
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
    /// (`"` → `'`) to match prettier. The internal `content` is fully *decoded*
    /// (`parse_string_literal`), so re-serializing it would corrupt escapes — e.g.
    /// emit `\a` as a literal newline (content loss). Mirrors `build_identifier_doc`
    /// and the plain-declaration-value path (`extract_string_value`); the decoded
    /// `content` is only the fallback when the span is unavailable.
    fn build_string_doc(&self, content: &str, quote: char, span: Span) -> DocId {
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            // The span covers the full literal including quotes (see
            // `parse_string_literal`); strip them and re-emit with quote normalization.
            if raw.len() >= 2 && (raw.starts_with('\'') || raw.starts_with('"')) {
                let inner = &raw[1..raw.len() - 1];
                return self
                    .d()
                    .text_owned(value_normalization::format_string_value(inner, quote));
            }
        }
        // Fallback: semantic formatting from decoded content (span unavailable)
        let formatted = value_normalization::format_string_value(content, quote);
        self.d().text_owned(formatted)
    }

    /// Build a doc for a dimension value (number + unit)
    ///
    /// Normalizes trailing zeros and adds leading zeros, preserving source
    /// characteristics like leading zeros and signs.
    fn build_dimension_doc(&self, span: Span) -> DocId {
        let raw = span.extract(self.source);
        let normalized = value_normalization::normalize_dimension_from_source(raw);
        self.d().text_owned(normalized)
    }

    /// Build a doc for a color value
    ///
    /// Preserves color syntax (hex, rgb, hsl, etc.) from source.
    fn build_color_doc(&self, color: &crate::ast::internal::Color, span: Span) -> DocId {
        let formatted = value_normalization::format_color_from_source(color, self.source, span);
        self.d().text_owned(formatted)
    }

    /// Build a doc for a function value with automatic wrapping
    ///
    /// Uses proper doc structure with group/softline/indent so the renderer
    /// decides wrapping based on actual line position (like Prettier).
    ///
    /// - Multi-arg functions: wrap each arg on its own line when exceeds width
    /// - Single-arg List (e.g., drop-shadow): wrap on space separators
    /// - Single-arg non-List (e.g., url): never wraps
    fn build_value_function_doc(&self, name: &str, args: &[CssValue], span: Span) -> DocId {
        let d = self.d();
        // For functions with no parsed args (like supports()), extract from source
        if args.is_empty() && span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            // An unparsed `url(...)` reaches here when the prelude path leaves its opaque
            // content unparsed (e.g. `@import url(a.css)`). Trim only the whitespace inside
            // the parens to match prettier, exactly like the parsed-args url path below.
            // Other empty-args functions (`supports(...)`) stay verbatim.
            if name.eq_ignore_ascii_case("url")
                && let Some(trimmed) = crate::url::trim_url_raw(raw)
            {
                return d.text_owned(trimmed);
            }
            return d.text_owned(raw.to_string());
        }

        if name == "url" {
            // Quoted url() — a single string arg. Print it through the normal string
            // path so the quote is normalized (`"x"` → `'x'`), matching prettier.
            if let [arg @ CssValue::String { .. }] = args {
                let name_doc = d.text_owned(name.to_string());
                let open = d.text("(");
                let args_doc = self.build_css_value_doc(arg);
                let close = d.text(")");
                return d.concat(&[name_doc, open, args_doc, close]);
            }
            // Unquoted url() — the content is opaque. Emit the raw source verbatim,
            // stripping only the whitespace right after `url(` and right before `)`
            // (prettier's `printer-postcss.js` url handling). Rejoining parsed args
            // would drop empty/trailing comma segments (`url(a,b,)` → `url(a,b)`),
            // silently changing the URL — the comma is part of the resource ref.
            if span.end_usize() <= self.source.len()
                && let Some(raw) = crate::url::trim_url_raw(span.extract(self.source))
            {
                return d.text_owned(raw);
            }
            // Fallback (span unavailable): rejoin args with no space after commas.
            let name_doc = d.text_owned(name.to_string());
            let open = d.text("(");
            let args_doc = d.join(args.iter().map(|arg| self.build_css_value_doc(arg)), ",");
            let close = d.text(")");
            return d.concat(&[name_doc, open, args_doc, close]);
        }

        // var() empty fallback: `var(--a,)` — the trailing comma is kept with no space
        // after it. The empty fallback is the final empty-identifier arg (see the parser's
        // var-specific handling). `var(--a, red)` keeps the normal `, ` separator.
        if name.eq_ignore_ascii_case("var")
            && args.len() >= 2
            && matches!(args.last(), Some(CssValue::Identifier { name: n, .. }) if n.is_empty())
        {
            let name_doc = d.text_owned(name.to_string());
            let open = d.text("(");
            let real = &args[..args.len() - 1];
            let args_doc = d.join(real.iter().map(|arg| self.build_css_value_doc(arg)), ", ");
            let comma = d.text(",");
            let close = d.text(")");
            return d.concat(&[name_doc, open, args_doc, comma, close]);
        }

        if !has_wrappable_args(args) {
            // Single simple arg - inline only, no break points
            let name_doc = d.text_owned(name.to_string());
            let open = d.text("(");
            let args_doc = d.join(args.iter().map(|arg| self.build_css_value_doc(arg)), ", ");
            let close = d.text(")");
            return d.concat(&[name_doc, open, args_doc, close]);
        }

        // Build with group/softline structure for automatic wrapping
        // Structure: name(
        //   arg1,
        //   arg2,
        //   arg3
        // )
        // When flat: name(arg1, arg2, arg3)
        let mut inner_parts = Vec::new();
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

        let name_doc = d.text_owned(name.to_string());
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
    fn build_space_fill_value_doc(&self, values: &[CssValue]) -> DocId {
        let d = self.d();
        let parts = self.build_space_fill_parts(values);
        d.group(d.indent(d.fill(&parts)))
    }

    /// Build a doc for a value list joined by `sep` — `" "` for a space-separated
    /// list (`CssValue::List`), `", "` for a comma-separated one
    /// (`CssValue::CommaSeparated`).
    pub(crate) fn build_separated_values_doc(
        &self,
        values: &[CssValue],
        sep: &'static str,
    ) -> DocId {
        self.d()
            .join(values.iter().map(|v| self.build_css_value_doc(v)), sep)
    }
}
