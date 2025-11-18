// CSS rule and declaration formatting
//
// Handles formatting of:
// - CSS rules (selector + declarations block)
// - CSS declarations (property: value;)
//
// Selector formatting is handled by the selectors module.

use super::{Printer, source_fidelity};
use crate::ast::internal::{self, CssValue};
use tsv_lang::{printing, Span};

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

                    // Check for all consecutive inline comments after the declaration
                    let mut last_end = decl.span.end;
                    let mut inline_comments = 0;

                    while let Some(internal::CssBlockChild::Comment(next_comment)) =
                        rule.declarations.get(i + 1 + inline_comments)
                        && printing::is_same_line(self.source, last_end, next_comment.span.start)
                    {
                        if inline_comments == 0 {
                            // First inline comment - remove the trailing newline from declaration
                            self.buffer_remove_trailing_newline();
                        }
                        self.write(" /*");
                        self.write(&next_comment.content);
                        self.write("*/");
                        last_end = next_comment.span.end;
                        inline_comments += 1;
                    }

                    if inline_comments > 0 {
                        self.write("\n");
                        i += inline_comments; // Skip all inline comments
                    }
                }
                internal::CssBlockChild::Comment(comment) => {
                    // Standalone comment (not inline after a declaration)
                    // Preserve blank line before comment if present in source
                    let mut added_blank_line = false;
                    if i > start_index
                        && let Some(prev_child) = rule.declarations.get(i - 1)
                        && printing::has_blank_line_between(self.source, prev_child.span().end, comment.span.start)
                    {
                        self.write("\n");
                        added_blank_line = true;
                    }

                    // Check if next sibling is a nested rule - if so, add blank line before comment
                    // (but only if we didn't already add one from source preservation)
                    if !added_blank_line
                        && let Some(next_child) = rule.declarations.get(i + 1)
                        && matches!(
                            next_child,
                            internal::CssBlockChild::Rule(_) | internal::CssBlockChild::Atrule(_)
                        )
                    {
                        // Comment before nested rule - add blank line before comment
                        if i > start_index {
                            self.write("\n");
                        }
                    }
                    self.write_indent();
                    self.print_css_comment(comment);
                    self.write("\n");
                }
                internal::CssBlockChild::Rule(nested_rule) => {
                    // CSS Nesting Module - format nested rule
                    // Add blank line before nested rule if there's a previous sibling
                    // UNLESS the previous sibling was a comment (blank line already added)
                    let prev_is_comment = i > 0
                        && matches!(
                            rule.declarations.get(i - 1),
                            Some(internal::CssBlockChild::Comment(_))
                        );
                    if i > start_index && !prev_is_comment {
                        self.write("\n");
                    }
                    self.write_indent();
                    self.print_css_rule(nested_rule);

                    // Check for inline comment after nested rule's closing brace
                    let mut has_inline_comment = false;
                    if let Some(internal::CssBlockChild::Comment(next_comment)) =
                        rule.declarations.get(i + 1)
                    {
                        // Check if comment is on same line as nested rule's closing brace
                        if printing::is_same_line(self.source, nested_rule.span.end, next_comment.span.start) {
                            self.write(" /*");
                            self.write(&next_comment.content);
                            self.write("*/");
                            has_inline_comment = true;
                        }
                    }

                    self.write("\n");

                    // Add blank line after nested rule if next sibling is a declaration
                    // (Don't add for comments - comment handles its own spacing)
                    if let Some(next_child) = rule.declarations.get(i + 1)
                        && let internal::CssBlockChild::Declaration(_) = next_child
                    {
                        self.write("\n");
                    }

                    // Skip inline comment since we already printed it
                    if has_inline_comment {
                        i += 1;
                    }
                }
                internal::CssBlockChild::Atrule(nested_atrule) => {
                    // Nested at-rule (e.g., @media inside a rule)
                    // Add blank line before nested at-rule if there's a previous sibling
                    let prev_is_comment = i > 0
                        && matches!(
                            rule.declarations.get(i - 1),
                            Some(internal::CssBlockChild::Comment(_))
                        );
                    if i > start_index && !prev_is_comment {
                        self.write("\n");
                    }
                    self.write_indent();
                    self.print_css_atrule(nested_atrule);
                    self.write("\n");

                    // Add blank line after nested at-rule if next sibling exists and is a declaration/comment
                    if let Some(next_child) = rule.declarations.get(i + 1)
                        && matches!(
                            next_child,
                            internal::CssBlockChild::Declaration(_)
                                | internal::CssBlockChild::Comment(_)
                        )
                    {
                        self.write("\n");
                    }
                }
            }
            i += 1;
        }
        self.indent_level -= 1;

        // Closing brace (with indentation)
        self.write_indent();
        self.write("}");
    }

    /// Check if a property should use multiline formatting
    ///
    /// Prettier uses multiline for comma-separated lists when:
    /// 1. Property has newline after colon in source (preserve formatting), OR
    /// 2. Any comma-separated item contains space-separated values (structure-based)
    ///
    /// Custom properties (--*) always stay inline.
    fn should_use_multiline(&self, decl: &internal::CssDeclaration) -> bool {
        // Only apply to comma-separated values with multiple items
        let values = match &decl.value {
            CssValue::CommaSeparated { values, .. } if values.len() > 1 => values,
            _ => return false,
        };

        // Custom properties always inline
        if decl.property.starts_with("--") {
            return false;
        }

        // Check source: is there a newline between `:` and the first value?
        let decl_source = decl.span.extract(self.source);
        if let Some(colon_pos) = decl_source.find(':') {
            let after_colon = &decl_source[colon_pos + 1..];
            for ch in after_colon.chars() {
                if ch == '\n' {
                    return true;
                }
                if !ch.is_whitespace() {
                    break;
                }
            }
        }

        // Structure-based: check if any comma-separated item has space-separated values
        // This catches cases like: `opacity 0.3s ease, transform 0.3s ease-out`
        // where each item is a space-separated list (the "one bad apple" rule)
        values.iter().any(|v| matches!(v, CssValue::List { .. }))
    }

    /// Format a CSS declaration (property: value;)
    pub(super) fn print_css_declaration(&mut self, decl: &internal::CssDeclaration) {
        self.write_indent();

        // Extract property name from source to preserve escape sequences
        // See: docs/SVELTE_COMPATIBILITY.md (CSS Quirks section)
        let decl_source = decl.span.extract(self.source);
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
                    self.print_nested_value(val); // Use nested_value for normalization
                    if i < values.len() - 1 {
                        self.write(",\n");
                    }
                }
            }
            _ => {
                // Fallback to regular formatting
                self.print_nested_value(value); // Use nested_value for normalization
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
        if span.end as usize <= self.source.len() {
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
                if args.is_empty() && span.end as usize <= self.source.len() {
                    let raw = span.extract(self.source);
                    self.write(raw);
                } else {
                    // Reconstruct function from name and args
                    self.write(name);
                    self.write("(");
                    // url() functions don't get spaces after commas (prettier behavior)
                    let is_url = name == "url";
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            if is_url {
                                self.write(",");
                            } else {
                                self.write(", ");
                            }
                        }
                        self.print_nested_value(arg); // Use nested_value to preserve source fidelity
                    }
                    self.write(")");
                }
            }
            CssValue::List { values, .. } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.print_nested_value(val); // Use nested_value to preserve source fidelity
                }
            }
            CssValue::CommaSeparated { values, .. } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_nested_value(val); // Use nested_value to preserve source fidelity
                }
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
                self.write(name);
                self.write("(");
                // url() functions don't get spaces after commas (prettier behavior)
                let is_url = name == "url";
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        if is_url {
                            self.write(",");
                        } else {
                            self.write(", ");
                        }
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
