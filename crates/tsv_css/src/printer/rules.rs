// CSS rule and declaration formatting
//
// Handles formatting of:
// - CSS rules (selector + declarations block)
// - CSS declarations (property: value;)
//
// Selector formatting is handled by the selectors module.

use super::{source_fidelity, Printer};
use crate::ast::internal::{self, CssValue};

impl<'a> Printer<'a> {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn print_css_rule(&mut self, rule: &internal::CssRule) {
        // Format selector (uses selectors module)
        self.print_selector_list(&rule.selector);

        // Check if first child is a comment between selector and opening brace
        let mut start_index = 0;
        if let Some(internal::CssBlockChild::Comment(comment)) = rule.declarations.first() {
            // Check if comment is before the opening brace (between selector and {)
            // block_span.start is the position of the opening brace
            if comment.span.start < rule.block_span.start {
                // Comment is between selector and brace - print inline
                // Always add space before comment for readability (normalize)
                // This is an intentional divergence from prettier (which preserves no-space)
                self.write(" /*");
                self.write(&comment.content);
                self.write("*/");
                start_index = 1; // Skip this comment when processing declarations
            }
        }

        self.write(" {\n");

        // Format declarations and comments with indentation
        self.indent_level += 1;
        let mut i = start_index;
        while i < rule.declarations.len() {
            let child = &rule.declarations[i];
            match child {
                internal::CssBlockChild::Declaration(decl) => {
                    self.print_css_declaration(decl);

                    // Check if next child is an inline comment
                    if let Some(internal::CssBlockChild::Comment(next_comment)) =
                        rule.declarations.get(i + 1)
                        && self.is_same_line(decl.span.end, next_comment.span.start) {
                            // Print comment inline (backspace to remove the newline)
                            // Note: We need to remove the trailing \n from declaration
                            self.buffer_remove_trailing_newline();
                            self.write(" /*");
                            self.write(&next_comment.content);
                            self.write("*/\n");
                            i += 1; // Skip the comment in the next iteration
                        }
                }
                internal::CssBlockChild::Comment(comment) => {
                    // Standalone comment (not inline after a declaration)
                    self.write_indent();
                    self.write("/*");
                    self.write(&comment.content);
                    self.write("*/\n");
                }
                internal::CssBlockChild::Rule(_) | internal::CssBlockChild::Atrule(_) => {
                    // Nested rules not expected in regular CSS rules
                    // (only in at-rules like @media)
                }
            }
            i += 1;
        }
        self.indent_level -= 1;

        // Closing brace
        self.write("}");
    }


    /// Check if a property should use multiline formatting
    fn should_use_multiline(&self, decl: &internal::CssDeclaration) -> bool {
        matches!(decl.property.as_str(), "box-shadow" | "text-shadow")
            && matches!(&decl.value, CssValue::CommaSeparated { values, .. } if values.len() > 1)
    }


    /// Format a CSS declaration (property: value;)
    pub(super) fn print_css_declaration(&mut self, decl: &internal::CssDeclaration) {
        self.write_indent();

        // Extract property name from source to preserve escape sequences
        // See: docs/SVELTE_COMPATIBILITY.md (CSS Quirks section)
        let decl_source = &self.source[decl.span.start as usize..decl.span.end as usize];
        let property_normalized = source_fidelity::extract_property_name(decl_source);

        // Write property name (normalized with spaces around comments)
        self.write(&property_normalized);

        // Check if property needs multiline formatting
        if self.should_use_multiline(decl) {
            self.write(":\n");
            self.indent_level += 1;
            self.print_css_value_multiline(&decl.value);
            self.indent_level -= 1;
            self.write(";\n");
        } else if self.value_comments.contains_key(&decl.span.start) {
            // Value has comments - extract from source to preserve them
            if let Some(normalized) = source_fidelity::extract_value_with_comments(decl_source) {
                self.write(": ");
                self.write(&normalized);
                self.write(";\n");
            } else {
                // Fallback: shouldn't happen
                self.write(": ");
                self.write(decl_source);
                self.write(";\n");
            }
        } else if let CssValue::String { quote, .. } = &decl.value {
            // String values: extract from source to preserve escapes
            if let Some(formatted) = source_fidelity::extract_string_value(decl_source, *quote) {
                self.write(": ");
                self.write(&formatted);
                self.write(";\n");
            } else {
                // Fallback: use semantic formatting
                self.write(": ");
                let formatted = source_fidelity::format_string_value("", *quote);
                self.write(&formatted);
                self.write(";\n");
            }
        } else {
            // All other values: use standard formatting
            // Property with comment: `color /* comment */` → ` : ` → `color /* comment */ : `
            // Property without comment: `color` → `: ` → `color: `
            if property_normalized.contains("/*") {
                self.write(" : ");
            } else {
                self.write(": ");
            }
            self.print_css_value(&decl.value);
            self.write(";\n");
        }
    }


