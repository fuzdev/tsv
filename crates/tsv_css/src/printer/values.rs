//! CSS value printing
//!
//! Handles printing of all CSS value types:
//! - Simple values (identifiers, strings, dimensions, colors)
//! - Compound values (lists, functions)
//! - Semantic formatting with source fidelity

use super::{Printer, source_fidelity};
use crate::ast::internal::CssValue;
use tsv_lang::{PrintConfig, Span};

impl<'a> Printer<'a> {
    pub(super) fn value_to_string(&self, value: &CssValue) -> String {
        use tsv_lang::OutputBuffer;

        let buffer = OutputBuffer::new();
        // Use a very large print width to prevent any wrapping during width calculation
        let no_wrap_config = PrintConfig {
            print_width: 10000,
            ..self.config
        };
        let mut temp_printer = Printer {
            buffer,
            source: self.source,
            indent_level: 0, // Don't include indentation in width calculation
            config: no_wrap_config,
            comments: self.comments,
        };
        // Use semantic printing to avoid source extraction (which includes original formatting)
        temp_printer.print_css_value_semantic(value);
        temp_printer.buffer.into_string()
    }

    /// Format a nested value (function arg, list item)
    ///
    /// Tries source extraction first (spans are accurate from ValueParser).
    /// Normalizes formatting whitespace while preserving source fidelity.
    ///
    /// This enables preserving source fidelity for nested values (leading zeros, etc.)
    /// even with multiline input.
    ///
    /// Functions and lists are NOT extracted from source - they're formatted semantically
    /// to ensure correct spacing normalization.
    pub(super) fn print_nested_value(&mut self, value: &CssValue) {
        // Functions, composite values, colors, dimensions, and strings should be formatted semantically
        // to normalize their internal spacing, decimal representation, and quote style
        // (e.g., `rgba(0,0,0,0.1)` → `rgba(0, 0, 0, 0.1)`, `45.0deg` → `45deg`, `url("x")` → `url('x')`)
        match value {
            CssValue::Function { .. }
            | CssValue::List { .. }
            | CssValue::CommaSeparated { .. }
            | CssValue::Color { .. }
            | CssValue::Dimension { .. }
            | CssValue::String { .. } => {
                self.print_css_value_semantic(value);
                return;
            }
            _ => {}
        }

        let span = value.span();

        // Try source extraction (spans are now accurate from ValueParser!)
        if span.end_usize() <= self.source.len() {
            let raw = span.extract(self.source);

            if !raw.is_empty() {
                // Normalize formatting whitespace while preserving source fidelity
                // Collapse '\n', '\t', '\r' to ' ' but preserve content
                let normalized = self.normalize_whitespace(raw);
                self.write(&normalized);
            } else {
                // Empty value - use semantic formatting
                self.print_css_value_semantic(value);
            }
        } else {
            // Fallback: span is invalid, use semantic formatting
            self.print_css_value_semantic(value);
        }
    }

    /// Normalize whitespace in extracted source text
    ///
    /// Single-pass normalization that:
    /// - Collapses consecutive whitespace (including \n, \t, \r) to single spaces
    /// - Removes spaces after opening parentheses: `( expr` → `(expr`
    /// - Removes spaces before closing parentheses: `expr )` → `expr)`
    /// - Preserves all whitespace inside quoted strings
    ///
    /// This matches prettier's normalization behavior for calc() and other functions.
    fn normalize_whitespace(&self, s: &str) -> String {
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

    /// Format a CSS value semantically (from AST, no source extraction)
    ///
    /// Used as fallback when source extraction is not possible.
    /// Always formats from AST structure, never extracts from source.
    pub(super) fn print_css_value_semantic(&mut self, value: &CssValue) {
        match value {
            CssValue::Identifier { name, .. } => {
                let formatted = source_fidelity::format_identifier_value(name);
                self.write(&formatted);
            }
            CssValue::String { content, quote, .. } => {
                let formatted = source_fidelity::format_string_value(content, *quote);
                self.write(&formatted);
            }
            CssValue::Dimension { span, .. } => {
                self.print_dimension(*span);
            }
            CssValue::Color { color, span } => {
                // Extract and reformat with syntax preservation
                let formatted =
                    source_fidelity::format_color_from_source(color, self.source, *span);
                self.write(&formatted);
            }
            CssValue::Function { name, args, span } => {
                // For functions with no parsed args (like supports()), extract from source
                if args.is_empty() && span.end_usize() <= self.source.len() {
                    let raw = span.extract(self.source);
                    self.write(raw);
                } else {
                    self.print_function_value(name, args);
                }
            }
            CssValue::List { values, .. } => {
                self.print_space_separated_values(values);
            }
            CssValue::CommaSeparated { values, .. } => {
                self.print_comma_separated_values(values);
            }
        }
    }

    /// Print a dimension value using source-based normalization
    ///
    /// This preserves leading zeros (01.5px), signs (+10px, -0px), while normalizing
    /// trailing zeros (1.50px → 1.5px) and adding leading zeros (.5px → 0.5px).
    /// Matches prettier's exact behavior.
    fn print_dimension(&mut self, span: Span) {
        let raw = span.extract(self.source);
        let normalized = source_fidelity::normalize_dimension_from_source(raw);
        self.write(&normalized);
    }

    /// Print space-separated values
    fn print_space_separated_values(&mut self, values: &[CssValue]) {
        for (i, val) in values.iter().enumerate() {
            if i > 0 {
                self.write(" ");
            }
            self.print_nested_value(val);
        }
    }

    /// Print comma-separated values
    fn print_comma_separated_values(&mut self, values: &[CssValue]) {
        for (i, val) in values.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_nested_value(val);
        }
    }

    /// Print a CSS function value with optional wrapping
    ///
    /// Shared helper for `print_css_value` and `print_css_value_semantic`.
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
            self.write(name);
            self.write("(");
            // WORKAROUND: url() data URIs contain commas that our parser incorrectly treats as
            // argument separators. Use no space after commas to preserve data URI format.
            let is_url = name == "url";
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    if is_url {
                        self.write(",");
                    } else {
                        self.write(", ");
                    }
                }
                self.print_nested_value(arg);
            }
            self.write(")");
        }
    }

    /// Format a CSS value (right-hand side of declaration)
    pub(super) fn print_css_value(&mut self, value: &CssValue) {
        match value {
            CssValue::Identifier { name, .. } => {
                self.write(name);
            }
            CssValue::String {
                content,
                quote,
                span,
            } => {
                // Extract raw source to preserve escape sequences
                let raw = span.extract(self.source);

                // Use raw source if it looks valid (starts and ends with quotes)
                if raw.starts_with(*quote) && raw.ends_with(*quote) {
                    self.write(raw);
                } else {
                    // Fallback: format from decoded content
                    let formatted = source_fidelity::format_string_value(content, *quote);
                    self.write(&formatted);
                }
            }
            CssValue::Dimension { span, .. } => {
                self.print_dimension(*span);
            }
            CssValue::Color { color, span } => {
                // Extract and reformat with syntax preservation
                let formatted =
                    source_fidelity::format_color_from_source(color, self.source, *span);
                self.write(&formatted);
            }
            CssValue::Function { name, args, .. } => {
                self.print_function_value(name, args);
            }
            CssValue::List { values, .. } => {
                self.print_space_separated_values(values);
            }
            CssValue::CommaSeparated { values, .. } => {
                self.print_comma_separated_values(values);
            }
        }
    }
}
