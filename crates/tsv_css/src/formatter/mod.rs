// CSS formatter - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates formatting of CSS nodes, core Formatter
// - **selectors.rs**: Selector formatting (reusable across rules and at-rules)
// - **rules.rs**: Rule and declaration formatting (uses selectors module)
// - **atrules.rs**: At-rule formatting (@media, @keyframes, etc., uses selectors and rules)
//
// ## Design Principles
//
// 1. **Match Prettier**: Format output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change CSS rendering semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability
// 4. **Reusability**: Shared formatting logic (selectors) used by multiple modules

mod atrules;
mod rules;
mod selectors;

use crate::ast::internal::CssNode;
use tsv_lang::OutputBuffer;

/// Format configuration
#[derive(Debug, Clone)]
pub struct FormatConfig {
    /// Indent string (default: tabs)
    pub indent: &'static str,
    /// Maximum line width (default: 100)
    #[allow(dead_code)] // TODO: Use for line wrapping decisions
    pub print_width: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent: "\t",
            print_width: 100,
        }
    }
}

/// Formatter state for building output
pub struct Formatter {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Format configuration
    config: FormatConfig,
    /// Original source (for blank line detection)
    source: Option<String>,
}

impl Formatter {
    /// Create a new formatter with default config
    pub fn new() -> Self {
        Self::with_config(FormatConfig::default())
    }

    /// Create a new formatter with the given config
    pub fn with_config(config: FormatConfig) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            source: None,
        }
    }

    /// Create a new formatter with source (for blank line preservation)
    pub fn with_source(source: &str) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config: FormatConfig::default(),
            source: Some(source.to_string()),
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write indentation based on current indent level
    ///
    /// Used for formatting nested structures like CSS rules.
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, self.config.indent);
    }

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Format a list of CSS nodes (rules)
    pub fn format_css_nodes(&mut self, nodes: &[CssNode]) {
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                let prev_node = &nodes[i - 1];
                let has_blank_line_in_source = self.has_blank_line_between(prev_node, node);

                let prev_is_rule = matches!(prev_node, CssNode::Rule(_));
                let prev_is_comment = matches!(prev_node, CssNode::Comment(_));
                let prev_is_atrule = matches!(prev_node, CssNode::Atrule(_));
                let curr_is_rule = matches!(node, CssNode::Rule(_));
                let curr_is_comment = matches!(node, CssNode::Comment(_));
                let curr_is_atrule = matches!(node, CssNode::Atrule(_));

                // Check if previous node was a comment at the start (no rule before it)
                let prev_comment_at_start = prev_is_comment && i == 1;

                // Prettier preserves blank lines from source
                if has_blank_line_in_source {
                    self.write("\n\n");
                } else if (prev_is_rule || prev_is_atrule) && curr_is_comment {
                    // Rule/AtRule → Comment: always add blank line
                    self.write("\n\n");
                } else if prev_is_comment
                    && (curr_is_rule || curr_is_atrule)
                    && !prev_comment_at_start
                {
                    // Comment → Rule/AtRule: add blank line only if comment wasn't at start
                    self.write("\n\n");
                } else if (prev_is_rule || prev_is_atrule) && (curr_is_rule || curr_is_atrule) {
                    // Rule/AtRule → Rule/AtRule: single newline
                    self.write("\n");
                } else {
                    // All other cases: single newline
                    self.write("\n");
                }
            }
            self.format_css_node(node);
        }
        // Add trailing newline (matches prettier)
        self.write("\n");
    }

    /// Check if there's a blank line in the source between two nodes
    fn has_blank_line_between(&self, prev: &CssNode, curr: &CssNode) -> bool {
        if let Some(source) = &self.source {
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
        } else {
            // No source available - default to single newline
            false
        }
    }

    /// Format a single CSS node
    fn format_css_node(&mut self, node: &CssNode) {
        match node {
            CssNode::Rule(rule) => self.format_css_rule(rule),
            CssNode::Comment(comment) => self.format_css_comment(comment),
            CssNode::Atrule(atrule) => self.format_css_atrule(atrule),
        }
    }

    /// Format a CSS comment
    fn format_css_comment(&mut self, comment: &crate::ast::internal::CssComment) {
        // Write comment with delimiters - content is preserved exactly as written
        self.write("/*");
        self.write(&comment.content);
        self.write("*/");
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Format CSS nodes to a string
pub fn format_css(nodes: &[CssNode]) -> String {
    let mut formatter = Formatter::new();
    formatter.format_css_nodes(nodes);
    formatter.into_string()
}

/// Format CSS nodes to a string with source (for blank line preservation)
pub fn format_css_with_source(nodes: &[CssNode], source: &str) -> String {
    let mut formatter = Formatter::with_source(source);
    formatter.format_css_nodes(nodes);
    formatter.into_string()
}
