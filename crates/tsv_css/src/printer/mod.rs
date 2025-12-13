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

use crate::ast::internal::{CssBlockChild, CssComment, CssNode, CssStyleSheet};
use std::collections::HashMap;
use tsv_lang::{OutputBuffer, PrintConfig, doc, printing};

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
    /// Value comments side table (declaration span.start -> comments in value)
    pub(crate) value_comments: &'a HashMap<u32, Vec<CssComment>>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with source and value comments
    pub fn new(source: &'a str, value_comments: &'a HashMap<u32, Vec<CssComment>>) -> Self {
        Self::with_config(source, value_comments, PrintConfig::default())
    }

    /// Create a new printer with the given config
    pub fn with_config(
        source: &'a str,
        value_comments: &'a HashMap<u32, Vec<CssComment>>,
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            source,
            value_comments,
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

    /// Print a list of CSS nodes (rules)
    pub fn print_css_nodes(&mut self, nodes: &[CssNode]) {
        let mut i = 0;
        while i < nodes.len() {
            let node = &nodes[i];

            if i > 0 {
                let prev_node = &nodes[i - 1];

                // Special case: consecutive comments on same line
                if let (CssNode::Comment(_), CssNode::Comment(curr_comment)) = (prev_node, node)
                    && printing::is_same_line(
                        self.source,
                        prev_node.span().end,
                        curr_comment.span.start,
                    )
                {
                    // Print comment inline with space separator
                    self.write(" /*");
                    self.write(&curr_comment.content);
                    self.write("*/");
                    i += 1;
                    continue;
                }

                let has_blank_line_in_source = self.has_blank_line_between(prev_node, node);

                // Prettier preserves blank lines from source, otherwise uses single newline
                if has_blank_line_in_source {
                    self.write("\n\n");
                } else {
                    // All transitions (rule→rule, rule→comment, comment→rule, etc.) use single newline
                    self.write("\n");
                }
            }

            self.print_css_node(node);

            // Check if next node is an inline comment after a rule/at-rule closing brace
            if matches!(node, CssNode::Rule(_) | CssNode::Atrule(_))
                && let Some(CssNode::Comment(next_comment)) = nodes.get(i + 1)
                && printing::is_same_line(self.source, node.span().end, next_comment.span.start)
            {
                // Print comment inline after the closing brace
                self.write(" /*");
                self.write(&next_comment.content);
                self.write("*/");
                i += 1; // Skip the comment in next iteration
            }

            i += 1;
        }
        // Add trailing newline (matches prettier)
        self.write("\n");
    }

    /// Check if there's a blank line in the source between two nodes
    fn has_blank_line_between(&self, prev: &CssNode, curr: &CssNode) -> bool {
        let source = self.source;
        let prev_end = prev.span().end as usize;
        let curr_start = curr.span().start as usize;

        // For adjacent spans, check for trailing whitespace in prev span
        // AND leading whitespace in curr span
        let (search_start, search_end) = if prev_end == curr_start {
            // Look back for trailing whitespace in prev span
            let mut start = prev_end;
            for ch in source[..prev_end].chars().rev().take(20) {
                if ch.is_whitespace() {
                    start = start.saturating_sub(ch.len_utf8());
                } else {
                    break;
                }
            }

            // Look ahead for leading whitespace in curr span
            let mut end = curr_start;
            for ch in source[curr_start..].chars().take(20) {
                if ch.is_whitespace() {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }

            (start, end)
        } else {
            // Non-adjacent spans - check the gap between them
            (prev_end, curr_start)
        };

        if search_start < search_end && search_end <= source.len() {
            let between = &source[search_start..search_end];
            // Blank line = 2+ newlines in the whitespace
            between.matches('\n').count() >= 2
        } else {
            false
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
            CssNode::Comment(comment) => self.print_css_comment(comment),
            CssNode::Atrule(atrule) => self.print_css_atrule(atrule),
        }
    }

    /// Print a CSS comment
    pub(crate) fn print_css_comment(&mut self, comment: &CssComment) {
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
            && printing::is_same_line(self.source, last_end, next_comment.span.start)
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
            && printing::is_same_line(self.source, last_end, next_comment.span.start)
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
        printing::has_blank_line_between(self.source, prev_end, curr_start)
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
    let mut printer = Printer::new(source, &stylesheet.value_comments);
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
    let mut printer = Printer::with_config(source, &stylesheet.value_comments, config);
    printer.print_css_nodes(&stylesheet.nodes);
    printer.into_string()
}
