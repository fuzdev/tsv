// Helper utilities for node formatting
//
// Provides utilities for expression tag formatting and source position tracking
// used in inline run grouping and multiline formatting decisions.

use crate::ast::internal::FragmentNode;
use crate::formatter_core::Formatter;

impl Formatter {
    /// Format an ExpressionTag
    ///
    /// Expression tags are Svelte-specific syntax for embedding TypeScript/JS
    /// expressions in the template: `{expression}`
    pub fn format_expression_tag(&mut self, tag: &crate::ast::internal::ExpressionTag) {
        self.write("{");
        // Format the expression directly
        self.format_ts_expression(&tag.expression);
        self.write("}");
    }

    /// Format a TypeScript expression (helper for expression tags)
    fn format_ts_expression(&mut self, expr: &tsv_ts::Expression) {
        match expr {
            tsv_ts::Expression::Literal(lit) => {
                self.write(&lit.raw);
            }
            tsv_ts::Expression::Identifier(id) => {
                let name = self.resolve_symbol(id.name);
                self.write(&name);
            }
        }
    }

    /// Get the content span for a node, skipping layout whitespace for text nodes
    ///
    /// Used for inline run grouping to determine if nodes are on the same source line.
    /// For text nodes, we skip both leading and trailing whitespace (which is often
    /// indentation and layout separation) to get the actual content position.
    pub fn get_content_span(&self, node: &FragmentNode) -> tsv_lang::Span {
        match node {
            FragmentNode::Text(text) => {
                // Skip leading and trailing whitespace to get the actual content position
                let leading_ws_len = text.raw.len() - text.raw.trim_start().len();
                let trailing_ws_len = text.raw.len() - text.raw.trim_end().len();
                tsv_lang::Span::new(
                    text.span.start + leading_ws_len as u32,
                    text.span.end - trailing_ws_len as u32,
                )
            }
            _ => node.span(),
        }
    }
}
