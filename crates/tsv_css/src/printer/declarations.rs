//! CSS declaration printing and wrapping logic
//!
//! Handles:
//! - Declaration printing (property: value;)
//! - Multiline wrapping decisions
//! - Width-based wrapping for long lists
//! - Doc building for width calculations

use super::{Printer, has_wrappable_args, value_normalization};
use crate::ast::internal::{self, CssValue};
use tsv_lang::doc::{self, DocContext, Mode, arena::DocId};

impl<'a> Printer<'a> {
    /// Write the declaration ending: optional `!important` tail and the semicolon with newline.
    ///
    /// The value span ends before the `!important` region, so that region — and any
    /// comments around it (`blue /* a */ !important /* b */;`) — is invisible to the
    /// value printers. Re-emit it from source here with comments preserved in place
    /// (like prettier) and `!`/`important` normalized to a single ` !important`.
    fn write_declaration_end(&mut self, decl: &internal::CssDeclaration) {
        if decl.is_important() {
            let bytes = self.source.as_bytes();
            let mut i = decl.span.end_usize();
            let mut out = String::new();
            while i < bytes.len() {
                match bytes[i] {
                    b';' | b'}' => break,
                    b'/' if bytes.get(i + 1) == Some(&b'*') => {
                        let end = self.source[i + 2..]
                            .find("*/")
                            .map_or(bytes.len(), |rel| i + 2 + rel + 2);
                        out.push(' ');
                        out.push_str(&self.source[i..end]);
                        i = end;
                    }
                    b'!' => {
                        out.push_str(" !important");
                        i += 1;
                    }
                    c if c.is_ascii_alphabetic() => {
                        // the `important` keyword itself — already emitted at the `!`
                        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                    }
                    _ => i += 1,
                }
            }
            self.write(&out);
        }
        self.write(";\n");
    }

    /// Check if any value in a list requires one-per-line formatting
    ///
    /// This matches Prettier's `shouldBreakList` which checks:
    /// `node.groups.some((node) => node.type === "value-comma_group")`
    ///
    /// Returns true if any value is a space-separated list (box-shadow, text-shadow, etc.)
    /// Functions are NOT checked here - they use doc-based wrapping with group/softline.
    fn any_value_needs_own_line(&self, values: &[CssValue]) -> bool {
        values.iter().any(|v| matches!(v, CssValue::List { .. }))
    }

