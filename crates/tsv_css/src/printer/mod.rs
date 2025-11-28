// CSS printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates printing of CSS nodes, core Printer
// - **selectors.rs**: Selector printing (reusable across rules and at-rules)
// - **rules.rs**: Rule and declaration printing (uses selectors module)
// - **atrules.rs**: At-rule printing (@media, @keyframes, etc., uses selectors and rules)
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change CSS rendering semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability
// 4. **Reusability**: Shared printing logic (selectors) used by multiple modules

mod atrules;
mod rules;
mod selectors;
pub mod source_fidelity;

use crate::ast::internal::{CssComment, CssNode, CssStyleSheet};
use std::collections::HashMap;
use tsv_lang::{OutputBuffer, PrintConfig, printing};

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
    fn print_css_comment(&mut self, comment: &crate::ast::internal::CssComment) {
        // Write comment with delimiters - content is preserved exactly as written
        self.write("/*");
        self.write(&comment.content);
        self.write("*/");
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
