//! CSS declaration printing and wrapping logic
//!
//! Handles:
//! - Declaration printing (property: value;)
//! - Multiline wrapping decisions
//! - Width-based wrapping for long lists
//! - Doc building for width calculations

use super::{Printer, source_fidelity};
use crate::ast::internal::{self, CssValue};
use tsv_lang::doc;

impl<'a> Printer<'a> {
    /// Check if any value in a list requires one-per-line formatting
    ///
    /// Returns true if any value is:
    /// - A space-separated list (box-shadow, text-shadow, etc.)
    /// - A function that would wrap (linear-gradient, polygon, etc.)
    ///
    /// This is the "one bad apple" rule - if one item needs its own line, all do.
    fn any_value_needs_own_line(&self, values: &[CssValue]) -> bool {
        values.iter().any(|v| {
            // Space-separated list
            if matches!(v, CssValue::List { .. }) {
                return true;
            }
            // Function that would wrap
            if let CssValue::Function { name, args, .. } = v
                && self.should_wrap_function(name, args)
            {
                return true;
            }
            false
        })
    }

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

        // Structure-based check using shared helper
        self.any_value_needs_own_line(values)
    }

    /// Check if a value should use width-based wrapping
    ///
    /// Prettier uses width-based wrapping for:
    /// - Long comma-separated lists (font-family, animation-name, etc.)
    /// - Long space-separated lists (transform chains, filter chains, etc.)
    ///
    /// Returns (needs_wrapping, is_comma_separated)
    fn should_wrap_value_width_based(&self, value: &CssValue, property: &str) -> (bool, bool) {
        // Custom properties never wrap width-based
        if property.starts_with("--") {
            return (false, false);
        }

        match value {
            CssValue::CommaSeparated { values, .. } => {
                let doc = self.build_list_doc(values, ", ");
                let context_offset = property.len() + 4; // property + `: ` + `; `
                let exceeds_width =
                    !doc::fits_at(&doc, &self.config, self.indent_level, 0, context_offset);
                (exceeds_width, true)
            }
            CssValue::List { values, .. } => {
                let doc = self.build_list_doc(values, " ");
                let context_offset = property.len() + 4; // property + `: ` + `; `
                let exceeds_width =
                    !doc::fits_at(&doc, &self.config, self.indent_level, 0, context_offset);
                (exceeds_width, false)
            }
            _ => (false, false),
        }
    }

    /// Build doc representation of a list for width checking
    ///
    /// Consolidates comma-separated and space-separated list building.
    fn build_list_doc(&self, values: &[CssValue], separator: &'static str) -> doc::Doc {
        let docs: Vec<_> = values.iter().map(|v| self.build_value_doc(v)).collect();
        doc::join(docs, separator)
    }

    /// Check if a function should wrap its arguments
    ///
    /// Prettier wraps ALL multi-arg functions when they exceed print width.
    /// This includes: gradients, polygon(), calc(), clamp(), min(), max(), var(), rgb(), hsl(), etc.
    ///
    /// Single-arg functions (like url('long-path')) never wrap because they have no
    /// natural break points. But functions with space-separated args (like drop-shadow)
    /// CAN wrap because they have multiple logical items.
    pub(super) fn should_wrap_function(&self, name: &str, args: &[CssValue]) -> bool {
        // Check if we have wrappable content:
        // 1. Multiple comma-separated args (linear-gradient, rgb, etc.)
        // 2. Single arg that is a List with multiple space-separated items (drop-shadow)
        let has_wrappable_content = args.len() >= 2
            || (args.len() == 1
                && matches!(&args[0], CssValue::List { values, .. } if values.len() >= 2));

        if !has_wrappable_content {
            return false;
        }

        // Build doc representation and check if it fits
        let func_doc = self.build_function_doc(name, args);

        // Reserve space for property name (~15 chars estimate) + `: ` + `; `
        let context_offset = 15 + 4;
        !doc::fits_at(
            &func_doc,
            &self.config,
            self.indent_level,
            0,
            context_offset,
        )
    }

    /// Build a doc representation of a function for width checking
    ///
    /// This builds a doc tree representing the function as it would appear inline,
    /// which is then used with `fits()` to check if it exceeds the print width.
    fn build_function_doc(&self, name: &str, args: &[CssValue]) -> doc::Doc {
        // WORKAROUND: url() data URIs contain commas that our parser incorrectly treats as
        // argument separators (e.g., "url(data:image/png;base64,ABC)" is parsed as 2 args).
        // Use no space after commas for url() to preserve the original data URI format.
        let is_url = name == "url";
        let separator = if is_url { "," } else { ", " };

        let arg_docs: Vec<_> = args.iter().map(|arg| self.build_value_doc(arg)).collect();
        let args_doc = doc::join(arg_docs, separator);
        let parens_doc = doc::parens(args_doc);

        doc::concat(vec![doc::text_owned(name.to_string()), parens_doc])
    }

    /// Build a doc representation of a value for width checking
    ///
    /// This recursively builds doc representations of nested values.
    fn build_value_doc(&self, value: &CssValue) -> doc::Doc {
        match value {
            CssValue::Identifier { name, .. } => doc::text_owned(name.to_string()),
            CssValue::String { content, .. } => {
                // Approximate string width (with quotes)
                doc::text_owned(format!("'{content}'"))
            }
            CssValue::Dimension { span, .. } => {
                let raw = span.extract(self.source);
                doc::text_owned(raw.to_string())
            }
            CssValue::Color { span, .. } => {
                let raw = span.extract(self.source);
                doc::text_owned(raw.to_string())
            }
            CssValue::Function { name, args, .. } => {
                // Recursively build nested functions
                let arg_docs: Vec<_> = args.iter().map(|arg| self.build_value_doc(arg)).collect();
                let args_doc = doc::join(arg_docs, ", ");
                let parens_doc = doc::parens(args_doc);
                doc::concat(vec![doc::text_owned(name.to_string()), parens_doc])
            }
            CssValue::List { values, .. } => {
                let docs: Vec<_> = values.iter().map(|v| self.build_value_doc(v)).collect();
                doc::join_doc(docs, doc::text(" "))
            }
            CssValue::CommaSeparated { values, .. } => {
                let docs: Vec<_> = values.iter().map(|v| self.build_value_doc(v)).collect();
                doc::join_doc(docs, doc::text(", "))
            }
        }
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

        // Helper to write the declaration ending (!important if needed, then semicolon)
        let important = decl.important;

        // Check if property needs multiline formatting (structure-based)
        if self.should_use_multiline(decl) {
            self.write(":\n");
            self.indent_level += 1;
            self.print_css_value_multiline(&decl.value);
            self.indent_level -= 1;
            if important {
                self.write(" !important");
            }
            self.write(";\n");
        // Check if value needs width-based wrapping
        } else if let (true, is_comma) =
            self.should_wrap_value_width_based(&decl.value, &decl.property)
        {
            // Width-based wrapping
            if is_comma {
                // Comma-separated: property:\n\titem1, item2
                self.write(":\n");
                self.indent_level += 1;
                self.print_comma_list_wrapped(&decl.value);
                self.indent_level -= 1;
            } else {
                // Space-separated: property: item1 item2\n\titem3
                self.write(": ");
                self.indent_level += 1;
                // First line already has property name + ": " consumed
                let first_line_offset = decl.property.len() + 2; // property + ": "
                self.print_space_list_wrapped(&decl.value, first_line_offset);
                self.indent_level -= 1;
            }
            if important {
                self.write(" !important");
            }
            self.write(";\n");
        } else if self.has_value_comments_in_decl(decl) {
            // Value has comments - extract from source to preserve them
            if let Some(normalized) = source_fidelity::extract_value_with_comments(decl_source) {
                self.write(": ");
                self.write(&normalized);
                if important {
                    self.write(" !important");
                }
                self.write(";\n");
            } else {
                // Fallback: shouldn't happen
                self.write(": ");
                self.write(decl_source);
                if important {
                    self.write(" !important");
                }
                self.write(";\n");
            }
        } else if let CssValue::String { quote, .. } = &decl.value {
            // String values: extract from source to preserve escapes
            if let Some(formatted) = source_fidelity::extract_string_value(decl_source, *quote) {
                self.write(": ");
                self.write(&formatted);
                if important {
                    self.write(" !important");
                }
                self.write(";\n");
            } else {
                // Fallback: use semantic formatting
                self.write(": ");
                let formatted = source_fidelity::format_string_value("", *quote);
                self.write(&formatted);
                if important {
                    self.write(" !important");
                }
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
            if important {
                self.write(" !important");
            }
            self.write(";\n");
        }
    }

    /// Format a CSS value on multiple lines with greedy packing
    ///
    /// This is called when value needs wrapping (detected via newline in source or width).
    /// Uses greedy packing (like prettier's fill algorithm) to pack multiple items per line.
    ///
    /// Exception: Properties with space-separated items (like box-shadow, text-shadow)
    /// or wrappable functions (like gradients) use true one-per-line formatting.
    fn print_css_value_multiline(&mut self, value: &CssValue) {
        let CssValue::CommaSeparated { values, .. } = value else {
            // Fallback to regular formatting
            self.print_nested_value(value);
            return;
        };

        if self.any_value_needs_own_line(values) {
            // True one-per-line for shadow-like properties and wrappable functions
            for (i, val) in values.iter().enumerate() {
                self.write_indent();
                self.print_nested_value(val);
                if i < values.len() - 1 {
                    self.write(",\n");
                }
            }
        } else {
            // Greedy packing for simple lists (font-family, animation-name, etc.)
            self.print_comma_list_wrapped(value);
        }
    }

    /// Format a comma-separated list with width-based wrapping using doc::fill
    ///
    /// Breaks long comma-separated lists intelligently to fit within print width.
    /// Uses doc::fill() for greedy packing (pack as many items per line as fit).
    /// Pattern: property:\n\titem1, item2,\n\titem3;
    fn print_comma_list_wrapped(&mut self, value: &CssValue) {
        let CssValue::CommaSeparated { values, .. } = value else {
            self.print_nested_value(value);
            return;
        };

        // Build fill doc with comma+line separators
        let fill_doc = self.build_comma_fill_doc(values);

        // Write first line indentation, then let fill handle the rest
        self.write_indent();
        self.write_doc(&fill_doc);
    }

    /// Build a fill doc for comma-separated values
    ///
    /// Creates a doc that packs values greedily:
    /// - In flat mode: `item1, item2, item3`
    /// - When broken: `item1, item2,\n  item3, item4,\n  item5`
    fn build_comma_fill_doc(&self, values: &[CssValue]) -> doc::Doc {
        let mut parts = Vec::new();
        for (i, val) in values.iter().enumerate() {
            // Use value_to_string for source-fidelity formatting
            parts.push(doc::text_owned(self.value_to_string(val)));
            if i < values.len() - 1 {
                // Separator: ", " in flat mode, ",\n" when broken
                parts.push(doc::concat(vec![doc::text(","), doc::line()]));
            }
        }

        // Reserve 1 char for trailing semicolon to prevent fill from packing
        // to exactly printWidth and then exceeding when ';' is added
        let context = doc::DocContext {
            trailing_reserve: 1,
        };
        doc::with_context(doc::fill(parts), context)
    }

    /// Format a space-separated list with width-based wrapping using doc::fill
    ///
    /// Breaks long space-separated lists (like transform chains) when they exceed print width.
    /// Uses doc::fill() for greedy packing (pack as many items per line as fit).
    /// Pattern: property: item1 item2\n\titem3;
    /// Note: First line stays inline with property, subsequent lines are indented
    ///
    /// The `_first_line_offset` parameter is no longer needed - write_doc() automatically
    /// uses current_column() which accounts for the property name already written.
    fn print_space_list_wrapped(&mut self, value: &CssValue, _first_line_offset: usize) {
        let CssValue::List { values, .. } = value else {
            self.print_nested_value(value);
            return;
        };

        // Build fill doc with space/line separators
        let fill_doc = self.build_space_fill_doc(values);

        // First line is inline (no indent), write_doc uses current_column for width calc
        self.write_doc(&fill_doc);
    }

    /// Build a fill doc for space-separated values
    ///
    /// Creates a doc that packs values greedily:
    /// - In flat mode: `item1 item2 item3`
    /// - When broken: `item1 item2\n  item3 item4\n  item5`
    fn build_space_fill_doc(&self, values: &[CssValue]) -> doc::Doc {
        let mut parts = Vec::new();
        for (i, val) in values.iter().enumerate() {
            // Use value_to_string for source-fidelity formatting
            parts.push(doc::text_owned(self.value_to_string(val)));
            if i < values.len() - 1 {
                // Separator: " " in flat mode, "\n" when broken
                parts.push(doc::line());
            }
        }

        // Reserve 1 char for trailing semicolon to prevent fill from packing
        // to exactly printWidth and then exceeding when ';' is added
        let context = doc::DocContext {
            trailing_reserve: 1,
        };
        doc::with_context(doc::fill(parts), context)
    }
}