    fn should_use_multiline(&self, decl: &internal::CssDeclaration) -> bool {
        // Only apply to comma-separated values with multiple items
        let values = match &decl.value {
            CssValue::CommaSeparated { values, .. } if values.len() > 1 => values,
            _ => return false,
        };

        // Custom properties skip structure-based multiline (one-per-line for List values).
        // They use width-based wrapping via should_wrap_value_width_based instead.
        // See fixture: declaration_long_multiline (continuation indent for space-separated items)
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
    /// - Long comma-separated lists (font-family, custom properties, etc.)
    /// - Long space-separated lists (transform chains, filter chains, etc.)
    ///
    /// Returns (needs_wrapping, is_comma_separated)
    fn should_wrap_value_width_based(&self, value: &CssValue, property: &str) -> (bool, bool) {
        match value {
            CssValue::CommaSeparated { values, .. } => {
                let doc = self.build_list_doc(values, ", ");
                let available = doc::available_width(
                    self.effective_indent(),
                    0,
                    property.len() + 3, // property + ": " + ";"
                );
                let exceeds_width = !doc::arena_fits::<dyn doc::TextResolver>(
                    &self.arena,
                    doc,
                    available,
                    Mode::Flat,
                    None,
                );
                (exceeds_width, true)
            }
            CssValue::List { values, .. } => {
                let doc = self.build_list_doc(values, " ");
                let available = doc::available_width(
                    self.effective_indent(),
                    0,
                    property.len() + 3, // property + ": " + ";"
                );
                let exceeds_width = !doc::arena_fits::<dyn doc::TextResolver>(
                    &self.arena,
                    doc,
                    available,
                    Mode::Flat,
                    None,
                );
                (exceeds_width, false)
            }
            _ => (false, false),
        }
    }

    /// Build doc representation of a list for width checking
    ///
    /// Consolidates comma-separated and space-separated list building.
    fn build_list_doc(&self, values: &[CssValue], separator: &'static str) -> DocId {
        self.d().join(
            values.iter().map(|v| self.build_css_value_doc(v)),
            separator,
        )
    }

    /// Check if a function should wrap its arguments (with explicit context offset)
    ///
    /// Prettier wraps ALL multi-arg functions when they exceed print width.
    /// This includes: gradients, polygon(), calc(), clamp(), min(), max(), var(), rgb(), hsl(), etc.
    ///
    /// Single-arg functions (like url('long-path')) never wrap because they have no
    /// natural break points. But functions with space-separated args (like drop-shadow)
    /// CAN wrap because they have multiple logical items.
    ///
    /// `context_offset` should be: property.len() + 3 (for ": " + ";")
    fn should_wrap_function_with_offset(
        &self,
        name: &str,
        args: &[CssValue],
        context_offset: usize,
    ) -> bool {
        if !has_wrappable_args(args) {
            return false;
        }

        let d = self.d();
        // Build inline doc representation for width checking
        // url() uses comma without space; others use ", "
        let separator = if name == "url" { "," } else { ", " };
        let args_doc = d.join(
            args.iter().map(|arg| self.build_css_value_doc(arg)),
            separator,
        );
        let name_doc = d.text_owned(name.to_string());
        let func_doc = d.concat(&[name_doc, d.parens(args_doc)]);

        let available = doc::available_width(self.effective_indent(), 0, context_offset);
        !doc::arena_fits::<dyn doc::TextResolver>(
            &self.arena,
            func_doc,
            available,
            Mode::Flat,
            None,
        )
    }

    /// Format a CSS declaration (property: value;)
    pub(super) fn print_css_declaration(&mut self, decl: &internal::CssDeclaration) {
        self.write_indent();

        // Extract property name from source to preserve escape sequences
        let decl_source = decl.span.extract(self.source);
        let property_normalized = value_normalization::extract_property_name(decl_source);
        self.write(&property_normalized);

        // Dispatch to appropriate handler based on value type and formatting needs
        if self.is_grid_multirow_value(decl) {
            self.print_decl_grid_multirow(decl);
        } else if self.should_use_multiline(decl) {
            self.print_decl_multiline(decl);
        } else if let (true, is_comma) =
            self.should_wrap_value_width_based(&decl.value, &decl.property)
        {
            self.print_decl_width_wrapped(decl, is_comma);
        } else if let CssValue::Function { name, args, span } = &decl.value {
            self.print_decl_function(decl, decl_source, name, args, *span);
        } else if self.has_value_comments_in_decl(decl) {
            self.print_decl_with_comments(decl, decl_source);
        } else if let CssValue::String { quote, .. } = &decl.value {
            self.print_decl_string(decl, decl_source, *quote);
        } else {
            self.print_decl_default(decl, &property_normalized);
        }
    }

    /// Print declaration with multiline formatting (structure-based)
    fn print_decl_multiline(&mut self, decl: &internal::CssDeclaration) {
        self.write(":\n");
        self.indent_level += 1;
        self.print_css_value_multiline(&decl.value);
        self.indent_level -= 1;
        self.write_declaration_end(decl);
    }

    /// Print declaration with width-based wrapping
    fn print_decl_width_wrapped(&mut self, decl: &internal::CssDeclaration, is_comma: bool) {
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
            self.print_space_list_wrapped(&decl.value);
            self.indent_level -= 1;
        }
        self.write_declaration_end(decl);
    }

    /// Print declaration with function value
    fn print_decl_function(
        &mut self,
        decl: &internal::CssDeclaration,
        decl_source: &str,
        name: &str,
        args: &[CssValue],
        span: tsv_lang::Span,
    ) {
        let has_comments = self.has_value_comments_in_decl(decl);
        let needs_wrap = self.function_needs_wrapping(decl, has_comments, name, args, span);

        if needs_wrap {
            self.print_wrapped_function(decl, has_comments, name, args);
        } else {
            self.print_inline_function(decl_source, has_comments, &decl.value);
        }
        self.write_declaration_end(decl);
    }

    /// Check if a function needs wrapping
    fn function_needs_wrapping(
        &self,
        decl: &internal::CssDeclaration,
        has_comments: bool,
        name: &str,
        args: &[CssValue],
        span: tsv_lang::Span,
    ) -> bool {
        if has_comments {
            // Use NORMALIZED source length for accurate width
            let func_source = span.extract(self.source);
            let normalized = value_normalization::normalize_value_spacing(func_source);
            let inline_len = decl.property.len() + 2 + normalized.len() + 1;
            self.indent_width() + inline_len > tsv_lang::PRINT_WIDTH
        } else {
            let context_offset = decl.property.len() + 3;
            self.should_wrap_function_with_offset(name, args, context_offset)
        }
    }

