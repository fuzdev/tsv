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

use super::{Printer, source_fidelity};
use crate::ast::internal::CssValue;
use tsv_lang::{Span, doc};

impl<'a> Printer<'a> {
    /// Format a nested value (function arg, list item)
    ///
    /// Uses the doc builder for most values. Functions are handled specially
    /// to check if they need wrapping (when they exceed print width).
    pub(super) fn print_nested_value(&mut self, value: &CssValue) {
        // Functions need special handling for wrapping
        if let CssValue::Function { name, args, .. } = value {
            self.print_function_value(name, args);
            return;
        }
        // All other values use doc builder
        let doc = self.build_css_value_doc(value);
        self.write_doc(&doc);
    }

    /// Print a CSS function value with optional wrapping
    ///
    /// Wraps function arguments when they exceed print width.
    fn print_function_value(&mut self, name: &str, args: &[CssValue]) {
        if self.should_wrap_function(name, args) {
            // Wrap function arguments
            self.write(name);
            self.write("(\n");
            self.indent_level += 1;
            for (i, arg) in args.iter().enumerate() {
                self.write_indent();
                self.print_nested_value(arg);
                if i < args.len() - 1 {
                    self.write(",\n");
                }
            }
            self.indent_level -= 1;
            self.write("\n");
            self.write_indent();
            self.write(")");
        } else {
            // Inline function (no wrapping)
            let doc = self.build_value_function_doc(name, args, Span::new(0, 0));
            self.write_doc(&doc);
        }
    }

    /// Format a CSS value (right-hand side of declaration)
    ///
    /// Uses the doc builder which handles source fidelity and proper formatting.
    pub(super) fn print_css_value(&mut self, value: &CssValue) {
        let doc = self.build_css_value_doc(value);
        self.write_doc(&doc);
    }

    // ========================================================================
    // Doc Builders - all formatting logic expressed as doc IR
    // ========================================================================

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
                let normalized = Self::normalize_whitespace(raw);
                return doc::text_owned(normalized);
            }
        }
        // Fallback: semantic formatting
        let formatted = source_fidelity::format_identifier_value(name);
        doc::text_owned(formatted)
    }

    /// Normalize whitespace in extracted source text
    ///
    /// Single-pass normalization that:
    /// - Collapses consecutive whitespace to single spaces
    /// - Removes spaces after opening parentheses: `( expr` → `(expr`
    /// - Removes spaces before closing parentheses: `expr )` → `expr)`
    /// - Preserves all whitespace inside quoted strings
    ///
    /// This matches prettier's normalization behavior for calc() and other functions.
    fn normalize_whitespace(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        let mut in_string = false;
        let mut string_delim = '\0';
        let mut prev_was_whitespace = false;

        while let Some(ch) = chars.next() {
            match ch {
                // String delimiter handling
                '\'' | '"' if !in_string => {
                    in_string = true;
                    string_delim = ch;
                    result.push(ch);
                    prev_was_whitespace = false;
                }
                c if in_string && c == string_delim => {
                    in_string = false;
                    result.push(ch);
                    prev_was_whitespace = false;
                }
                _ if in_string => {
                    // Inside string - preserve everything
                    result.push(ch);
                    prev_was_whitespace = false;
                }
                // Opening paren - skip following whitespace
                '(' if !in_string => {
                    result.push(ch);
                    // Skip all following whitespace
                    while let Some(&next) = chars.peek() {
                        if next.is_whitespace() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    prev_was_whitespace = false;
                }
                // Closing paren - remove trailing whitespace
                ')' if !in_string => {
                    while result.ends_with(|c: char| c.is_whitespace()) {
                        result.pop();
                    }
                    result.push(ch);
                    prev_was_whitespace = false;
                }
                // Whitespace - collapse consecutive
                ' ' | '\n' | '\t' | '\r' if !in_string => {
                    if !prev_was_whitespace {
                        result.push(' ');
                        prev_was_whitespace = true;
                    }
                }
                // Regular character
                _ => {
                    result.push(ch);
                    prev_was_whitespace = false;
                }
            }
        }

        result.trim().to_string()
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

    /// Build a doc for a function value (inline)
    ///
    /// Note: This builds the inline representation. Wrapped functions are
    /// handled by the declaration printer which adds newlines and indentation.
    fn build_value_function_doc(&self, name: &str, args: &[CssValue], span: Span) -> doc::Doc {
        // For functions with no parsed args (like supports()), extract from source
        if args.is_empty() && span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);
            return doc::text_owned(raw.to_string());
        }

        // WORKAROUND: url() data URIs contain commas that our parser incorrectly treats as
        // argument separators. Use no space after commas to preserve data URI format.
        let is_url = name == "url";
        let separator = if is_url { "," } else { ", " };

        let arg_docs: Vec<_> = args
            .iter()
            .map(|arg| self.build_css_value_doc(arg))
            .collect();
        let args_doc = doc::join(arg_docs, separator);

        doc::concat(vec![
            doc::text_owned(name.to_string()),
            doc::text("("),
            args_doc,
            doc::text(")"),
        ])
    }

    /// Build a doc for space-separated values
    fn build_space_separated_doc(&self, values: &[CssValue]) -> doc::Doc {
        let docs: Vec<_> = values.iter().map(|v| self.build_css_value_doc(v)).collect();
        doc::join(docs, " ")
    }

    /// Build a doc for comma-separated values
    fn build_comma_separated_doc(&self, values: &[CssValue]) -> doc::Doc {
        let docs: Vec<_> = values.iter().map(|v| self.build_css_value_doc(v)).collect();
        doc::join(docs, ", ")
    }
}
