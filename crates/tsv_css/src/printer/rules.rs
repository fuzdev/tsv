// CSS rule and declaration formatting
//
// Handles formatting of:
// - CSS rules (selector + declarations block)
// - CSS declarations (property: value;)
//
// Selector formatting is handled by the selectors module.

use super::{Printer, source_fidelity};
use crate::ast::internal::{self, CssValue};
use tsv_lang::{PrintConfig, Span, doc, printing};

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
                        && printing::has_blank_line_between(
                            self.source,
                            prev_child.span().end,
                            comment.span.start,
                        )
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
                        if printing::is_same_line(
                            self.source,
                            nested_rule.span.end,
                            next_comment.span.start,
                        ) {
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
        //
        // Also check if any item is a function that would wrap (shouldBreakList behavior)
        // This catches cases like: `linear-gradient(...), radial-gradient(...)`
        // where if one function wraps, all items go one-per-line
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
    fn build_list_doc(&self, values: &[CssValue], separator: &str) -> doc::Doc {
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
    fn should_wrap_function(&self, name: &str, args: &[CssValue]) -> bool {
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
        let mut parts = vec![doc::text(name), doc::text("(")];

        // WORKAROUND: url() data URIs contain commas that our parser incorrectly treats as
        // argument separators (e.g., "url(data:image/png;base64,ABC)" is parsed as 2 args).
        // Use no space after commas for url() to preserve the original data URI format.
        let is_url = name == "url";

        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                if is_url {
                    parts.push(doc::text(","));
                } else {
                    parts.push(doc::text(", "));
                }
            }
            parts.push(self.build_value_doc(arg));
        }

        parts.push(doc::text(")"));
        doc::concat(parts)
    }

    /// Build a doc representation of a value for width checking
    ///
    /// This recursively builds doc representations of nested values.
    fn build_value_doc(&self, value: &CssValue) -> doc::Doc {
        match value {
            CssValue::Identifier { name, .. } => doc::text(name.to_string()),
            CssValue::String { content, .. } => {
                // Approximate string width (with quotes)
                doc::text(format!("'{content}'"))
            }
            CssValue::Dimension { span, .. } => {
                let raw = span.extract(self.source);
                doc::text(raw.to_string())
            }
            CssValue::Color { span, .. } => {
                let raw = span.extract(self.source);
                doc::text(raw.to_string())
            }
            CssValue::Function { name, args, .. } => {
                // Recursively build nested functions
                let mut parts = vec![doc::text(name.to_string()), doc::text("(")];
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    parts.push(self.build_value_doc(arg));
                }
                parts.push(doc::text(")"));
                doc::concat(parts)
            }
            CssValue::List { values, .. } => {
                let mut parts = Vec::new();
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(" "));
                    }
                    parts.push(self.build_value_doc(val));
                }
                doc::concat(parts)
            }
            CssValue::CommaSeparated { values, .. } => {
                let mut parts = Vec::new();
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    parts.push(self.build_value_doc(val));
                }
                doc::concat(parts)
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

        // Check if property needs multiline formatting (structure-based)
        if self.should_use_multiline(decl) {
            self.write(":\n");
            self.indent_level += 1;
            self.print_css_value_multiline(&decl.value);
            self.indent_level -= 1;
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

    /// Format a CSS value on multiple lines with greedy packing
    ///
    /// This is called when value needs wrapping (detected via newline in source or width).
    /// Uses greedy packing (like prettier's fill algorithm) to pack multiple items per line.
    ///
    /// Exception: Properties with space-separated items (like box-shadow, text-shadow)
    /// or wrappable functions (like gradients) use true one-per-line formatting.
    fn print_css_value_multiline(&mut self, value: &CssValue) {
        match value {
            CssValue::CommaSeparated { values, .. } => {
                // Check if any item needs one-per-line formatting:
                // - Space-separated list (box-shadow, text-shadow, etc.)
                // - Function that would wrap (linear-gradient, polygon, etc.)
                let needs_one_per_line = values.iter().any(|v| {
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
                });

                if needs_one_per_line {
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
            _ => {
                // Fallback to regular formatting
                self.print_nested_value(value);
            }
        }
    }

    /// Format a comma-separated list with width-based wrapping using greedy packing
    ///
    /// Breaks long comma-separated lists intelligently to fit within print width.
    /// Uses pair-based lookahead (like prettier's fill algorithm) to pack multiple items per line.
    /// Pattern: property:\n\titem1, item2,\n\titem3;
    fn print_comma_list_wrapped(&mut self, value: &CssValue) {
        let values = match value {
            CssValue::CommaSeparated { values, .. } => values,
            _ => {
                // Fallback
                self.print_nested_value(value);
                return;
            }
        };

        // Calculate available width on continuation lines
        // Account for base_indent_offset (e.g., when CSS is formatted inside Svelte)
        let total_indent = self.indent_level + self.config.base_indent_offset;
        let indent_width = total_indent * self.config.tab_width;
        let available = self.config.print_width.saturating_sub(indent_width);

        // Pre-calculate all item widths for lookahead
        let item_widths: Vec<usize> = values
            .iter()
            .map(|v| self.value_to_string(v).len())
            .collect();

        let mut current_line_items: Vec<&CssValue> = Vec::new();
        let mut current_width = 0;

        for (i, val) in values.iter().enumerate() {
            let val_width = item_widths[i];

            // Calculate what adding this item to the current line would cost
            let separator_width = if current_line_items.is_empty() { 0 } else { 2 }; // ", " before this item
            let item_contribution = separator_width + val_width;

            // Pair-based lookahead: check if adding THIS item AND NEXT item would fit
            // This is the key to greedy packing - we only break when we MUST
            let next_contribution = if i + 1 < values.len() {
                2 + item_widths[i + 1] // ", " + next item width
            } else {
                0 // No next item
            };

            // Width if we add current item + trailing comma (for non-final lines)
            let line_width_with_item = current_width + item_contribution;
            // Width if we also add the next item
            let line_width_with_pair = line_width_with_item + next_contribution;

            // Three-way decision (matching prettier's fill algorithm):
            // 1. Both current and next fit → add current, continue
            // 2. Only current fits → add current, break AFTER current, next goes to new line
            // 3. Neither fits → break BEFORE current

            // Note: prettier's greedy fill allows 1 char overflow (emergent behavior)
            // We use <= instead of < to allow fitting exactly at the boundary
            let both_fit = next_contribution == 0 || line_width_with_pair <= available;
            let current_fits = line_width_with_item <= available;

            if both_fit {
                // Case 1: Both fit - add current to line
                current_line_items.push(val);
                current_width += item_contribution;
            } else if current_fits && !current_line_items.is_empty() {
                // Case 2: Only current fits - add current, then break
                current_line_items.push(val);

                // Write current line with items and trailing comma
                self.write_indent();
                for (j, item) in current_line_items.iter().enumerate() {
                    if j > 0 {
                        self.write(", ");
                    }
                    self.print_nested_value(item);
                }
                self.write(",\n");

                // Start fresh for next iteration
                current_line_items = Vec::new();
                current_width = 0;
            } else if !current_line_items.is_empty() {
                // Case 3: Neither fits - break before current
                self.write_indent();
                for (j, item) in current_line_items.iter().enumerate() {
                    if j > 0 {
                        self.write(", ");
                    }
                    self.print_nested_value(item);
                }
                self.write(",\n");

                // Start new line with this item
                current_line_items = vec![val];
                current_width = val_width;
            } else {
                // First item on a line - just add it
                current_line_items.push(val);
                current_width += item_contribution;
            }
        }

        // Write final line (no trailing comma on final line)
        if !current_line_items.is_empty() {
            self.write_indent();
            for (j, item) in current_line_items.iter().enumerate() {
                if j > 0 {
                    self.write(", ");
                }
                self.print_nested_value(item);
            }
        }
    }

    /// Format a space-separated list with width-based wrapping using greedy packing
    ///
    /// Breaks long space-separated lists (like transform chains) when they exceed print width.
    /// Uses pair-based lookahead (like prettier's fill algorithm) to pack multiple items per line.
    /// Pattern: property: item1 item2\n\titem3;
    /// Note: First line stays inline with property, subsequent lines are indented
    ///
    /// `first_line_offset` is the width already consumed on the first line (property + ": ")
    fn print_space_list_wrapped(&mut self, value: &CssValue, first_line_offset: usize) {
        let values = match value {
            CssValue::List { values, .. } => values,
            _ => {
                // Fallback
                self.print_nested_value(value);
                return;
            }
        };

        // Calculate available width on continuation lines
        // Account for base_indent_offset (e.g., when CSS is formatted inside Svelte)
        let total_indent = self.indent_level + self.config.base_indent_offset;
        let indent_width = total_indent * self.config.tab_width;
        let continuation_available = self.config.print_width.saturating_sub(indent_width);

        // First line has less space (property name already written)
        // Calculate first line available: print_width - (prev indent + property + ": ")
        let prev_indent_width = (total_indent - 1) * self.config.tab_width; // one less indent on first line
        let first_line_available = self
            .config
            .print_width
            .saturating_sub(prev_indent_width + first_line_offset);

        // Pre-calculate all item widths for lookahead
        let item_widths: Vec<usize> = values
            .iter()
            .map(|v| self.value_to_string(v).len())
            .collect();

        let mut current_line_items: Vec<&CssValue> = Vec::new();
        let mut current_width = 0;
        let mut is_first_line = true;

        for (i, val) in values.iter().enumerate() {
            let val_width = item_widths[i];

            // Calculate what adding this item would cost
            let separator_width = if current_line_items.is_empty() { 0 } else { 1 }; // " "
            let item_contribution = separator_width + val_width;

            // Pair-based lookahead: check if adding THIS item AND NEXT item would fit
            let next_contribution = if i + 1 < values.len() {
                1 + item_widths[i + 1] // " " + next item width
            } else {
                0 // No next item
            };

            let line_width_with_item = current_width + item_contribution;
            let line_width_with_pair = line_width_with_item + next_contribution;

            // Use different available width for first line vs continuation lines
            let available = if is_first_line {
                first_line_available
            } else {
                continuation_available
            };

            // Three-way decision (matching prettier's fill algorithm):
            // 1. Both current and next fit → add current, continue
            // 2. Only current fits → add current, break AFTER current, next goes to new line
            // 3. Neither fits → break BEFORE current
            let both_fit = next_contribution == 0 || line_width_with_pair <= available;
            let current_fits = line_width_with_item <= available;

            if both_fit {
                // Case 1: Both fit - add current to line
                current_line_items.push(val);
                current_width += item_contribution;
            } else if current_fits && !current_line_items.is_empty() {
                // Case 2: Only current fits - add current, then break
                current_line_items.push(val);

                // Write current line
                for (j, item) in current_line_items.iter().enumerate() {
                    if j > 0 {
                        self.write(" ");
                    }
                    self.print_nested_value(item);
                }
                self.write("\n");

                // Start new line
                if is_first_line {
                    is_first_line = false;
                }
                self.write_indent();
                current_line_items = Vec::new();
                current_width = 0;
            } else if !current_line_items.is_empty() {
                // Case 3: Neither fits - break before current
                for (j, item) in current_line_items.iter().enumerate() {
                    if j > 0 {
                        self.write(" ");
                    }
                    self.print_nested_value(item);
                }
                self.write("\n");

                // Start new line with this item
                if is_first_line {
                    is_first_line = false;
                }
                self.write_indent();
                current_line_items = vec![val];
                current_width = val_width;
            } else {
                // First item on a line - just add it
                current_line_items.push(val);
                current_width += item_contribution;
            }
        }

        // Write final line
        if !current_line_items.is_empty() {
            for (j, item) in current_line_items.iter().enumerate() {
                if j > 0 {
                    self.write(" ");
                }
                self.print_nested_value(item);
            }
        }
    }

    /// Convert a value to a string for width calculation
    fn value_to_string(&self, value: &CssValue) -> String {
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
            value_comments: self.value_comments,
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
                    // Check if function should wrap
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
                // Check if function should wrap
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