    /// Print wrapped function: func(\n\targ1,\n\targ2\n)
    fn print_wrapped_function(
        &mut self,
        decl: &internal::CssDeclaration,
        has_comments: bool,
        name: &str,
        args: &[CssValue],
    ) {
        self.write(": ");
        self.write(name);
        self.write("(\n");
        self.indent_level += 1;

        if has_comments {
            self.print_function_args_from_source(decl, name, args);
        } else {
            self.print_function_args_semantic_wrapped(args);
        }

        self.indent_level -= 1;
        self.write("\n");
        self.write_indent();
        self.write(")");
    }

    /// Print function args with semantic formatting and wrapping
    fn print_function_args_semantic_wrapped(&mut self, args: &[CssValue]) {
        for (i, arg) in args.iter().enumerate() {
            self.write_indent();
            if let CssValue::List { values, .. } = arg
                && self.space_list_exceeds_width(values, 2)
            {
                self.indent_level += 1;
                let fill_doc = self.build_space_fill_doc(values, 0);
                self.write_arena_doc(fill_doc);
                self.indent_level -= 1;
            } else {
                self.print_nested_value(arg);
            }
            if i < args.len() - 1 {
                self.write(",\n");
            }
        }
    }

    /// Print inline function (no wrapping)
    fn print_inline_function(&mut self, decl_source: &str, has_comments: bool, value: &CssValue) {
        self.write(": ");
        if has_comments
            && let Some(normalized) = value_normalization::extract_value_with_comments(decl_source)
        {
            self.write(&normalized);
        } else {
            self.print_css_value(value);
        }
    }

    /// Print declaration with comments in value (non-function)
    fn print_decl_with_comments(&mut self, decl: &internal::CssDeclaration, decl_source: &str) {
        self.write(": ");
        if let Some(normalized) = value_normalization::extract_value_with_comments(decl_source) {
            self.write(&normalized);
        } else {
            self.write(decl_source);
        }
        self.write_declaration_end(decl);
    }

    /// Print declaration with string value
    fn print_decl_string(
        &mut self,
        decl: &internal::CssDeclaration,
        decl_source: &str,
        quote: char,
    ) {
        self.write(": ");
        if let Some(formatted) = value_normalization::extract_string_value(decl_source, quote) {
            self.write(&formatted);
        } else {
            let formatted = value_normalization::format_string_value("", quote);
            self.write(&formatted);
        }
        self.write_declaration_end(decl);
    }

    /// Print declaration with default formatting
    fn print_decl_default(&mut self, decl: &internal::CssDeclaration, property: &str) {
        // Property with comment: `color /* comment */` → ` : `
        // Property without comment: `color` → `: `
        if property.contains("/*") {
            self.write(" : ");
        } else {
            self.write(": ");
        }
        // Empty custom-property value carrying !important (`--a: !important;`): the `: `
        // separator already supplies the single space, so emit `!important` without the
        // extra leading space `write_declaration_end` adds — avoids `--a:  !important;`.
        if decl.is_important()
            && matches!(&decl.value, CssValue::Identifier { name, .. } if name.is_empty())
        {
            self.write("!important;\n");
            return;
        }
        self.print_css_value(&decl.value);
        self.write_declaration_end(decl);
    }

    /// Check if this is a grid property with multiple row string values
    /// where consecutive values are on different source lines.
    ///
    /// Matches Prettier's source-position-dependent grid formatting
    /// (comma-separated-value-group.js lines 421-436): if consecutive values
    /// are on different source lines, wrap each to its own line.
    /// Properties: `grid-template-areas`, `grid-template*`, `grid`
    fn is_grid_multirow_value(&self, decl: &internal::CssDeclaration) -> bool {
        let prop = &decl.property;
        let is_grid_prop = prop == "grid" || prop.starts_with("grid-template");
        if !is_grid_prop {
            return false;
        }
        let values = match &decl.value {
            CssValue::List { values, .. }
                if values.len() >= 2
                    && values.iter().all(|v| matches!(v, CssValue::String { .. })) =>
            {
                values
            }
            _ => return false,
        };
        // Check source positions: are consecutive values on different lines?
        let source_bytes = self.source.as_bytes();
        for pair in values.windows(2) {
            let end = pair[0].span().end_usize();
            let start = pair[1].span().start_usize();
            if end <= start && source_bytes[end..start].contains(&b'\n') {
                return true;
            }
        }
        false
    }

