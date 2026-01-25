// CSS printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates printing of CSS nodes, core Printer
// - **selectors.rs**: Selector printing (reusable across rules and at-rules)
// - **rules.rs**: CSS rule printing (selector + block structure)
// - **declarations.rs**: Declaration printing + wrapping logic
// - **values.rs**: CSS value printing (all value types)
// - **atrules.rs**: At-rule printing (@media, @keyframes, etc., uses selectors and rules)
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change CSS rendering semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability
// 4. **Reusability**: Shared printing logic (selectors) used by multiple modules
// 5. **Hierarchy-Following**: Module structure mirrors CSS spec (rules → declarations → values)

mod atrules;
mod declarations;
mod rules;
mod selectors;
pub mod source_fidelity;
mod values;

use crate::ast::internal::{Comment, CssBlockChild, CssNode, CssStyleSheet, CssValue};
use tsv_lang::{CommentPosition, OutputBuffer, PrintConfig, classify_comment_fast, doc, printing};

/// Check if function args have wrappable content (break points)
///
/// Returns true if:
/// 1. Multiple comma-separated args (linear-gradient, rgb, etc.)
/// 2. Single arg that is a List with multiple space-separated items (drop-shadow)
pub(crate) fn has_wrappable_args(args: &[CssValue]) -> bool {
    args.len() >= 2
        || (args.len() == 1
            && matches!(&args[0], CssValue::List { values, .. } if values.len() >= 2))
}

/// Printer state for building output
pub struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Print configuration
    config: PrintConfig,
    /// Original source (for blank line detection and raw value extraction)
    pub(crate) source: &'a str,
    /// All comments sorted by span.start
    pub(crate) comments: &'a [Comment],
    /// Precomputed line break positions for O(log n) line boundary lookups
    pub(crate) line_breaks: &'a [u32],
}

impl<'a> Printer<'a> {
    /// Create a new printer with source, comments, and line_breaks
    pub fn new(source: &'a str, comments: &'a [Comment], line_breaks: &'a [u32]) -> Self {
        Self::with_config(source, comments, line_breaks, PrintConfig::default())
    }

