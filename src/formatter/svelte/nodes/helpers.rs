// Helper utilities for node formatting
//
// Provides utilities for expression tag formatting and source position tracking
// used in inline run grouping and multiline formatting decisions.

use crate::ast::internal::FragmentNode;
use crate::formatter::Formatter;

impl Formatter {
    /// Format an ExpressionTag
    ///
    /// Expression tags are Svelte-specific syntax for embedding TypeScript/JS
    /// expressions in the template: `{expression}`
    pub fn format_expression_tag(&mut self, tag: &crate::ast::internal::ExpressionTag) {
        self.write("{");
        self.format_expression(&tag.expression);
        self.write("}");
    }

    /// Get the content span for a node, skipping leading whitespace for text nodes
    ///
    /// Used for inline run grouping to determine if nodes are on the same source line.
    /// For text nodes, we skip leading whitespace (which is often indentation) to get
    /// the actual content position.
    pub fn get_content_span(&self, node: &FragmentNode) -> crate::span::Span {
        match node {
            FragmentNode::Text(text) => {
                // Skip leading whitespace to get the actual content position
                let leading_ws_len = text.raw.len() - text.raw.trim_start().len();
                crate::span::Span::new(text.span.start + leading_ws_len as u32, text.span.end)
            }
            _ => node.span(),
        }
    }
}