    /// Print grid property with multiple row strings, one per line
    ///
    /// Format: `property:\n\t'row1'\n\t'row2'\n\t'row3';`
    fn print_decl_grid_multirow(&mut self, decl: &internal::CssDeclaration) {
        self.write(":\n");
        if let CssValue::List { values, .. } = &decl.value {
            self.indent_level += 1;
            for (i, val) in values.iter().enumerate() {
                self.write_indent();
                self.print_css_value(val);
                if i < values.len() - 1 {
                    self.write("\n");
                }
            }
            self.indent_level -= 1;
        }
        self.write_declaration_end(decl);
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
                // Check if value is a List that exceeds width - use continuation fill
                // Reserve 1 char for trailing comma
                if let CssValue::List {
                    values: list_values,
                    ..
                } = val
                    && self.space_list_exceeds_width(list_values, 1)
                {
                    // Use fill doc for long space-separated lists
                    // Increment indent for continuation lines
                    self.indent_level += 1;
                    let fill_doc = self.build_space_fill_doc(list_values, 1);
                    self.write_arena_doc(fill_doc);
                    self.indent_level -= 1;
                } else {
                    self.print_nested_value(val);
                }
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
        self.write_arena_doc(fill_doc);
    }

    /// Build a fill doc for comma-separated values
    ///
    /// Creates a doc that packs values greedily:
    /// - In flat mode: `item1, item2, item3`
    /// - When broken: `item1, item2,\n  item3, item4,\n  item5`
    ///
    /// For space-separated items (CssValue::List), each item is wrapped as
    /// `group(indent(fill([sub1, line, sub2, ...])))` so fill can break within
    /// items with continuation indent. This matches prettier's
    /// `printCommaSeparatedValueGroup` which returns `group(indent(fill(parts)))`.
    fn build_comma_fill_doc(&self, values: &[CssValue]) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        for (i, val) in values.iter().enumerate() {
            if let CssValue::List {
                values: list_values,
                ..
            } = val
            {
                // Space-separated values: build as group(indent(fill([sub1, line, sub2])))
                // so fill can break within items with continuation indent
                let sub_parts = self.build_space_fill_parts(list_values);
                let sub_fill = d.fill(&sub_parts);
                let sub_indented = d.indent(sub_fill);
                parts.push(d.group(sub_indented));
            } else {
                parts.push(self.build_css_value_doc(val));
            }
            if i < values.len() - 1 {
                // Separator: ", " in flat mode, ",\n" when broken
                let comma = d.text(",");
                let line = d.line();
                parts.push(d.concat(&[comma, line]));
            }
        }

