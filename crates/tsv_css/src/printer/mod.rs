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

use crate::ast::internal::CssNode;
use tsv_lang::OutputBuffer;

/// Print configuration
#[derive(Debug, Clone)]
pub struct PrintConfig {
    /// Indent string (default: tabs)
    pub indent: &'static str,
    /// Maximum line width (default: 100)
    #[allow(dead_code)] // TODO: Use for line wrapping decisions
    pub print_width: usize,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            indent: "\t",
            print_width: 100,
        }
    }
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
}

impl<'a> Printer<'a> {
    /// Create a new printer with source
    pub fn new(source: &'a str) -> Self {
        Self::with_config(source, PrintConfig::default())
    }

    /// Create a new printer with the given config
    pub fn with_config(source: &'a str, config: PrintConfig) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            source,
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

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Print a list of CSS nodes (rules)
    pub fn print_css_nodes(&mut self, nodes: &[CssNode]) {
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                let prev_node = &nodes[i - 1];
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

/// Format CSS nodes to a string
/// Requires source for blank line preservation and raw value extraction
pub fn format_css(nodes: &[CssNode], source: &str) -> String {
    let mut printer = Printer::new(source);
    printer.print_css_nodes(nodes);
    printer.into_string()
}

/// Format CSS nodes to a string with source (deprecated - use format_css)
/// This function is kept for backward compatibility
#[deprecated(note = "Use format_css instead - source is now always required")]
pub fn format_css_with_source(nodes: &[CssNode], source: &str) -> String {
    format_css(nodes, source)
}
