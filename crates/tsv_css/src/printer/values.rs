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

use super::{Printer, has_wrappable_args, source_fidelity};
use crate::ast::internal::CssValue;
use tsv_lang::{Span, doc};

impl<'a> Printer<'a> {
    /// Format a CSS value
    ///
    /// Uses the doc builder which handles source fidelity and proper formatting.
    pub(super) fn print_css_value(&mut self, value: &CssValue) {
        let doc = self.build_css_value_doc(value);
        self.write_doc(&doc);
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
    pub(super) fn build_css_value_doc(&self, value: &CssValue) -> doc::Doc {
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
            CssValue::List { values, .. } => self.build_space_separated_doc(values),
            CssValue::CommaSeparated { values, .. } => self.build_comma_separated_doc(values),
        }
    }

    /// Build a doc for an identifier value
    ///
    /// Uses source extraction to preserve escapes, with whitespace normalization
    /// for parenthesized expressions (like calc sub-expressions).
    fn build_identifier_doc(&self, name: &str, span: Span) -> doc::Doc {
        // Try source extraction first to preserve any escapes
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            if !raw.is_empty() {
                // Normalize whitespace for parenthesized expressions
                // (e.g., "(  100%  -  40px  )" → "(100% - 40px)")
                let normalized = source_fidelity::normalize_css_whitespace(raw);
                return doc::text_owned(normalized);
            }
        }
        // Fallback: semantic formatting
        let formatted = source_fidelity::format_identifier_value(name);
        doc::text_owned(formatted)
    }

    /// Build a doc for a string value
    ///
    /// Always uses semantic formatting to normalize quotes (double → single).
    /// Prettier prefers single quotes in CSS strings for consistency.
    fn build_string_doc(&self, content: &str, quote: char, _span: Span) -> doc::Doc {
        // Use semantic formatting which normalizes quotes (prefers single quotes)
        let formatted = source_fidelity::format_string_value(content, quote);
        doc::text_owned(formatted)
    }

    /// Build a doc for a dimension value (number + unit)
    ///
    /// Normalizes trailing zeros and adds leading zeros, preserving source
    /// characteristics like leading zeros and signs.
    fn build_dimension_doc(&self, span: Span) -> doc::Doc {
        let raw = span.extract(self.source);
        let normalized = source_fidelity::normalize_dimension_from_source(raw);
        doc::text_owned(normalized)
    }

    /// Build a doc for a color value
    ///
    /// Preserves color syntax (hex, rgb, hsl, etc.) from source.
    fn build_color_doc(&self, color: &crate::ast::internal::Color, span: Span) -> doc::Doc {
        let formatted = source_fidelity::format_color_from_source(color, self.source, span);
        doc::text_owned(formatted)
    }

    /// Build a doc for a function value with automatic wrapping
    ///
    /// Uses proper doc structure with group/softline/indent so the renderer
    /// decides wrapping based on actual line position (like Prettier).
    ///
    /// - Multi-arg functions: wrap each arg on its own line when exceeds width
    /// - Single-arg List (e.g., drop-shadow): wrap on space separators
    /// - Single-arg non-List (e.g., url): never wraps
    fn build_value_function_doc(&self, name: &str, args: &[CssValue], span: Span) -> doc::Doc {
        // For functions with no parsed args (like supports()), extract from source
        if args.is_empty() && span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            return doc::text_owned(raw.to_string());
        }

        // WORKAROUND: url() data URIs contain commas that our parser incorrectly treats as
        // argument separators. Use no space after commas to preserve data URI format.
        // url() also never wraps since it has no natural break points.
        let is_url = name == "url";
        if is_url {
            return doc::concat(vec![
                doc::text_owned(name.to_string()),
                doc::text("("),
                doc::join(args.iter().map(|arg| self.build_css_value_doc(arg)), ","),
                doc::text(")"),
            ]);
        }

        if !has_wrappable_args(args) {
            // Single simple arg - inline only, no break points
            return doc::concat(vec![
                doc::text_owned(name.to_string()),
                doc::text("("),
                doc::join(args.iter().map(|arg| self.build_css_value_doc(arg)), ", "),
                doc::text(")"),
            ]);
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
            inner_parts.push(self.build_css_value_doc(arg));
            if i < args.len() - 1 {
                inner_parts.push(doc::text(","));
                inner_parts.push(doc::line()); // space when flat, newline when broken
            }
        }

        doc::group(doc::concat(vec![
            doc::text_owned(name.to_string()),
            doc::text("("),
            doc::indent(doc::concat(vec![
                doc::softline(), // nothing when flat, newline when broken
                doc::concat(inner_parts),
            ])),
            doc::softline(), // nothing when flat, newline when broken
            doc::text(")"),
        ]))
    }

    /// Build a doc for space-separated values
    fn build_space_separated_doc(&self, values: &[CssValue]) -> doc::Doc {
        doc::join(values.iter().map(|v| self.build_css_value_doc(v)), " ")
    }

    /// Build a doc for comma-separated values
    fn build_comma_separated_doc(&self, values: &[CssValue]) -> doc::Doc {
        doc::join(values.iter().map(|v| self.build_css_value_doc(v)), ", ")
    }
}