        // Reserve 1 char for trailing semicolon to prevent fill from packing
        // to exactly printWidth and then exceeding when ';' is added
        let context = DocContext {
            trailing_reserve: 1,
        };
        let fill = d.fill(&parts);
        d.with_context(fill, context)
    }

    /// Format a space-separated list with width-based wrapping using doc::fill
    ///
    /// Breaks long space-separated lists (like transform chains) when they exceed print width.
    /// Uses doc::fill() for greedy packing (pack as many items per line as fit).
    /// Pattern: property: item1 item2\n\titem3;
    /// Note: First line stays inline with property, subsequent lines are indented
    fn print_space_list_wrapped(&mut self, value: &CssValue) {
        let CssValue::List { values, .. } = value else {
            self.print_nested_value(value);
            return;
        };

        // Build fill doc with space/line separators
        // Reserve 1 char for trailing semicolon
        let fill_doc = self.build_space_fill_doc(values, 1);

        // First line is inline (no indent), write_arena_doc uses current_column for width calc
        self.write_arena_doc(fill_doc);
    }

    /// Build fill parts for space-separated values (shared helper)
    ///
    /// Returns `[val1, line, val2, line, val3]` — suitable for `d.fill()`.
    /// Used by both declaration wrapping and function arg wrapping.
    pub(super) fn build_space_fill_parts(&self, values: &[CssValue]) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::with_capacity(values.len() * 2);
        for (i, val) in values.iter().enumerate() {
            parts.push(self.build_css_value_doc(val));
            if i < values.len() - 1 {
                parts.push(d.line());
            }
        }
        parts
    }

    /// Build a fill doc for space-separated values
    ///
    /// Creates a doc that packs values greedily:
    /// - In flat mode: `item1 item2 item3`
    /// - When broken: `item1 item2\n  item3 item4\n  item5`
    ///
    /// `trailing_reserve` accounts for characters after the list (comma, semicolon).
    fn build_space_fill_doc(&self, values: &[CssValue], trailing_reserve: usize) -> DocId {
        let d = self.d();
        let parts = self.build_space_fill_parts(values);
        let context = DocContext { trailing_reserve };
        let fill = d.fill(&parts);
        d.with_context(fill, context)
    }

    /// Check if a space-separated list would exceed width when printed inline
    ///
    /// Used to decide whether to use continuation-based fill printing.
    /// `trailing_reserve` accounts for characters after the list (comma, paren, semicolon).
    fn space_list_exceeds_width(&self, values: &[CssValue], trailing_reserve: usize) -> bool {
        let list_doc = self.build_list_doc(values, " ");
        let available = doc::available_width(self.effective_indent(), 0, trailing_reserve);
        !doc::arena_fits::<dyn doc::TextResolver>(
            &self.arena,
            list_doc,
            available,
            Mode::Flat,
            None,
        )
    }

    /// Print function arguments from source, preserving comments
    ///
    /// Used when a function has comments in its arguments and needs wrapping.
    /// Extracts each argument from the source string to preserve comments.
    fn print_function_args_from_source(
        &mut self,
        decl: &internal::CssDeclaration,
        func_name: &str,
        args: &[CssValue],
    ) {
        let decl_source = decl.span.extract(self.source);

        // Extract function args content from source, or fall back to semantic printing
        let Some(args_content) = value_normalization::extract_function_args(decl_source, func_name)
        else {
            self.print_function_args_semantic(args);
            return;
        };

        // Split by top-level commas and print each normalized arg
        let arg_strs = value_normalization::split_args_by_comma(args_content);
        for (i, arg_str) in arg_strs.iter().enumerate() {
            self.write_indent();
            let normalized = value_normalization::normalize_value_spacing(arg_str);

            // Check if this arg has space-separated values that would exceed width
            // Split by top-level spaces (not inside parens) to get individual values
            let space_parts = value_normalization::split_by_space_preserving_parens(&normalized);
            if space_parts.len() > 1 && self.arg_string_exceeds_width(&normalized) {
                // Use fill wrapping with continuation indent
                self.print_space_separated_with_fill(&space_parts);
            } else {
                self.write(&normalized);
            }

            if i < arg_strs.len() - 1 {
                self.write(",\n");
            }
        }
    }

    /// Check if an arg string would exceed width when printed at current position
    fn arg_string_exceeds_width(&self, arg: &str) -> bool {
        self.indent_width() + arg.len() > tsv_lang::PRINT_WIDTH
    }

    /// Print space-separated values with fill wrapping
    ///
    /// Uses continuation indent for wrapped lines. When the first part is a comment
    /// that fills the line, the comment is printed separately at base indent, then
    /// the value parts use continuation indent.
    fn print_space_separated_with_fill(&mut self, parts: &[&str]) {
        if parts.len() < 2 {
            if let Some(part) = parts.first() {
                self.write(part);
            }
            return;
        }

        // Check if first part is a comment that fills the line
        let first_is_comment = parts[0].trim().starts_with("/*");
        let first_len = parts[0].len();
        let second_len = parts[1].len();
        let first_fills_line =
            self.indent_width() + first_len + 1 + second_len > tsv_lang::PRINT_WIDTH;

        // When comment fills line: print it separately, then handle values with continuation
        let (value_parts, use_continuation) =
            if first_is_comment && first_fills_line && parts.len() > 2 {
                self.write(parts[0]);
                self.write("\n");
                self.write_indent();

                // Check if value parts need continuation indent
                let val1_len = parts[1].len();
                let val2_len = parts[2].len();
                let needs_wrap =
                    self.indent_width() + val1_len + 1 + val2_len > tsv_lang::PRINT_WIDTH;
                (&parts[1..], needs_wrap)
            } else {
                // Normal case: check if first two items fit together
                let both_fit =
                    self.indent_width() + first_len + 1 + second_len <= tsv_lang::PRINT_WIDTH;
                (parts, both_fit)
            };

        // Build and write fill doc
        let fill_doc = self.build_fill_parts_from_strings(value_parts);
        if use_continuation {
            self.indent_level += 1;
        }
        self.write_arena_doc(fill_doc);
        if use_continuation {
            self.indent_level -= 1;
        }
    }

    /// Build fill doc parts from string slices
    fn build_fill_parts_from_strings(&self, parts: &[&str]) -> DocId {
        let d = self.d();
        let mut doc_parts = Vec::with_capacity(parts.len() * 2);
        for (i, part) in parts.iter().enumerate() {
            doc_parts.push(d.text_owned((*part).to_string()));
            if i < parts.len() - 1 {
                doc_parts.push(d.line());
            }
        }
        d.fill(&doc_parts)
    }

    /// Print function arguments semantically (fallback when source extraction fails)
    fn print_function_args_semantic(&mut self, args: &[CssValue]) {
        for (i, arg) in args.iter().enumerate() {
            self.write_indent();
            self.print_nested_value(arg);
            if i < args.len() - 1 {
                self.write(",\n");
            }
        }
    }
}