    /// Format a CSS value on multiple lines (for shadow properties)
    ///
    /// Used for properties like box-shadow and text-shadow that should format
    /// comma-separated values on multiple lines for readability.
    ///
    /// Uses hybrid formatting: preserves source fidelity for leaf values (dimensions, strings)
    /// while normalizing spacing for composite structures (functions, lists).
    fn print_css_value_multiline(&mut self, value: &CssValue) {
        match value {
            CssValue::CommaSeparated { values, .. } => {
                for (i, val) in values.iter().enumerate() {
                    self.write_indent();
                    self.print_nested_value(val);  // Use nested_value for normalization
                    if i < values.len() - 1 {
                        self.write(",\n");
                    }
                }
            }
            _ => {
                // Fallback to regular formatting
                self.print_nested_value(value);  // Use nested_value for normalization
            }
        }
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
    fn print_nested_value(&mut self, value: &CssValue) {
        // Functions, composite values, and colors should be formatted semantically
        // to normalize their internal spacing (e.g., `rgba(0,0,0,0.1)` → `rgba(0, 0, 0, 0.1)`)
        match value {
            CssValue::Function { .. } | CssValue::List { .. } | CssValue::CommaSeparated { .. } | CssValue::Color { .. } => {
                self.print_css_value_semantic(value);
                return;
            }
            _ => {}
        }

        let span = value.span();

        // Try source extraction (spans are now accurate from ValueParser!)
        if span.end as usize <= self.source.len() {
            let raw = &self.source[span.start as usize..span.end as usize];

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
    /// Collapses consecutive whitespace characters (including \n, \t, \r) to single spaces
    /// while preserving whitespace inside quoted strings.
    ///
    /// This enables source extraction to work correctly with multiline input.
    fn normalize_whitespace(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut in_string = false;
        let mut string_delim = '\0';
        let mut prev_was_whitespace = false;

        for ch in s.chars() {
            match ch {
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
                ' ' | '\n' | '\t' | '\r' => {
                    // Outside string - collapse consecutive whitespace
                    if !prev_was_whitespace {
                        result.push(' ');
                        prev_was_whitespace = true;
                    }
                }
                _ => {
                    // Regular character
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
    fn print_css_value_semantic(&mut self, value: &CssValue) {
        match value {
            CssValue::Identifier { name, .. } => {
                let formatted = source_fidelity::format_identifier_value(name);
                self.write(&formatted);
            }
            CssValue::String { content, quote, .. } => {
                let formatted = source_fidelity::format_string_value(content, *quote);
                self.write(&formatted);
            }
            CssValue::Dimension { value, unit, .. } => {
                let formatted = source_fidelity::format_dimension_value(*value, unit);
                self.write(&formatted);
            }
            CssValue::Color { color, .. } => {
                let formatted = source_fidelity::format_color_value(color);
                self.write(&formatted);
            }
            CssValue::Function { name, args, .. } => {
                self.write(name);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_nested_value(arg);  // Use nested_value to preserve source fidelity
                }
                self.write(")");
            }
            CssValue::List { values, .. } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.print_nested_value(val);  // Use nested_value to preserve source fidelity
                }
            }
            CssValue::CommaSeparated { values, .. } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_nested_value(val);  // Use nested_value to preserve source fidelity
                }
            }
        }
    }

    /// Format a CSS value (right-hand side of declaration)
    pub(super) fn print_css_value(&mut self, value: &CssValue) {
        match value {
            CssValue::Identifier { name, .. } => {
                self.write(name);
            }
            CssValue::String { content, quote, span } => {
                // Extract raw source to preserve escape sequences
                let raw = &self.source[span.start as usize..span.end as usize];

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
                // Extract raw dimension from source to preserve leading zeros, etc.
                let raw = &self.source[span.start as usize..span.end as usize];
                self.write(raw);
            }
            CssValue::Color { color, .. } => {
                let formatted = source_fidelity::format_color_value(color);
                self.write(&formatted);
            }
            CssValue::Function { name, args, .. } => {
                self.write(name);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    // Try source extraction first (spans are now accurate from ValueCursor!)
                    // Falls back to semantic formatting if extraction fails
                    self.print_nested_value(arg);
                }
                self.write(")");
            }
            CssValue::List { values, .. } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    // Try source extraction first (spans are now accurate from ValueCursor!)
                    // Falls back to semantic formatting if extraction fails
                    self.print_nested_value(val);
                }
            }
            CssValue::CommaSeparated { values, .. } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    // Try source extraction first (spans are now accurate from ValueCursor!)
                    // Falls back to semantic formatting if extraction fails
                    self.print_nested_value(val);
                }
            }
        }
    }
}
