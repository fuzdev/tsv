// Helper utilities for node formatting
//
// Provides utilities for expression tag formatting and source position tracking
// used in inline run grouping and multiline formatting decisions.

use crate::ast::internal::FragmentNode;
use crate::printer::Printer;
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

impl<'a> Printer<'a> {
    /// Format an ExpressionTag
    ///
    /// Expression tags are Svelte-specific syntax for embedding TypeScript/JS
    /// expressions in the template: `{expression}`
    pub fn print_expression_tag(&mut self, tag: &crate::ast::internal::ExpressionTag) {
        self.write("{");
        // Format the expression directly
        self.print_ts_expression(&tag.expression);
        self.write("}");
    }

    /// Format a TypeScript expression (helper for expression tags)
    ///
    /// Delegates to TypeScript printer logic for string literals (quote conversion).
    /// Static HTML attributes always use double quotes per HTML spec.
    fn print_ts_expression(&mut self, expr: &tsv_ts::Expression) {
        match expr {
            tsv_ts::Expression::Literal(lit) => {
                self.print_ts_literal(lit);
            }
            tsv_ts::Expression::Identifier(id) => {
                let name = self.resolve_symbol(id.name);
                self.write(&name);
            }
            tsv_ts::Expression::ObjectExpression(obj) => {
                // TODO: This is a simplified implementation that should be replaced
                // with proper TypeScript printer delegation for complex formatting.
                // For now, handle simple inline objects.
                self.write("{");
                if !obj.properties.is_empty() {
                    self.write(" ");
                    for (i, prop) in obj.properties.iter().enumerate() {
                        self.print_ts_expression(&prop.key);
                        if !prop.shorthand {
                            self.write(": ");
                            self.print_ts_expression(&prop.value);
                        }
                        if i < obj.properties.len() - 1 {
                            self.write(", ");
                        }
                    }
                    self.write(" ");
                }
                self.write("}");
            }
        }
    }

    /// Format a TypeScript literal (numbers and strings)
    ///
    /// For strings, applies smart quote selection (singleQuote: true) matching
    /// the TypeScript printer behavior. This ensures consistent quote handling
    /// across expression attributes and template expressions.
    fn print_ts_literal(&mut self, lit: &tsv_ts::Literal) {
        match &lit.value {
            tsv_ts::LiteralValue::Number(n) => {
                // Format the number value
                self.write(&n.to_string());
            }
            tsv_ts::LiteralValue::String { content: _, quote } => {
                // Extract raw literal from source (preserves escape sequences)
                // TypeScript AST was parsed with base_offset, so spans are absolute
                // positions in the full Svelte source.
                let start = lit.span.start as usize;
                let end = lit.span.end as usize;
                let raw_literal = &self.source()[start..end];

                // Extract content without surrounding quotes
                let raw_content = &raw_literal[1..raw_literal.len() - 1];

                // Format using shared utility (handles quote selection and escaping)
                let formatted =
                    format_string_literal(raw_content, *quote, StringFormatOptions::default());

                self.write(&formatted);
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