    /// Create a new printer with the given config
    pub fn with_config(
        source: &'a str,
        comments: &'a [Comment],
        line_breaks: &'a [u32],
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            config,
            source,
            comments,
            line_breaks,
        }
    }

    /// Check if two positions are on the same line (O(log n) binary search)
    #[inline]
    pub(crate) fn is_same_line(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::is_same_line_fast(self.line_breaks, prev_end, curr_start)
    }

    /// Check if there's a blank line (2+ newlines) between two positions (O(log n) binary search)
    #[inline]
    pub(crate) fn has_blank_line_between(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::has_blank_line_between_fast(self.line_breaks, prev_end, curr_start)
    }

    /// Check if a declaration has value comments (comments inside the value, not property name)
    ///
    /// Value comments are comments that appear after the colon, e.g., `color: /* comment */ red;`
    /// Detected by scanning the source text directly (value comments are not stored in the Vec).
    pub(crate) fn has_value_comments_in_decl(
        &self,
        decl: &crate::ast::internal::CssDeclaration,
    ) -> bool {
        let decl_source = decl.span.extract(self.source);
        if let Some(colon_pos) = decl_source.find(':') {
            let value_part = &decl_source[colon_pos + 1..];
            value_part.contains("/*")
        } else {
            false
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write indentation based on current indent level
    ///
    /// Used for printing nested structures like CSS rules.
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, self.config.indent);
    }

    /// Remove trailing newline from buffer (for inline comment handling)
    pub(crate) fn buffer_remove_trailing_newline(&mut self) {
        self.buffer.pop_if_ends_with('\n');
    }

    /// Get the current column position (for doc-builder width calculations)
    ///
    /// Includes base_indent_offset to account for Svelte wrapper indentation
    /// that will be added to each line during final formatting.
    pub(crate) fn current_column(&self) -> usize {
        let col = self.buffer.current_column(self.config.tab_width);
        // Add wrapper indent width so fill calculations account for final indentation
        col + (self.config.base_indent_offset * self.config.tab_width)
    }

    /// Get the effective indent level for width calculations
    ///
    /// Includes base_indent_offset to account for external context (e.g., Svelte wrapper)
    /// that adds indentation to the final output.
    pub(crate) fn effective_indent(&self) -> usize {
        self.indent_level + self.config.base_indent_offset
    }

    /// Get the visual width of current indentation in characters
    ///
    /// Converts indent level to actual character width based on tab_width.
    pub(crate) fn indent_width(&self) -> usize {
        self.effective_indent() * self.config.tab_width
    }

    /// Write a Doc to the buffer, accounting for current column and indent level
    ///
    /// This handles the common pattern of:
    /// 1. Get current column position (which already includes base_indent_offset after newlines)
    /// 2. Print doc with indent-aware width calculations
    /// 3. Write the result to the buffer
    ///
    /// Note: base_indent_offset is already accounted for in position tracking after newlines
    /// (see doc::render_single_doc line breaks). We should NOT add it again here.
    pub(crate) fn write_doc(&mut self, doc: &doc::Doc) {
        let current_col = self.current_column();
        let output = doc::print_doc_with_indent(doc, &self.config, current_col, self.indent_level);
        self.write(&output);
    }

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Print a list of CSS nodes (rules) with comments interspersed by position
    pub fn print_css_nodes(&mut self, nodes: &[CssNode]) {
        // Use comment index for efficient traversal (comments are sorted)
        let mut comment_idx = 0;
        let mut prev_end: u32 = 0;
        let mut printed_any = false;

        for node in nodes {
            let node_start = node.span().start;
            let node_end = node.span().end;

            // Print comments between prev_end and this node
            let comments_before =
                self.print_leading_comments(prev_end, node_start, &mut comment_idx);

            // Add separator before node
            if printed_any || comments_before > 0 {
                // Determine where to measure blank line from
                let gap_start = if comments_before > 0 {
                    self.comments
                        .get(comment_idx.saturating_sub(1))
                        .map_or(prev_end, |c| c.span.end)
                } else {
                    prev_end
                };

                if self.has_blank_line_between_spans(gap_start, node_start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            }

            self.print_css_node(node);

            // Check for inline comments on same line as node's closing brace
            let inline_count = self.print_inline_comments_after_node(node_end, &mut comment_idx);

            prev_end = if inline_count > 0 {
                self.comments
                    .get(comment_idx - 1)
                    .map_or(node_end, |c| c.span.end)
            } else {
                node_end
            };

            printed_any = true;
        }

        // Print trailing comments after all nodes
        self.print_trailing_comments(prev_end, &mut comment_idx);

        // Add trailing newline (matches prettier)
        self.write("\n");
    }

    /// Print leading comments between prev_end and curr_start
    /// Returns the number of comments printed
    fn print_leading_comments(
        &mut self,
        prev_end: u32,
        curr_start: u32,
        comment_idx: &mut usize,
    ) -> usize {
        let mut printed = 0;
        let mut last_end = prev_end;

        while *comment_idx < self.comments.len() {
            let comment = &self.comments[*comment_idx];
            if comment.span.start >= curr_start {
                break;
            }

            // Skip comments that are inside previous node's span (e.g., prelude comments in at-rules)
            // These are handled by the node's own printing logic via comments_in_range()
            if comment.span.end <= prev_end {
                *comment_idx += 1;
                continue;
            }

            let position = classify_comment_fast(comment, prev_end, curr_start, self.line_breaks);

            // Skip trailing comments (same line as prev node)
            if prev_end > 0 && matches!(position, CommentPosition::Trailing) {
                *comment_idx += 1;
                last_end = comment.span.end;
                continue;
            }

            // Print with proper spacing
            if printed > 0 {
                // Check if this comment is on the same line as the previous comment
                if self.is_same_line(last_end, comment.span.start) {
                    self.write(" ");
                } else if self.has_blank_line_between_spans(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            } else if prev_end > 0 {
                // First comment after a node
                if self.has_blank_line_between_spans(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            }

            self.print_css_comment(comment);
            last_end = comment.span.end;
            *comment_idx += 1;
            printed += 1;
        }

        printed
    }

    /// Print inline comments on the same line after a node
    /// Returns the number of comments printed
    fn print_inline_comments_after_node(
        &mut self,
        node_end: u32,
        comment_idx: &mut usize,
    ) -> usize {
        let mut printed = 0;
        let mut last_end = node_end;

        while *comment_idx < self.comments.len() {
            let comment = &self.comments[*comment_idx];
            if !self.is_same_line(last_end, comment.span.start) {
                break;
            }

            self.write(" ");
            self.print_css_comment(comment);
            last_end = comment.span.end;
            *comment_idx += 1;
            printed += 1;
        }

        printed
    }

    /// Print trailing comments after all nodes
    fn print_trailing_comments(&mut self, prev_end: u32, comment_idx: &mut usize) {
        let mut last_end = prev_end;

        while *comment_idx < self.comments.len() {
            let comment = &self.comments[*comment_idx];

            // Skip comments that are inside previous node's span (e.g., prelude comments in at-rules)
            // These are handled by the node's own printing logic via comments_in_range()
            if comment.span.end <= prev_end {
                *comment_idx += 1;
                continue;
            }

            // Skip inline comments (same line as last item) - already handled
            if self.is_same_line(prev_end, comment.span.start) {
                *comment_idx += 1;
                last_end = comment.span.end;
                continue;
            }

            // Print with proper spacing
            if self.has_blank_line_between_spans(last_end, comment.span.start) {
                self.write("\n\n");
            } else {
                self.write("\n");
            }

            self.print_css_comment(comment);
            last_end = comment.span.end;
            *comment_idx += 1;
        }
    }

    /// Check if there's an opening brace between two spans
    ///
    /// Used to detect if a comment is inside a block (after `{`) vs after a selector (before `{`)
    pub(crate) fn has_opening_brace_between(&self, prev_end: u32, curr_start: u32) -> bool {
        let prev_end = prev_end as usize;
        let curr_start = curr_start as usize;

        if prev_end > curr_start || curr_start > self.source.len() {
            return false;
        }

        let between = &self.source[prev_end..curr_start];
        between.contains('{')
    }

    /// Normalize comment spacing in raw strings
    ///
    /// Ensures spaces around comment delimiters:
    /// - Add space before `/*` if there isn't one (unless at start)
    /// - Add space after `*/` if there isn't one (unless at end)
    ///
    /// Examples:
    /// - `"foo/* comment */bar"` → `"foo /* comment */ bar"`
    /// - `"/* comment */bar"` → `"/* comment */ bar"`
    /// - `"foo/* comment */"` → `"foo /* comment */"`
    pub(crate) fn normalize_comment_spacing(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 10);
        let mut chars = s.char_indices().peekable();

        while let Some((i, ch)) = chars.next() {
            if ch == '/' && chars.peek().map(|(_, c)| c) == Some(&'*') {
                // Found start of comment
                // Add space before /* if not at start and previous char isn't whitespace
                if i > 0 && !result.ends_with(char::is_whitespace) {
                    result.push(' ');
                }
                result.push('/');
                chars.next(); // consume the '*'
                result.push('*');
            } else if ch == '*' && chars.peek().map(|(_, c)| c) == Some(&'/') {
                // Found end of comment
                result.push('*');
                chars.next(); // consume the '/'
                result.push('/');
                // Add space after */ if next char exists and isn't whitespace
                if let Some((_, next_ch)) = chars.peek()
                    && !next_ch.is_whitespace()
                {
                    result.push(' ');
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Print a single CSS node
    fn print_css_node(&mut self, node: &CssNode) {
        match node {
            CssNode::Rule(rule) => self.print_css_rule(rule),
            CssNode::Atrule(atrule) => self.print_css_atrule(atrule),
        }
    }

    /// Print a CSS comment
    pub(crate) fn print_css_comment(&mut self, comment: &Comment) {
        // Write comment with delimiters - content is preserved exactly as written
        self.write("/*");
        self.write(&comment.content);
        self.write("*/");
    }

    /// Try to print inline comments after the current item
    ///
    /// Checks if the next item is a comment on the same line as `prev_end`.
    /// If so, prints it inline and returns the number of comments consumed.
    ///
    /// This consolidates the repeated pattern across rules.rs and atrules.rs.
    pub(crate) fn try_print_inline_comments(
        &mut self,
        children: &[CssBlockChild],
        current_idx: usize,
        prev_end: u32,
    ) -> usize {
        let mut consumed = 0;
        let mut last_end = prev_end;

        while let Some(CssBlockChild::Comment(next_comment)) =
            children.get(current_idx + 1 + consumed)
            && self.is_same_line(last_end, next_comment.span.start)
        {
            self.write(" /*");
            self.write(&next_comment.content);
            self.write("*/");
            last_end = next_comment.span.end;
            consumed += 1;
        }

        consumed
    }

    /// Try to print inline comments after a declaration
    ///
    /// Similar to `try_print_inline_comments` but handles the declaration-specific
    /// case where we need to remove the trailing newline before the first comment.
    pub(crate) fn try_print_inline_comments_after_decl(
        &mut self,
        children: &[CssBlockChild],
        current_idx: usize,
        prev_end: u32,
    ) -> usize {
        let mut consumed = 0;
        let mut last_end = prev_end;

        while let Some(CssBlockChild::Comment(next_comment)) =
            children.get(current_idx + 1 + consumed)
            && self.is_same_line(last_end, next_comment.span.start)
        {
            if consumed == 0 {
                // First inline comment - remove the trailing newline from declaration
                self.buffer_remove_trailing_newline();
            }
            self.write(" /*");
            self.write(&next_comment.content);
            self.write("*/");
            last_end = next_comment.span.end;
            consumed += 1;
        }

        if consumed > 0 {
            self.write("\n");
        }

        consumed
    }

    /// Check if there's a blank line between two spans in the source
    pub(crate) fn has_blank_line_between_spans(&self, prev_end: u32, curr_start: u32) -> bool {
        self.has_blank_line_between(prev_end, curr_start)
    }

    /// Check if previous sibling is a comment
    pub(crate) fn prev_is_comment(children: &[CssBlockChild], index: usize) -> bool {
        index > 0 && matches!(children.get(index - 1), Some(CssBlockChild::Comment(_)))
    }

    /// Check if previous sibling is a nested rule
    pub(crate) fn prev_is_rule(children: &[CssBlockChild], index: usize) -> bool {
        index > 0 && matches!(children.get(index - 1), Some(CssBlockChild::Rule(_)))
    }

    /// Get the end span of the previous sibling
    pub(crate) fn prev_span_end(children: &[CssBlockChild], index: usize) -> Option<u32> {
        if index == 0 {
            return None;
        }
        children.get(index - 1).map(|child| match child {
            CssBlockChild::Declaration(d) => d.span.end,
            CssBlockChild::Comment(c) => c.span.end,
            CssBlockChild::Rule(r) => r.span.end,
            CssBlockChild::Atrule(a) => a.span.end,
        })
    }
}

/// Format CSS stylesheet to a string
/// Requires source for blank line preservation and raw value extraction
pub fn format_css(stylesheet: &CssStyleSheet, source: &str) -> String {
    let mut printer = Printer::new(source, &stylesheet.comments, &stylesheet.line_breaks);
    printer.print_css_nodes(&stylesheet.nodes);
    printer.into_string()
}

/// Format CSS stylesheet with custom configuration
/// Use this when CSS is nested inside another language (e.g., Svelte)
/// with base_indent_offset to account for wrapper indentation
pub fn format_css_with_config(
    stylesheet: &CssStyleSheet,
    source: &str,
    config: PrintConfig,
) -> String {
    let mut printer = Printer::with_config(
        source,
        &stylesheet.comments,
        &stylesheet.line_breaks,
        config,
    );
    printer.print_css_nodes(&stylesheet.nodes);
    printer.into_string()
}
