//! CSS declaration printing and wrapping logic
//!
//! Handles:
//! - Declaration printing (property: value;)
//! - Multiline wrapping decisions
//! - Width-based wrapping for long lists
//! - Doc building for width calculations

use super::{Printer, value_normalization};
use crate::ast::internal::{self, CssValue};
use tsv_lang::PRINT_WIDTH;
use tsv_lang::doc::{DocBuf, DocContext, arena::DocId};

impl<'a> Printer<'a> {
    /// Write the declaration ending: optional `!important` tail and the semicolon with newline.
    ///
    /// The value span ends before the `!important` region, so that region — and any
    /// comments around it (`blue /* a */ !important /* b */;`) — is invisible to the
    /// value printers. Re-emit it from source here with comments preserved in place
    /// (like prettier) and `!`/`important` normalized to a single ` !important`.
    fn write_declaration_end(&mut self, decl: &internal::CssDeclaration<'_>) {
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

    /// Emit a format-ignored declaration verbatim from source. The value span excludes
    /// the trailing `!important` region (and any comments inside it) plus the `;`, so
    /// `write_declaration_end` re-emits both — preserving a comment *after* the bang
    /// (`blue !important /* y */;`) that a hand-rolled synthetic ` !important` would drop.
    /// Shared by the rule and at-rule block loops.
    pub(super) fn write_format_ignore_declaration(&mut self, decl: &internal::CssDeclaration<'_>) {
        self.write_indent();
        self.write(decl.span.extract(self.source));
        self.write_declaration_end(decl);
    }

    /// Check if any value in a list requires one-per-line formatting
    ///
    /// This matches Prettier's `shouldBreakList` which checks:
    /// `node.groups.some((node) => node.type === "value-comma_group")`
    ///
    /// Returns true if any value is a space-separated list (box-shadow, text-shadow, etc.)
    /// Functions are NOT checked here - they use doc-based wrapping with group/softline.
    fn any_value_needs_own_line(&self, values: &[CssValue<'_>]) -> bool {
        values.iter().any(|v| matches!(v, CssValue::List { .. }))
    }

    fn should_use_multiline(&self, decl: &internal::CssDeclaration<'_>) -> bool {
        // Only apply to comma-separated values with multiple items
        let values = match &decl.value {
            CssValue::CommaSeparated { values, .. } if values.len() > 1 => values,
            _ => return false,
        };

        // Custom properties skip structure-based multiline (one-per-line for List values).
        // They take the self-deciding width path via `print_decl_value_list` instead.
        // See fixture: declaration_long_multiline (continuation indent for space-separated items)
        if decl.property.starts_with("--") {
            return false;
        }

        // Check source: is there a newline between `:` and the first value?
        let decl_source = decl.span.extract(self.source);
        if let Some(colon_pos) = value_normalization::find_declaration_colon(decl_source) {
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

    /// Print a declaration whose comment-free value is a comma- or space-separated
    /// list, as one self-deciding doc: the renderer's own fit check chooses inline
    /// vs. wrapped, so the wrap decision and the emission are a single representation
    /// and cannot drift (the doc-first shape the at-rule prelude and the function /
    /// multiline-continuation paths already use). Replaces the former measure-then-emit
    /// pair (a discarded flat-join measured to decide, then a *different* fill emitted).
    ///
    /// The two list kinds wrap differently, so each builds its own shape:
    /// - **comma** breaks *after* the colon — flat `prop: a, b`, broken
    ///   `prop:\n\ta,\n\tb` — via `group(indent([line, comma_fill]))`. The group's
    ///   flat-fit reserves the `;` through `comma_fill`'s own `trailing_reserve`, while
    ///   the `line` covers the colon-space, so the boundary matches the old width check
    ///   (`indent + property + ": " + ";" + join <= PRINT_WIDTH`) exactly.
    /// - **space** keeps `: ` literal and wraps only the tail — flat `prop: a b c`,
    ///   broken `prop: a b\n\tc` — via `indent(space_fill)`. A `space_fill` that fits
    ///   renders byte-identical to the old flat join; its last-item `trailing_reserve`
    ///   reproduces the old measure pass's wrap decision (and never leaks into the
    ///   nested `var`/`calc`/`color-mix` groups, which render in the fill's forced flat
    ///   mode — the boundary the suffix mechanism would have broken wrongly).
    ///
    /// Value comments aren't in the CSS AST, so a comment-bearing list isn't routed
    /// here (the dispatch guard); it stays on the source-extracting comment path.
    fn print_decl_value_list(&mut self, decl: &internal::CssDeclaration<'_>) {
        let doc = match &decl.value {
            CssValue::CommaSeparated { values, .. } => {
                let fill = self.build_comma_fill_doc(values);
                let d = self.d();
                let body = d.group(d.indent(d.concat(&[d.line(), fill])));
                d.concat(&[d.text(":"), body])
            }
            CssValue::List { values, .. } => {
                let fill = self.build_space_fill_doc(values, 1);
                let d = self.d();
                d.concat(&[d.text(": "), d.indent(fill)])
            }
            // The dispatch in `print_css_declaration` only routes comma/space lists here;
            // fall back to the plain `: value` form rather than panicking, matching the
            // crate's other defensive value guards (e.g. `print_comma_list_wrapped`).
            _ => {
                let value_doc = self.build_css_value_doc(&decl.value);
                let d = self.d();
                d.concat(&[d.text(": "), value_doc])
            }
        };
        self.write_arena_doc(doc);
        self.write_declaration_end(decl);
    }

    /// Format a CSS declaration (property: value;)
    pub(super) fn print_css_declaration(&mut self, decl: &internal::CssDeclaration<'_>) {
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
        } else if matches!(
            &decl.value,
            CssValue::CommaSeparated { .. } | CssValue::List { .. }
        ) && !self.has_value_comments_in_decl(decl)
        {
            self.print_decl_value_list(decl);
        } else if let CssValue::Function { name, args, span } = &decl.value {
            self.print_decl_function(decl, decl_source, name, args, *span);
        } else if self.has_value_comments_in_decl(decl) {
            self.print_decl_with_comments(decl, decl_source);
        } else if matches!(&decl.value, CssValue::String { .. }) {
            self.print_decl_string(decl, decl_source);
        } else {
            self.print_decl_default(decl, &property_normalized);
        }
    }

    /// Print declaration with multiline formatting (structure-based)
    fn print_decl_multiline(&mut self, decl: &internal::CssDeclaration<'_>) {
        self.write(":\n");
        self.indent_level += 1;
        self.print_css_value_multiline(&decl.value);
        self.indent_level -= 1;
        self.write_declaration_end(decl);
    }

    /// Print declaration with function value.
    ///
    /// A comment-free value renders through the shared `build_value_function_doc`
    /// group: the renderer's own fit check — with the trailing `;` reserved via
    /// `write_arena_doc_reserving` — decides flat-vs-wrapped, so the wrap decision
    /// and the emission are a single doc and cannot drift. A value with comments
    /// stays on the imperative source-extraction path, since CSS value comments
    /// aren't stored in the AST and so can't be expressed as a doc.
    fn print_decl_function(
        &mut self,
        decl: &internal::CssDeclaration<'_>,
        decl_source: &str,
        name: &str,
        args: &[CssValue<'_>],
        span: tsv_lang::Span,
    ) {
        if self.has_value_comments_in_decl(decl) {
            self.print_decl_function_with_comments(decl, decl_source, name, args, span);
        } else {
            self.write(": ");
            let doc = self.build_value_function_doc(name, args, span);
            // Reserve the trailing `;` for the OUTERMOST function group only (the
            // property + `: ` + `;` boundary the old measure pass used). `!important`
            // is not counted, so a function value carrying `!important` wraps on the
            // function alone.
            self.write_arena_doc_reserving(doc, 1);
        }
        self.write_declaration_end(decl);
    }

    /// Render a value doc, reserving `reserve` columns of trailing punctuation
    /// (the declaration's `;`) for the **outermost** group's fit decision only.
    ///
    /// Unlike `write_arena_doc_with_suffix` — whose `EmbedContext::suffix_width`
    /// every group's fit check subtracts — this appends a measurement-only trailing
    /// node (it renders nothing) after the doc. The outermost group's flat line
    /// reaches that node and counts it, but a nested group (a nested `calc`, a paren
    /// group) is separated from it by the outermost group's softline break, so its
    /// lookahead stops there and it never reserves the column. That keeps prettier's
    /// exact-width-boundary layout for nested groups, which a global suffix would
    /// wrongly break (e.g. a 100-column nested paren group).
    fn write_arena_doc_reserving(&mut self, doc: DocId, reserve: usize) {
        let reserved = {
            let d = self.d();
            let marker = d.with_context(
                d.empty(),
                DocContext {
                    trailing_reserve: reserve,
                    ..Default::default()
                },
            );
            d.concat(&[doc, marker])
        };
        self.write_arena_doc(reserved);
    }

    /// Print a function-valued declaration whose value contains comments.
    ///
    /// CSS value comments aren't stored in the AST, so the value is reconstructed
    /// from source text: a wrapped function splits its args from source (preserving
    /// the comments in place), an inline one re-emits the normalized value verbatim.
    fn print_decl_function_with_comments(
        &mut self,
        decl: &internal::CssDeclaration<'_>,
        decl_source: &str,
        name: &str,
        args: &[CssValue<'_>],
        span: tsv_lang::Span,
    ) {
        // Width check uses the NORMALIZED source length (comments included), since the
        // comments aren't in the doc and the value must round-trip verbatim.
        let func_source = span.extract(self.source);
        let normalized = value_normalization::normalize_value_spacing(func_source);
        let inline_len = decl.property.len() + 2 + normalized.len() + 1;
        let needs_wrap = self.indent_width() + inline_len > PRINT_WIDTH;

        self.write(": ");
        if needs_wrap {
            // Wrapped: func(\n\targ1,\n\targ2\n)
            self.write(name);
            self.write("(\n");
            self.indent_level += 1;
            self.print_function_args_from_source(decl, name, args);
            self.indent_level -= 1;
            self.write("\n");
            self.write_indent();
            self.write(")");
        } else if let Some(normalized) =
            value_normalization::extract_value_with_comments(decl_source)
        {
            self.write(&normalized);
        } else {
            self.print_css_value(&decl.value);
        }
    }

    /// Print declaration with comments in value (non-function)
    fn print_decl_with_comments(&mut self, decl: &internal::CssDeclaration<'_>, decl_source: &str) {
        self.write(": ");
        if let Some(normalized) = value_normalization::extract_value_with_comments(decl_source) {
            self.write(&normalized);
        } else {
            self.write(decl_source);
        }
        self.write_declaration_end(decl);
    }

    /// Print declaration with string value
    fn print_decl_string(&mut self, decl: &internal::CssDeclaration<'_>, decl_source: &str) {
        // The original quote is the first byte of the string value's span (recovered
        // from source, not stored).
        let quote = self.source.as_bytes()[decl.value.span().start_usize()] as char;
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
    fn print_decl_default(&mut self, decl: &internal::CssDeclaration<'_>, property: &str) {
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
            && matches!(&decl.value, CssValue::Identifier { span } if span.extract(self.source).trim().is_empty())
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
    fn is_grid_multirow_value(&self, decl: &internal::CssDeclaration<'_>) -> bool {
        let prop = decl.property;
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
    fn print_decl_grid_multirow(&mut self, decl: &internal::CssDeclaration<'_>) {
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
    fn print_css_value_multiline(&mut self, value: &CssValue<'_>) {
        let CssValue::CommaSeparated { values, .. } = value else {
            // Fallback to regular formatting
            self.print_nested_value(value);
            return;
        };

        if self.any_value_needs_own_line(values) {
            // True one-per-line for shadow-like properties and wrappable functions
            for (i, val) in values.iter().enumerate() {
                self.write_indent();
                // Every item reserves the trailing `,`/`;` (width 1) for its own fit
                // decision, so a wrappable item breaks one column early rather than
                // letting the terminator push the line to 101 (matching prettier and
                // tsv's hard-print-width stance). A space-separated List value self-wraps
                // via `build_space_fill_value_doc`'s `group(indent(fill))`; a non-List
                // value (e.g. a gradient function) wraps via its own value group.
                let doc = if let CssValue::List {
                    values: list_values,
                    ..
                } = val
                {
                    self.build_space_fill_value_doc(list_values)
                } else {
                    self.build_css_value_doc(val)
                };
                self.write_arena_doc_reserving(doc, 1);
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
    fn print_comma_list_wrapped(&mut self, value: &CssValue<'_>) {
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
    fn build_comma_fill_doc(&self, values: &[CssValue<'_>]) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
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
            ..Default::default()
        };
        let fill = d.fill(&parts);
        d.with_context(fill, context)
    }

    /// Build fill parts for space-separated values (shared helper)
    ///
    /// Returns `[val1, line, val2, line, val3]` — suitable for `d.fill()`.
    /// Used by both declaration wrapping and function arg wrapping.
    pub(super) fn build_space_fill_parts(&self, values: &[CssValue<'_>]) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::with_capacity(values.len() * 2);
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
    fn build_space_fill_doc(&self, values: &[CssValue<'_>], trailing_reserve: usize) -> DocId {
        let d = self.d();
        let parts = self.build_space_fill_parts(values);
        let context = DocContext {
            trailing_reserve,
            ..Default::default()
        };
        let fill = d.fill(&parts);
        d.with_context(fill, context)
    }

    /// Print function arguments from source, preserving comments
    ///
    /// Used when a function has comments in its arguments and needs wrapping.
    /// Extracts each argument from the source string to preserve comments.
    fn print_function_args_from_source(
        &mut self,
        decl: &internal::CssDeclaration<'_>,
        func_name: &str,
        args: &[CssValue<'_>],
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
        self.indent_width() + arg.len() > PRINT_WIDTH
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
        let first_fills_line = self.indent_width() + first_len + 1 + second_len > PRINT_WIDTH;

        // When comment fills line: print it separately, then handle values with continuation
        let (value_parts, use_continuation) =
            if first_is_comment && first_fills_line && parts.len() > 2 {
                self.write(parts[0]);
                self.write("\n");
                self.write_indent();

                // Check if value parts need continuation indent
                let val1_len = parts[1].len();
                let val2_len = parts[2].len();
                let needs_wrap = self.indent_width() + val1_len + 1 + val2_len > PRINT_WIDTH;
                (&parts[1..], needs_wrap)
            } else {
                // Normal case: check if first two items fit together
                let both_fit = self.indent_width() + first_len + 1 + second_len <= PRINT_WIDTH;
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
        let mut doc_parts = DocBuf::with_capacity(parts.len() * 2);
        for (i, part) in parts.iter().enumerate() {
            doc_parts.push(d.text_owned((*part).to_string()));
            if i < parts.len() - 1 {
                doc_parts.push(d.line());
            }
        }
        d.fill(&doc_parts)
    }

    /// Print function arguments semantically (fallback when source extraction fails)
    fn print_function_args_semantic(&mut self, args: &[CssValue<'_>]) {
        for (i, arg) in args.iter().enumerate() {
            self.write_indent();
            self.print_nested_value(arg);
            if i < args.len() - 1 {
                self.write(",\n");
            }
        }
    }
}
