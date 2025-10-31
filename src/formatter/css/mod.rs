// CSS formatter - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates formatting of CSS nodes
// - **rules.rs**: Rule and declaration formatting (selectors, properties, values)
//
// ## Design Principles
//
// 1. **Match Prettier**: Format output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change CSS rendering semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod rules;

use crate::ast::internal::CssNode;
use crate::formatter::Formatter;

impl Formatter {
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
