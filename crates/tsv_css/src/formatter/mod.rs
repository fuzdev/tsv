// CSS formatter - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates formatting of CSS nodes, core Formatter
// - **rules.rs**: Rule and declaration formatting (selectors, properties, values)
//
// ## Design Principles
//
// 1. **Match Prettier**: Format output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change CSS rendering semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod rules;

use crate::ast::internal::CssNode;

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
    buffer: String,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Format configuration
    config: FormatConfig,
}

impl Formatter {
    /// Create a new formatter with default config
    pub fn new() -> Self {
        Self::with_config(FormatConfig::default())
    }

    /// Create a new formatter with the given config
    pub fn with_config(config: FormatConfig) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            config,
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    /// Write indentation based on current indent level
    ///
    /// Used for formatting nested structures like CSS rules.
    pub(crate) fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.write(self.config.indent);
        }
    }

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer
    }

    /// Format a list of CSS nodes (rules)
    pub fn format_css_nodes(&mut self, nodes: &[CssNode]) {
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                self.write("\n"); // Blank line between rules
            }
            self.format_css_node(node);
        }
        // Add trailing newline (matches prettier)
        self.write("\n");
    }

    /// Format a single CSS node
    fn format_css_node(&mut self, node: &CssNode) {
        match node {
            CssNode::Rule(rule) => self.format_css_rule(rule),
            // TODO: Add more node types as needed (AtRule, Comment, etc.)
        }
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
